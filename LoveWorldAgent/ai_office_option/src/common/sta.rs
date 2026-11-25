use serde_json::{Map, Value}; // JSON 类型
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering}; // 忙状态与连续超时
use std::sync::mpsc::{self, Receiver, Sender}; // 工作队列
use std::sync::{Arc, Mutex, OnceLock}; // 共享状态与一次性初始化
use std::thread; // STA 线程
use std::time::Duration; // 超时
use windows::Win32::System::Com::{CoInitializeEx, CoUninitialize, COINIT_APARTMENTTHREADED}; // COM 初始化
use crate::word::report::{err_json, ToolError}; // 错误信封

const MAX_CONSECUTIVE: u32 = 3; // 连续超时熔断阈值
pub type Dispatcher = fn(&str, &Map<String, Value>) -> Value; // 族内工具执行函数

enum Job { // STA 工作项
    Dispatch { // 工具分派
        cmd: String, // 工具名
        args: Map<String, Value>, // 参数
        reply: Sender<Value>, // 回复通道
    },
    Custom { // 短时 STA 内回调（如读取 HWND）
        task: Box<dyn FnOnce() + Send>,
    },
}
struct Inner { // 共享忙状态
    busy: AtomicBool, // STA 是否正在执行
    timeouts: AtomicU32, // 连续超时/忙碌次数
}
pub struct StaHandle { // 每族独立 STA 句柄
    tx: Mutex<Sender<Job>>, // 任务发送端
    inner: Arc<Inner>, // 忙状态
    label: &'static str, // 线程名
}
pub struct StaCell(OnceLock<StaHandle>); // 静态单元

impl StaCell { // 静态单元操作
    pub const fn new() -> Self { Self(OnceLock::new()) } // const 构造，可用于 static
    pub fn handle(&'static self, label: &'static str, worker: Dispatcher) -> &'static StaHandle { // 懒启动并返回句柄
        self.0.get_or_init(|| start(label, worker)) // 首次创建
    }
}
fn start(label: &'static str, worker: Dispatcher) -> StaHandle { // 启动 STA 线程
    let (tx, rx) = mpsc::channel::<Job>(); // 建通道
    let inner = Arc::new(Inner { busy: AtomicBool::new(false), timeouts: AtomicU32::new(0) }); // 共享状态
    let worker_inner = inner.clone(); // 线程副本
    thread::Builder::new().name(label.into()).spawn(move || worker_loop(label, worker, rx, worker_inner)).ok(); // 启动
    StaHandle { tx: Mutex::new(tx), inner, label } // 返回
}
fn worker_loop(label: &'static str, worker: Dispatcher, rx: Receiver<Job>, inner: Arc<Inner>) { // STA 串行循环
    let hr = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) }; // 初始化 STA
    if hr.is_err() { // 初始化失败
        while let Ok(job) = rx.recv() {
            if let Job::Dispatch { reply, .. } = job {
                let _ = reply.send(err_json(&ToolError::new("COM_ERROR", format!("{label} STA 初始化失败"))));
            }
        }
        return; // 退出
    }
    while let Ok(job) = rx.recv() { // 处理工作项
        match job {
            Job::Dispatch { cmd, args, reply } => {
                inner.busy.store(true, Ordering::SeqCst); // 标记忙
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| worker(&cmd, &args))); // 捕获 panic
                let value = match result { Ok(value) => value, Err(_) => err_json(&ToolError::new("INTERNAL_PANIC", "工具执行异常，已恢复").recoverable("重试该操作")) }; // 组装回复
                inner.busy.store(false, Ordering::SeqCst); // 标记空闲
                inner.timeouts.store(0, Ordering::SeqCst); // 正常完成清零
                let _ = reply.send(value); // 回复
            }
            Job::Custom { task } => {
                let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(task));
            }
        }
    }
    unsafe { CoUninitialize(); } // 释放 COM
}
impl StaHandle { // 句柄操作
    pub fn submit(&self, cmd: &str, args: &Map<String, Value>, timeout: Duration) -> Value { // 提交并等待
        if self.inner.busy.load(Ordering::SeqCst) { // 上一请求未完成
            let count = self.inner.timeouts.fetch_add(1, Ordering::SeqCst) + 1; // 累计
            crate::log_warn!("sta", "{} 工具 {cmd} 被拒绝：STA 忙碌（连续 {count} 次）", self.label); // 记录
            if count >= MAX_CONSECUTIVE { thread::spawn(|| { thread::sleep(Duration::from_millis(200)); std::process::exit(3); }); } // 受控回收
            return err_json(&ToolError::new("APP_BUSY", "Office 正在执行上一个操作，请稍后重试").recoverable("等待当前操作完成或重启 sidecar 后重试")); // 拒绝
        }
        let (reply_tx, reply_rx) = mpsc::channel::<Value>(); // 回复通道
        let sent = self.tx.lock().map(|tx| tx.send(Job::Dispatch { cmd: cmd.to_string(), args: args.clone(), reply: reply_tx }).is_ok()).unwrap_or(false); // 投递
        if !sent { return err_json(&ToolError::new("INTERNAL_PANIC", "STA 工作线程已退出").recoverable("重试该操作")); } // 发送失败
        match reply_rx.recv_timeout(timeout) { // 等待回复
            Ok(value) => value, // 成功
            Err(mpsc::RecvTimeoutError::Timeout) => { // 超时不退出
                let count = self.inner.timeouts.fetch_add(1, Ordering::SeqCst) + 1; // 累计
                crate::log_warn!("sta", "{} 工具 {cmd} 超时 {:?}（连续 {count} 次）", self.label, timeout); // 记录
                if count >= MAX_CONSECUTIVE { thread::spawn(|| { thread::sleep(Duration::from_millis(200)); std::process::exit(3); }); } // 受控回收
                err_json(&ToolError::new("TIMEOUT", "Office 操作超时").recoverable("如当前操作仍阻塞，后续请求将返回 APP_BUSY")) // 超时帧
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => err_json(&ToolError::new("APP_GONE", "STA 工作线程断开").recoverable("重试该操作")), // 断开
        }
    }
    pub fn submit_fn<T: Send + 'static>(&self, timeout: Duration, f: impl FnOnce() -> T + Send + 'static) -> Option<T> { // 在所属 STA 线程执行短任务（不计入熔断）
        if self.inner.busy.load(Ordering::SeqCst) {
            return None;
        }
        let (reply_tx, reply_rx) = mpsc::sync_channel::<T>(1);
        let task = Box::new(move || {
            let _ = reply_tx.send(f());
        });
        let sent = self.tx.lock().map(|tx| tx.send(Job::Custom { task }).is_ok()).unwrap_or(false);
        if !sent {
            return None;
        }
        reply_rx.recv_timeout(timeout).ok()
    }
}