use anyhow::{anyhow, bail, Result}; // 错误类型
use std::collections::{HashMap, VecDeque}; // 元素缓存与 LRU 队列
use std::hash::{Hash, Hasher}; // 退化稳定键哈希
use std::sync::{Arc, Mutex}; // 线程通道共享
use windows::core::Interface; // COM 接口转换
use windows::Win32::Foundation::{HWND, POINT}; // 基础类型
use windows::Win32::System::Com::{CoCreateInstance, CoInitializeEx, CoUninitialize, CLSCTX_ALL, COINIT_MULTITHREADED}; // COM 初始化
use windows::Win32::System::Ole::{SafeArrayAccessData, SafeArrayDestroy, SafeArrayGetLBound, SafeArrayGetUBound, SafeArrayUnaccessData}; // RuntimeId 数组读取
use windows::Win32::UI::Accessibility::{CUIAutomation, IUIAutomation, IUIAutomation2, IUIAutomationElement}; // UIA 接口
use crate::uia::element::element_info; // 元素属性（退化稳定键）
type JobFn = Box<dyn FnOnce(&mut UiaWorker) -> Result<serde_json::Value> + Send>; // 工作线程任务
type Reply = tokio::sync::oneshot::Sender<Result<serde_json::Value, String>>; // 异步回复通道
type Job = (JobFn, Reply); // 任务与回复
#[derive(Clone)] // 可克隆句柄
pub struct UiaHandle { // UIA 线程句柄
    sender: Arc<Mutex<std::sync::mpsc::Sender<Job>>>, // 任务发送端
}
impl UiaHandle { // 句柄方法
    pub async fn call<F>(&self, job: F) -> Result<serde_json::Value> // 提交任务并等待结果
    where
        F: FnOnce(&mut UiaWorker) -> Result<serde_json::Value> + Send + 'static, // 任务签名
    {
        let (reply_tx, reply_rx) = tokio::sync::oneshot::channel(); // 创建回复通道
        { // 限制锁作用域，避免跨 await 持有
            let sender = self.sender.lock().map_err(|_| anyhow!("UIA 任务通道锁失效"))?; // 获取发送端
            sender.send((Box::new(job), reply_tx)).map_err(|_| anyhow!("UIA 工作线程已退出"))?; // 投递任务
        }
        match tokio::time::timeout(std::time::Duration::from_secs(25), reply_rx).await { // 单次 UIA 任务 25 秒超时（B7）
            Ok(Ok(Ok(value))) => Ok(value), // 成功
            Ok(Ok(Err(err))) => Err(anyhow!(err)), // 工具错误
            Ok(Err(err)) => Err(anyhow!("UIA 工作线程无响应: {err}")), // 通道关闭
            Err(_) => Err(anyhow!("UIA 操作超时（25 秒），请缩小范围后重试")), // 超时返回明确错误
        }
    }
}
pub struct UiaWorker { // UIA 工作器（仅在专用线程内使用）
    automation: IUIAutomation, // UIA 自动化入口
    cache: HashMap<i64, IUIAutomationElement>, // 元素 ID 缓存
    order: VecDeque<i64>, // LRU 访问顺序（队首最旧）
    keys: HashMap<i64, Vec<i32>>, // 元素 ID → RuntimeId
    by_key: HashMap<Vec<i32>, i64>, // RuntimeId → 元素 ID（跨快照稳定）
    next_id: i64, // 下一个元素 ID
}
impl UiaWorker { // 工作器方法
    pub fn new() -> Result<Self> { // 初始化 COM 与 UIA
        unsafe { CoInitializeEx(None, COINIT_MULTITHREADED).ok()?; } // 初始化为 MTA
        let automation: IUIAutomation = unsafe { CoCreateInstance(&CUIAutomation, None, CLSCTX_ALL)? }; // 创建 UIA 实例
        if let Ok(extended) = automation.cast::<IUIAutomation2>() { let _ = unsafe { extended.SetConnectionTimeout(3000) }; let _ = unsafe { extended.SetTransactionTimeout(3000) }; } // 限制跨进程 UIA 调用时长，防挂死
        Ok(Self { automation, cache: HashMap::new(), order: VecDeque::new(), keys: HashMap::new(), by_key: HashMap::new(), next_id: 1 }) // 返回工作器
    }
    pub fn automation(&self) -> &IUIAutomation { &self.automation } // UIA 实例
    pub fn store(&mut self, element: &IUIAutomationElement) -> i64 { // 缓存元素并分配稳定 ID（上限 20000）
        let key = runtime_key(element).or_else(|| fallback_key(element)); // RuntimeId 不可用时用结构键兜底（B1）
        if let Some(key) = key.as_ref() { // 稳定键可用
            if let Some(existing) = self.by_key.get(key).copied() { // 同一元素已缓存
                let synthetic = key.first() == Some(&-1); // 结构键需确认不是同名兄弟
                let same = if synthetic { self.get(existing).ok().and_then(|cached| unsafe { self.automation.CompareElements(&cached, element) }.ok()).map(|value| value.as_bool()).unwrap_or(false) } else { true }; // CompareElements 去重
                if same { self.touch(existing); return existing; } // 复用 ID（跨快照稳定，供 diff 使用）
            }
        }
        while self.cache.len() >= 20000 { // 超限淘汰最久未用
            if let Some(oldest) = self.order.pop_front() { // 取出最旧 ID
                self.cache.remove(&oldest); // 移除元素
                if let Some(old_key) = self.keys.remove(&oldest) { self.by_key.remove(&old_key); } // 移除 RuntimeId 映射
            } else { break; } // 空队列退出
        }
        let element_id = self.next_id; // 当前 ID
        self.next_id += 1; // 递增
        self.cache.insert(element_id, element.clone()); // 写入缓存
        self.order.push_back(element_id); // 记录访问顺序
        if let Some(key) = key { // 记录 RuntimeId/结构键映射
            self.keys.insert(element_id, key.clone()); // ID → 键
            self.by_key.entry(key).or_insert(element_id); // 键 → 首个 ID（同名兄弟不复用，避免错元素）
        }
        element_id // 返回 ID
    }
    fn touch(&mut self, element_id: i64) { // 刷新 LRU 顺序
        if let Some(index) = self.order.iter().position(|item| *item == element_id) { // 命中位置
            self.order.remove(index); // 移除旧位置
            self.order.push_back(element_id); // 追加到队尾
        }
    }
    pub fn get(&self, element_id: i64) -> Result<IUIAutomationElement> { // 按 ID 取元素
        self.cache.get(&element_id).cloned().ok_or_else(|| anyhow!("元素 ID 不存在或已过期: {element_id}")) // 未命中报错
    }
    pub fn element_from_handle(&self, handle: i64) -> Result<IUIAutomationElement> { // 窗口句柄转元素
        let hwnd = HWND(handle as isize as *mut core::ffi::c_void); // 构造 HWND
        unsafe { self.automation.ElementFromHandle(hwnd) }.map_err(|err| anyhow!("ElementFromHandle 失败: {err}")) // 调用 UIA
    }
    #[allow(dead_code)] // P1 坐标工具将使用
    pub fn element_from_point(&self, x: i32, y: i32) -> Result<IUIAutomationElement> { // 坐标点转元素
        unsafe { self.automation.ElementFromPoint(POINT { x, y }) }.map_err(|err| anyhow!("ElementFromPoint 失败: {err}")) // 调用 UIA
    }
    pub fn wait(&self, milliseconds: u64) { // 简单等待
        std::thread::sleep(std::time::Duration::from_millis(milliseconds)); // TODO: P1 支持按元素/属性条件轮询等待
    }
}
fn runtime_key(element: &IUIAutomationElement) -> Option<Vec<i32>> { // 读取 UIA RuntimeId，作为跨快照稳定键
    unsafe { // 调用 UIA 与 Ole
        let array = element.GetRuntimeId().ok()?; // RuntimeId 数组
        if array.is_null() { return None; } // 空指针视为不可用
        let key = (|| { // 读取数组内容
            let lower = SafeArrayGetLBound(array, 1).ok()?; // 下界
            let upper = SafeArrayGetUBound(array, 1).ok()?; // 上界
            let count = (upper - lower + 1).max(0) as usize; // 元素数量
            if count < 2 { return None; } // 单元素 RuntimeId 不足以唯一标识（如酷狗自定义控件），退化为每次新 ID
            let mut data: *mut core::ffi::c_void = std::ptr::null_mut(); // 数据指针
            if SafeArrayAccessData(array, &mut data).is_err() { return None; } // 加锁失败
            let slice = std::slice::from_raw_parts(data as *const i32, count); // i32 数组
            let key = slice.to_vec(); // 复制为 Vec
            let _ = SafeArrayUnaccessData(array); // 解锁
            Some(key) // 返回键
        })(); // 立即执行
        let _ = SafeArrayDestroy(array); // 释放 RuntimeId 数组
        key // 返回结果
    }
}
fn fallback_key(element: &IUIAutomationElement) -> Option<Vec<i32>> { // RuntimeId 缺失时的结构稳定键（pid+类型+类名+AutomationId+矩形）
    let info = element_info(0, element); // 读取属性
    if info.process_id == 0 && info.control_type_id == 0 && info.rect.width == 0 && info.rect.height == 0 { return None; } // 属性全空时放弃
    if info.rect.width <= 0 && info.rect.height <= 0 && info.name.is_empty() && info.automation_id.is_empty() && info.class_name.is_empty() { return None; } // 零尺寸无标识容器不生成结构键，避免同名合并
    let mut hasher = std::collections::hash_map::DefaultHasher::new(); // 结构哈希
    info.automation_id.hash(&mut hasher); // AutomationId
    info.class_name.hash(&mut hasher); // 类名
    let hash = hasher.finish(); // 64 位哈希
    Some(vec![-1, info.process_id as i32, info.control_type_id, info.rect.x, info.rect.y, info.rect.width, info.rect.height, (hash & 0xFFFF_FFFF) as u32 as i32, (hash >> 32) as u32 as i32]) // 负数魔法位与 RuntimeId 区分
}
impl Drop for UiaWorker { // 线程退出时释放 COM
    fn drop(&mut self) { unsafe { CoUninitialize(); } } // 反初始化 COM
}
pub fn spawn() -> Result<UiaHandle> { // 启动专用 UIA 线程
    let (sender, receiver) = std::sync::mpsc::channel::<Job>(); // 任务通道
    let (ready_tx, ready_rx) = std::sync::mpsc::channel::<Result<(), String>>(); // 就绪通道
    std::thread::Builder::new().name("uia-worker".to_string()).spawn(move || { // 创建线程
        let mut worker = match UiaWorker::new() { // 初始化工作器
            Ok(worker) => { let _ = ready_tx.send(Ok(())); worker } // 通知就绪
            Err(err) => { let _ = ready_tx.send(Err(err.to_string())); return; } // 通知失败并退出
        };
        while let Ok((job, reply)) = receiver.recv() { // 串行处理任务
            let result = job(&mut worker).map_err(|err| err.to_string()); // 执行任务
            let _ = reply.send(result); // 回传结果
        }
    })?; // 启动失败直接返回
    match ready_rx.recv_timeout(std::time::Duration::from_secs(5)) { // 等待初始化结果
        Ok(Ok(())) => Ok(UiaHandle { sender: Arc::new(Mutex::new(sender)) }), // 就绪
        Ok(Err(err)) => bail!("初始化 UIA 失败: {err}"), // 初始化失败
        Err(err) => bail!("等待 UIA 线程就绪失败: {err}"), // 超时
    }
}
