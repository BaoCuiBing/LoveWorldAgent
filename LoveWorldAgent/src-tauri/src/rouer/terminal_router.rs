use std::collections::HashMap; // 右侧终端路由：conpty-oxide 启动 cmd.exe，用户/智能体会话分池，WebSocket 主通道与 SSE 兼容通道
use std::path::PathBuf; // 路径类型
use std::sync::atomic::{AtomicBool, Ordering}; // 原子布尔标志
use std::sync::Arc; // 会话与管理器共享
use std::time::Duration; // 超时与轮询间隔
use axum::body::Bytes; // 请求体类型
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade}; // WebSocket 消息与升级
use axum::extract::{Query, State}; // 查询参数与状态提取
use axum::response::sse::{Event, Sse}; // SSE 响应
use axum::response::{IntoResponse, Response}; // 响应 trait
use axum::routing::{get, post}; // GET/POST 方法
use axum::Router; // 路由器类型
use conpty_oxide::tokio::Command as PtyCommand; // PTY 命令构造器
use conpty_oxide::{PtyController, SessionOptions, Size as PtySize}; // PTY 控制器与尺寸
use futures_util::{SinkExt, StreamExt}; // 流读写扩展
use once_cell::sync::OnceCell; // 全局单例
use parking_lot::Mutex; // 互斥锁
use serde_json::{json, Value}; // JSON 构造
use tokio::io::{AsyncReadExt, AsyncWriteExt}; // 异步读写
use tokio::sync::{mpsc, oneshot}; // 通道类型
use uuid::Uuid; // 会话 id 生成
use crate::state::{as_bool, body_object, fail, json_str, ok, ApiResult, AppState}; // 请求解析与响应工具
use crate::utils::logger; // 日志输出
pub const READ_CHUNK: usize = 4096; // 单次从 PTY 读取的字节上限
pub const IDLE_LIMIT_SECONDS: f64 = 7200.0; // 空闲会话回收阈值（2 小时）
pub const DETACHED_LIMIT_SECONDS: f64 = 120.0; // 无订阅者会话回收阈值（防止关标签留下孤儿 cmd.exe）
pub const MAX_USER_SESSIONS: usize = 12; // 用户终端标签并发上限
pub const MAX_AGENT_SESSIONS: usize = 4; // 智能体终端并发上限（与用户分池，互不挤占）
pub const BUFFER_LIMIT: usize = 200000; // 会话输出缓冲上限（字符），超出保留尾部
pub const REPLAY_LIMIT: usize = 20000; // 重订阅时回放的尾部字符数
pub const INIT_TIMEOUT_SECONDS: u64 = 15; // 等待前端 init 帧的超时
pub const KINDS: [&str; 2] = ["user", "agent"]; // 会话归属：用户标签 / 智能体工具
pub type Sink = Arc<dyn Fn(Value) + Send + Sync>; // 订阅者回调：WebSocket 投递到 mpsc，SSE 投递到队列
pub struct TerminalSession { // 一个 PTY 会话的全部状态
    pub id: String, // 会话 id
    pub kind: String, // 归属类型
    pub root: String, // 项目根标识
    pub cwd: String, // 当前工作目录
    pub created: Mutex<f64>, // 最近活跃时间（读线程与写入都会刷新）
    pub detached_at: Mutex<f64>, // 无订阅者起始时间
    pub buffer: Mutex<String>, // 输出回滚缓冲
    pub subscribers: Mutex<Vec<Sink>>, // 订阅者回调列表
    pub persistent: AtomicBool, // 是否后台保活
    pub command: Mutex<String>, // 当前命令
    pub alive: AtomicBool, // 进程是否存活
    pub closed: AtomicBool, // 是否已请求关闭
    pub input: Mutex<Option<mpsc::UnboundedSender<Vec<u8>>>>, // conin 写入队列（丢弃即结束会话）
    pub control: Mutex<Option<PtyController>>, // conpty 尺寸/清屏控制句柄
    pub killer: Mutex<Option<oneshot::Sender<()>>>, // 请求终止整棵进程树
}
impl TerminalSession { // 会话状态查询
    pub fn subscriber_count(&self) -> usize { // 订阅者数量
        self.subscribers.lock().len() // 读取列表长度
    }
    pub fn run_alive(&self) -> bool { // 会话是否仍在运行
        self.alive.load(Ordering::SeqCst) && !self.closed.load(Ordering::SeqCst) // 存活且未关闭
    }
}
pub struct Unsubscriber { // 订阅守卫，析构时自动退订
    manager: Arc<TerminalManager>, // 管理器引用
    session: Arc<TerminalSession>, // 会话引用
    sink: Sink, // 订阅回调
}
impl Drop for Unsubscriber { // 析构退订
    fn drop(&mut self) { // 生命周期结束
        self.manager.unsubscribe(&self.session, &self.sink); // 移除订阅
    }
}
pub struct TerminalManager { // 终端会话管理器
    sessions: Mutex<HashMap<String, Arc<TerminalSession>>>, // id 到会话的映射
}
static MANAGER: OnceCell<Arc<TerminalManager>> = OnceCell::new(); // 全局单例
pub fn manager() -> Arc<TerminalManager> { // 获取全局管理器
    MANAGER.get_or_init(|| Arc::new(TerminalManager { sessions: Mutex::new(HashMap::new()) })).clone() // 首次访问时初始化
}
fn now_seconds() -> f64 { // 当前 Unix 时间戳
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|value| value.as_secs_f64()).unwrap_or(0.0) // 系统时间转秒
}
fn keep_tail(text: &mut String, limit: usize) { // 只保留文本尾部 limit 个字符
    let total = text.chars().count(); // 字符总数
    if total <= limit { return; } // 未超限直接返回
    let start = total - limit; // 起始位置
    *text = text.chars().skip(start).collect(); // 保留尾部
}
pub fn resolve_cwd(raw: &str) -> PathBuf { // 解析工作目录，限制在项目根内
    let base = crate::config::project_path().unwrap_or_else(crate::config::app_dir); // 项目根，未选回退应用目录
    let base = crate::config::strip_verbatim(std::fs::canonicalize(&base).unwrap_or(base)); // 规范化项目根
    let text = raw.trim(); // 目标目录文本
    if text.is_empty() { return base; } // 空则用项目根
    let candidate = PathBuf::from(text); // 候选路径
    let resolved = crate::config::strip_verbatim(std::fs::canonicalize(&candidate).unwrap_or(candidate)); // 规范化候选
    if resolved == base || resolved.starts_with(&base) { resolved } else { base } // 越界回退项目根
}
fn normalize_kind(kind: &str) -> String { // 归一化会话归属
    let text = kind.trim().to_lowercase(); // 去空白转小写
    if KINDS.contains(&text.as_str()) { text } else { "user".to_string() } // 非法值按用户处理
}
impl TerminalManager { // 管理器操作
    pub fn get(&self, id: &str) -> Option<Arc<TerminalSession>> { // 按 id 取会话
        self.sessions.lock().get(id).cloned() // 克隆 Arc
    }
    pub fn exists(&self, id: &str) -> bool { // 判断会话是否存在
        self.sessions.lock().contains_key(id) // 键存在性
    }
    pub fn all(&self) -> Vec<Arc<TerminalSession>> { // 按活跃时间排序列出全部会话
        let mut items: Vec<(f64, Arc<TerminalSession>)> = self.sessions.lock().values().map(|session| (*session.created.lock(), session.clone())).collect(); // 收集时间戳与会话
        items.sort_by(|left, right| left.0.partial_cmp(&right.0).unwrap_or(std::cmp::Ordering::Equal)); // 按时间升序
        items.into_iter().map(|(_, session)| session).collect() // 返回会话列表
    }
    fn broadcast(&self, session: &TerminalSession, frame: Value) { // 向所有订阅者广播帧
        let targets = session.subscribers.lock().clone(); // 克隆订阅者列表
        for target in targets { target(frame.clone()); } // 逐个投递
    }
    pub fn subscribe(&self, id: &str, sink: Sink) -> Option<Unsubscriber> { // 订阅会话，返回退订守卫
        let session = self.get(id)?; // 取会话
        session.subscribers.lock().push(sink.clone()); // 登记回调
        *session.detached_at.lock() = 0.0; // 清除脱离计时
        Some(Unsubscriber { manager: manager(), session, sink }) // 返回守卫
    }
    fn unsubscribe(&self, session: &TerminalSession, sink: &Sink) { // 移除订阅
        let mut targets = session.subscribers.lock(); // 加锁列表
        targets.retain(|item| !Arc::ptr_eq(item, sink)); // 按 Arc 指针移除
        if targets.is_empty() { *session.detached_at.lock() = now_seconds(); } // 无订阅者记录脱离时间
    }
    fn kill(&self, session: &TerminalSession) { // 终止会话并清理句柄
        session.closed.store(true, Ordering::SeqCst); // 标记已关闭
        session.alive.store(false, Ordering::SeqCst); // 标记不存活
        if let Some(sender) = session.killer.lock().take() { let _ = sender.send(()); } // 由守护任务调用 Child::kill 终止 Job 内整棵进程树
        session.input.lock().take(); // 关闭 conin，避免控制台宿主继续挂住
        session.control.lock().take(); // 释放 PTY 控制句柄
    }
    fn reapable(&self, session: &TerminalSession, deadline: f64, detached_deadline: f64) -> &'static str { // 判断会话是否可回收及原因
        if !session.run_alive() { return "exited"; } // 已退出
        if session.persistent.load(Ordering::SeqCst) { return ""; } // 保活会话不回收
        let detached = *session.detached_at.lock(); // 脱离时间
        let stamp = if detached > 0.0 { detached } else { *session.created.lock() }; // 取有效时间
        if session.subscriber_count() == 0 && stamp < detached_deadline { return "detached"; } // 无订阅者超时
        if *session.created.lock() < deadline { return "idle"; } // 空闲超时
        "" // 不回收
    }
    fn purge_locked(&self, map: &mut HashMap<String, Arc<TerminalSession>>) -> usize { // 清理可回收会话，返回数量
        let now = now_seconds(); // 当前时间
        let deadline = now - IDLE_LIMIT_SECONDS; // 空闲截止
        let detached_deadline = now - DETACHED_LIMIT_SECONDS; // 脱离截止
        let mut reaped = 0usize; // 回收计数
        let keys: Vec<String> = map.keys().cloned().collect(); // 键列表
        for key in keys { // 逐个检查
            let Some(session) = map.get(&key).cloned() else { continue; }; // 取会话
            let reason = self.reapable(&session, deadline, detached_deadline); // 判断原因
            if reason.is_empty() { continue; } // 不需回收
            if reason != "exited" { self.kill(&session); } // 非自然退出则终止
            map.remove(&key); // 移除会话
            reaped += 1; // 计数
            logger::info("terminal_router", &format!("终端会话回收({reason}): {key}")); // 记录回收
        }
        reaped // 返回回收数
    }
    pub fn purge(&self) -> usize { // 对外暴露清理
        let mut map = self.sessions.lock(); // 加锁
        self.purge_locked(&mut map) // 委托清理
    }
    fn evict(&self, map: &mut HashMap<String, Arc<TerminalSession>>, kind: &str) -> Option<String> { // 驱逐最旧的可回收会话：绝不驱逐 persistent 后台保活会话；先优先无订阅者，再允许驱逐仅有 UI 观察订阅的非保活会话
        let collect = |require_unsubscribed: bool| -> Vec<(bool, f64, String, Arc<TerminalSession>)> {
            map.iter()
                .filter(|(_, session)| {
                    session.kind == kind
                        && !session.persistent.load(Ordering::SeqCst)
                        && (!require_unsubscribed || session.subscriber_count() == 0)
                })
                .map(|(key, session)| {
                    let busy = !session.command.lock().is_empty();
                    (busy, *session.created.lock(), key.clone(), session.clone())
                })
                .collect()
        };
        let mut candidates = collect(true);
        if candidates.is_empty() { candidates = collect(false); } // 兜底：避免 UI WebSocket 旁观订阅导致 4 会话永久死锁
        candidates.sort_by(|left, right| {
            left.0.cmp(&right.0).then_with(|| left.1.partial_cmp(&right.1).unwrap_or(std::cmp::Ordering::Equal))
        });
        let (_, _, key, session) = candidates.into_iter().next()?; // 取最旧且空闲者
        self.kill(&session); // 终止
        map.remove(&key); // 移除
        Some(key) // 返回被驱逐 id
    }
    pub fn open(&self, cols: u16, rows: u16, cwd: Option<&str>, kind: &str, persistent: bool, root: &str) -> Result<Arc<TerminalSession>, String> { // 新建 PTY 会话
        let kind = normalize_kind(kind); // 归一化归属
        let target = resolve_cwd(cwd.unwrap_or_default()); // 解析工作目录
        let limit = if kind == "agent" { MAX_AGENT_SESSIONS } else { MAX_USER_SESSIONS }; // 对应并发上限
        { // 并发控制临界区
            let mut map = self.sessions.lock(); // 加锁
            self.purge_locked(&mut map); // 先清理
            while map.values().filter(|session| session.kind == kind).count() >= limit { // 达上限则驱逐
                match self.evict(&mut map, &kind) { // 驱逐最旧
                    Some(key) => logger::info("terminal_router", &format!("终端会话驱逐({kind}): {key}")), // 记录
                    None => return Err(format!("终端会话已达上限 {limit}，请先关闭部分标签")), // 无可驱逐
                }
            }
        }
        let handle = tokio::runtime::Handle::try_current().map_err(|_| "启动终端失败：异步运行时未就绪".to_string())?; // 取异步运行时
        let pty_size = PtySize::try_new(cols.max(40), rows.max(8)).map_err(|err| format!("启动终端失败：{err}"))?; // 构造 PTY 尺寸
        let mut command = PtyCommand::new("cmd.exe"); // 启动 cmd
        command.current_dir(&target); // 设置工作目录
        let parts = command.spawn_with(SessionOptions::new().size(pty_size)).map_err(|err| format!("启动终端失败：{err}"))?.into_parts(); // 启动 PTY
        let mut child = parts.child; // 子进程句柄
        let reader = parts.output; // 输出半端
        let mut writer = parts.input; // 输入半端
        let controller = parts.controller; // 控制句柄
        let (input_tx, mut input_rx) = mpsc::unbounded_channel::<Vec<u8>>(); // 输入队列
        let (kill_tx, mut kill_rx) = oneshot::channel::<()>(); // 终止信号
        let _ = input_tx.send(b"chcp 65001>nul\r".to_vec()); // 切换到 UTF-8 代码页
        let session = Arc::new(TerminalSession { // 构造会话
            id: Uuid::new_v4().simple().to_string()[..12].to_string(), // 12 位 id
            kind: kind.clone(), // 归属
            root: root.to_string(), // 项目根
            cwd: target.to_string_lossy().to_string(), // 工作目录
            created: Mutex::new(now_seconds()), // 活跃时间
            detached_at: Mutex::new(0.0), // 未脱离
            buffer: Mutex::new(String::new()), // 空缓冲
            subscribers: Mutex::new(Vec::new()), // 无订阅者
            persistent: AtomicBool::new(persistent), // 保活标志
            command: Mutex::new(String::new()), // 空命令
            alive: AtomicBool::new(true), // 存活
            closed: AtomicBool::new(false), // 未关闭
            input: Mutex::new(Some(input_tx)), // 输入发送端
            control: Mutex::new(Some(controller)), // 控制句柄
            killer: Mutex::new(Some(kill_tx)), // 终止发送端
        });
        self.sessions.lock().insert(session.id.clone(), session.clone()); // 登记会话
        let writer_session = session.clone(); // 克隆供写入任务
        handle.spawn(async move { // 写入任务：独占 conin 写半端，任务结束即丢弃写端并结束会话
            while !writer_session.closed.load(Ordering::SeqCst) { // 未关闭则持续
                let Some(chunk) = input_rx.recv().await else { break; }; // 接收输入，通道关闭则退出
                if writer.write_all(&chunk).await.is_err() { break; } // 写入失败退出
            }
        });
        let worker = session.clone(); // 克隆供读取任务
        let reader_manager = manager(); // 管理器引用
        handle.spawn(async move { // 读取任务：conout 单字节流，按 UTF-8 边界解码后广播
            let mut reader = reader; // 读取半端
            let mut chunk = vec![0u8; READ_CHUNK]; // 读取缓冲
            let mut pending: Vec<u8> = Vec::new(); // 未完成字节
            loop { // 持续读取
                match reader.read(&mut chunk).await { // 异步读
                    Ok(0) => break, // EOF
                    Ok(read) => { // 读到数据
                        pending.extend_from_slice(&chunk[..read]); // 追加到待解码
                        let text = decode_chunk(&mut pending); // 按 UTF-8 边界解码
                        if text.is_empty() { continue; } // 无完整字符
                        { // 更新回滚缓冲
                            let mut guard = worker.buffer.lock(); // 加锁
                            guard.push_str(&text); // 追加
                            keep_tail(&mut guard, BUFFER_LIMIT); // 保留尾部
                        }
                        *worker.created.lock() = now_seconds(); // 刷新活跃时间
                        reader_manager.broadcast(&worker, json!({"type": "data", "data": text})); // 广播数据帧
                    }
                    Err(_) => break, // 读取错误
                }
            }
            worker.alive.store(false, Ordering::SeqCst); // 标记不存活
            reader_manager.broadcast(&worker, json!({"type": "exit"})); // 广播退出
            logger::info("terminal_router", &format!("终端会话结束: {}", worker.id)); // 记录结束
        });
        let child_worker = session.clone(); // 克隆供进程树任务
        let child_manager = manager(); // 管理器引用
        handle.spawn(async move { // 进程树任务：持有 Child 保证 Job 存活，根进程退出后回收后代
            let killed = tokio::select! { // 等待自然退出或终止信号
                _ = child.wait() => false, // 自然退出
                _ = &mut kill_rx => true, // 收到终止信号
            };
            if killed { // 需强制终止
                if let Err(err) = child.kill() { logger::debug("terminal_router", &format!("终止终端进程失败: {} {err}", child_worker.id)); } // 终止进程树
                let _ = child.wait().await; // 等待退出
            }
            child_worker.alive.store(false, Ordering::SeqCst); // 标记不存活
            child_manager.broadcast(&child_worker, json!({"type": "exit"})); // 广播退出
        });
        logger::info("terminal_router", &format!("终端已打开: {} kind={kind} persistent={persistent} cwd={}", session.id, session.cwd)); // 记录打开
        Ok(session) // 返回会话
    }
    pub fn write(&self, id: &str, text: &str) -> Result<(), String> { // 向会话写入输入
        let session = self.get(id).ok_or_else(|| format!("终端会话不存在: {id}"))?; // 取会话
        let guard = session.input.lock(); // 加锁输入队列
        let sender = guard.as_ref().ok_or_else(|| format!("终端会话已结束: {id}"))?; // 取发送端
        sender.send(text.as_bytes().to_vec()).map_err(|_| format!("终端会话已结束: {id}"))?; // 发送字节
        *session.created.lock() = now_seconds(); // 刷新活跃时间
        Ok(()) // 完成
    }
    pub fn resize(&self, id: &str, cols: u16, rows: u16) -> Result<(), String> { // 调整 PTY 尺寸
        let session = self.get(id).ok_or_else(|| format!("终端会话不存在: {id}"))?; // 取会话
        let size = PtySize::try_new(cols.max(40), rows.max(8)).map_err(|err| err.to_string())?; // 构造尺寸
        let guard = session.control.lock(); // 加锁控制句柄
        let controller = guard.as_ref().ok_or_else(|| format!("终端会话已结束: {id}"))?; // 取句柄
        controller.resize(size).map_err(|err| err.to_string()) // 执行调整
    }
    pub fn set_persistent(&self, id: &str, enabled: bool, command: &str) -> Option<Value> { // 设置保活标志与命令
        let session = self.get(id)?; // 取会话
        session.persistent.store(enabled, Ordering::SeqCst); // 写入保活标志
        if !command.is_empty() { *session.command.lock() = command.chars().take(300).collect(); } // 记录命令，截断 300 字
        logger::info("terminal_router", &format!("终端会话保活: {id} enabled={enabled}")); // 记录
        Some(json!({"id": id, "persistent": enabled, "command": session.command.lock().clone(), "alive": session.run_alive()})) // 返回状态
    }
    pub fn read_since(&self, id: &str, start: usize) -> Option<(String, usize)> { // 读取 start 之后的增量输出
        let session = self.get(id)?; // 取会话
        Some(read_session(&session, start)) // 委托读取
    }
    pub fn tail(&self, id: &str, limit: usize) -> String { // 读取尾部输出
        let Some(session) = self.get(id) else { return String::new(); }; // 不存在返回空
        let guard = session.buffer.lock(); // 加锁缓冲
        let total = guard.chars().count(); // 总字符数
        let start = total.saturating_sub(limit); // 起始位置
        guard.chars().skip(start).collect() // 取尾部
    }
    pub fn close(&self, id: &str) -> bool { // 关闭会话
        let Some(session) = self.get(id) else { return false; }; // 不存在返回 false
        self.kill(&session); // 终止进程
        self.sessions.lock().remove(id); // 移除会话
        logger::info("terminal_router", &format!("终端已关闭: {id} kind={}", session.kind)); // 记录
        true // 成功
    }
    pub fn close_all(&self) { // 关闭全部会话
        let items: Vec<Arc<TerminalSession>> = self.sessions.lock().drain().map(|(_, session)| session).collect(); // 取出全部
        if items.is_empty() { return; } // 空则跳过
        for session in &items { self.kill(session); } // 逐个终止
        logger::info("terminal_router", &format!("已停止全部终端会话: {} 个", items.len())); // 记录
    }
    pub fn list(&self, kind: &str) -> Vec<Value> { // 列出会话，可按归属过滤
        let now = now_seconds(); // 当前时间
        self.all().into_iter().filter(|session| kind.is_empty() || session.kind == kind).map(|session| { // 过滤并投影
            let idle = now - *session.created.lock(); // 空闲秒数
            json!({"id": session.id, "kind": session.kind, "cwd": session.cwd, "alive": session.run_alive(), "persistent": session.persistent.load(Ordering::SeqCst), "command": session.command.lock().clone(), "subscribers": session.subscriber_count(), "idle_seconds": idle.round() as i64}) // 会话信息
        }).collect()
    }
    pub fn alive_by_id(&self, id: &str) -> bool { // 判断会话是否存活
        self.get(id).map(|session| session.run_alive()).unwrap_or(false) // 不存在视为不存活
    }
}
pub fn read_session(session: &TerminalSession, start: usize) -> (String, usize) { // 读取 start 之后的缓冲并返回总长度
    let guard = session.buffer.lock(); // 加锁缓冲
    let total = guard.chars().count(); // 总字符数
    let begin = start.min(total); // 起始位置钳制
    (guard.chars().skip(begin).collect(), total) // 返回增量与总长
}
fn decode_chunk(pending: &mut Vec<u8>) -> String { // 按 UTF-8 边界解码，剩余字节留待下次
    let mut cut = pending.len(); // 截取长度
    while cut > 0 && std::str::from_utf8(&pending[..cut]).is_err() { cut -= 1; } // 回退到合法边界
    if cut == 0 { return String::new(); } // 无完整字符
    let text = String::from_utf8_lossy(&pending[..cut]).to_string(); // 解码
    pending.drain(..cut); // 消费已解码字节
    text // 返回文本
}
pub fn router() -> Router<AppState> { // 注册终端路由
    Router::new() // 创建路由器
        .route("/api/terminal/ws", get(ws_terminal)) // WebSocket 通道
        .route("/api/terminal/sessions", get(sessions)) // 会话列表
        .route("/api/terminal/open", post(open_terminal)) // 新建会话
        .route("/api/terminal/input", post(input)) // 写入输入
        .route("/api/terminal/resize", post(resize)) // 调整尺寸
        .route("/api/terminal/close", post(close)) // 关闭会话
        .route("/api/terminal/persistent", post(persistent)) // 设置保活
        .route("/api/terminal/stream", get(stream)) // SSE 兼容通道
}
async fn sessions(State(state): State<AppState>, Query(query): Query<HashMap<String, String>>) -> ApiResult { // 列出终端会话
    let kind = query.get("kind").cloned().unwrap_or_default(); // 归属过滤
    ok(json!({"list": state.terminal.list(kind.trim())})) // 返回列表
}
async fn open_terminal(State(state): State<AppState>, body: Bytes) -> ApiResult { // 新建终端会话
    let payload = body_object(&body); // 解析请求体
    let cols = payload.get("cols").and_then(Value::as_i64).unwrap_or(120).clamp(40, 500) as u16; // 列数钳制
    let rows = payload.get("rows").and_then(Value::as_i64).unwrap_or(30).clamp(8, 300) as u16; // 行数钳制
    let kind = normalize_kind(&json_str(payload.get("kind"))); // 归属
    let cwd = json_str(payload.get("cwd")); // 工作目录
    let persistent = as_bool(payload.get("persistent"), false); // 保活标志
    let manager = state.terminal.clone(); // 克隆管理器
    match tokio::task::spawn_blocking(move || manager.open(cols, rows, Some(cwd.as_str()), &kind, persistent, "")).await { // 阻塞操作丢线程池
        Ok(Ok(session)) => ok(json!({"id": session.id, "cwd": session.cwd, "persistent": session.persistent.load(Ordering::SeqCst)})), // 成功
        Ok(Err(err)) => if err.contains("上限") { fail(429, &err) } else { fail(500, &err) }, // 上限返回 429
        Err(err) => fail(500, &format!("启动终端失败：{err}")), // 线程错误
    }
}
async fn input(State(state): State<AppState>, body: Bytes) -> ApiResult { // 向终端写入输入
    let payload = body_object(&body); // 解析请求体
    let id = json_str(payload.get("id")).trim().to_string(); // 会话 id
    let text = json_str(payload.get("data")); // 输入文本
    let manager = state.terminal.clone(); // 克隆管理器
    match tokio::task::spawn_blocking(move || manager.write(&id, &text)).await { // 阻塞操作丢线程池
        Ok(Ok(())) => ok(json!({"id": json_str(payload.get("id"))})), // 成功
        Ok(Err(err)) => fail(404, &err), // 会话不存在
        Err(err) => fail(500, &format!("写入终端失败：{err}")), // 线程错误
    }
}
async fn resize(State(state): State<AppState>, body: Bytes) -> ApiResult { // 调整终端尺寸
    let payload = body_object(&body); // 解析请求体
    let id = json_str(payload.get("id")).trim().to_string(); // 会话 id
    let cols = payload.get("cols").and_then(Value::as_i64).unwrap_or(120).max(40) as u16; // 列数
    let rows = payload.get("rows").and_then(Value::as_i64).unwrap_or(30).max(8) as u16; // 行数
    let manager = state.terminal.clone(); // 克隆管理器
    let target = id.clone(); // 克隆 id
    match tokio::task::spawn_blocking(move || manager.resize(&target, cols, rows)).await { // 阻塞操作丢线程池
        Ok(Ok(())) => ok(json!({"id": id, "cols": cols, "rows": rows})), // 成功
        Ok(Err(err)) => fail(404, &err), // 会话不存在
        Err(err) => fail(500, &format!("调整终端尺寸失败：{err}")), // 线程错误
    }
}
async fn close(State(state): State<AppState>, body: Bytes) -> ApiResult { // 关闭终端会话
    let payload = body_object(&body); // 解析请求体
    let id = json_str(payload.get("id")).trim().to_string(); // 会话 id
    if !state.terminal.exists(&id) { return fail(404, &format!("终端会话不存在: {id}")); } // 不存在
    if !state.terminal.close(&id) { return fail(404, &format!("终端会话不存在: {id}")); } // 关闭失败
    ok(json!({"id": id})) // 成功
}
async fn persistent(State(state): State<AppState>, body: Bytes) -> ApiResult { // 设置会话保活
    let payload = body_object(&body); // 解析请求体
    let id = json_str(payload.get("id")).trim().to_string(); // 会话 id
    let enabled = payload.get("persistent") != Some(&Value::Bool(false)); // 缺省启用
    let command = json_str(payload.get("command")); // 命令描述
    match state.terminal.set_persistent(&id, enabled, &command) { // 设置
        Some(data) => ok(data), // 成功
        None => fail(404, &format!("终端会话不存在: {id}")), // 不存在
    }
}
async fn ws_terminal(State(state): State<AppState>, Query(query): Query<HashMap<String, String>>, ws: WebSocketUpgrade) -> Response { // WebSocket 入口
    let id = query.get("id").cloned().unwrap_or_default().trim().to_string(); // 复用的会话 id
    let kind = normalize_kind(query.get("kind").map(String::as_str).unwrap_or("user")); // 归属
    let manager = state.terminal.clone(); // 克隆管理器
    ws.on_upgrade(move |socket| handle_ws(manager, socket, id, kind)) // 升级为 WebSocket
}
async fn send_text(socket: &mut WebSocket, frame: Value) -> bool { // 发送文本帧
    socket.send(Message::Text(frame.to_string().into())).await.is_ok() // 发送并返回是否成功
}
async fn handle_ws(manager: Arc<TerminalManager>, mut socket: WebSocket, id: String, kind: String) { // 处理 WebSocket 会话
    let reused = !id.is_empty(); // 是否复用现有会话
    let session = if reused { // 复用分支
        match manager.get(&id) { // 查找会话
            Some(session) => session, // 命中
            None => { // 不存在
                let _ = send_text(&mut socket, json!({"type": "error", "message": format!("终端会话不存在: {id}")})).await; // 回报错误
                return; // 退出
            }
        }
    } else { // 新建分支：等待 init 帧
        let frame = match tokio::time::timeout(Duration::from_secs(INIT_TIMEOUT_SECONDS), socket.recv()).await { // 等首帧
            Err(_) => { // 超时
                let _ = send_text(&mut socket, json!({"type": "error", "message": format!("等待 init 帧超时（{INIT_TIMEOUT_SECONDS}s）")})).await; // 回报超时
                return; // 退出
            }
            Ok(None) | Ok(Some(Err(_))) => return, // 连接关闭或错误
            Ok(Some(Ok(Message::Text(text)))) => serde_json::from_str::<Value>(&text).ok(), // 解析文本帧
            Ok(Some(Ok(_))) => None, // 非文本帧
        };
        let Some(init) = frame else { // 无有效 init
            let _ = send_text(&mut socket, json!({"type": "error", "message": "首帧必须是 init"})).await; // 回报错误
            return; // 退出
        };
        if init.get("type").and_then(Value::as_str) != Some("init") { // 类型校验
            let _ = send_text(&mut socket, json!({"type": "error", "message": "首帧必须是 init"})).await; // 回报错误
            return; // 退出
        }
        let cols = init.get("cols").and_then(Value::as_i64).unwrap_or(120).max(40) as u16; // 列数
        let rows = init.get("rows").and_then(Value::as_i64).unwrap_or(30).max(8) as u16; // 行数
        let cwd = json_str(init.get("cwd")); // 工作目录
        let target = manager.clone(); // 克隆管理器
        let worker_kind = kind.clone(); // 克隆归属
        match tokio::task::spawn_blocking(move || target.open(cols, rows, Some(cwd.as_str()), &worker_kind, false, "")).await { // 新建会话
            Ok(Ok(session)) => session, // 成功
            Ok(Err(err)) => { // 新建失败
                let code = if err.contains("上限") { 429 } else { 500 }; // 上限返回 429
                let _ = send_text(&mut socket, json!({"type": "error", "code": code, "message": err})).await; // 回报错误
                return; // 退出
            }
            Err(err) => { // 线程错误
                let _ = send_text(&mut socket, json!({"type": "error", "code": 500, "message": format!("启动终端失败：{err}")})).await; // 回报错误
                return; // 退出
            }
        }
    };
    let (tx, mut rx) = mpsc::unbounded_channel::<Value>(); // 出站帧通道
    let sink: Sink = Arc::new(move |frame| { // 订阅回调
        let _ = tx.send(frame); // 投递到通道
    });
    let Some(_unsubscriber) = manager.subscribe(&session.id, sink) else { // 订阅会话，守卫随函数结束退订
        let _ = send_text(&mut socket, json!({"type": "error", "message": "会话创建后丢失"})).await; // 订阅失败
        return; // 退出
    };
    if !send_text(&mut socket, json!({"type": "open", "id": session.id, "cwd": session.cwd.clone(), "reused": reused})).await { return; } // 发送打开确认
    if reused { // 复用时回放尾部
        let tail = manager.tail(&session.id, REPLAY_LIMIT); // 取尾部
        if !tail.is_empty() && !send_text(&mut socket, json!({"type": "replay", "data": tail})).await { return; } // 回放
    }
    let (mut sender, mut receiver) = socket.split(); // 拆分读写半端
    let mut outbound = tokio::spawn(async move { // 出站任务：会话事件转发到 WebSocket
        while let Some(frame) = rx.recv().await { // 接收事件
            let exit = frame.get("type").and_then(Value::as_str) == Some("exit"); // 是否退出帧
            if sender.send(Message::Text(frame.to_string().into())).await.is_err() { break; } // 发送
            if exit { break; } // 退出则结束
        }
    });
    let inbox_manager = manager.clone(); // 克隆管理器供入站
    let inbox_session = session.clone(); // 克隆会话供入站
    let mut inbound = tokio::spawn(async move { // 入站任务：处理前端发来的帧
        while let Some(message) = receiver.next().await { // 逐帧接收
            let text = match message { // 提取文本
                Ok(Message::Text(text)) => text.to_string(), // 文本帧
                Ok(Message::Close(_)) => break, // 关闭帧
                Ok(_) => continue, // 其他帧忽略
                Err(_) => break, // 错误
            };
            let Ok(frame) = serde_json::from_str::<Value>(&text) else { continue; }; // 解析 JSON
            match frame.get("type").and_then(Value::as_str).unwrap_or_default() { // 按类型分派
                "input" => { // 输入
                    if inbox_manager.write(&inbox_session.id, &json_str(frame.get("data"))).is_err() { break; } // 写入，失败退出
                }
                "resize" => { // 调整尺寸
                    let cols = frame.get("cols").and_then(Value::as_i64).unwrap_or(120).max(40) as u16; // 列数
                    let rows = frame.get("rows").and_then(Value::as_i64).unwrap_or(30).max(8) as u16; // 行数
                    if let Err(err) = inbox_manager.resize(&inbox_session.id, cols, rows) { logger::debug("terminal_router", &format!("终端 resize 失败: {} {err}", inbox_session.id)); } // 失败记日志
                }
                "close" => { // 关闭
                    inbox_manager.close(&inbox_session.id); // 关闭会话
                    break; // 退出
                }
                _ => {} // 未知类型忽略
            }
        }
    });
    tokio::select! { // 任一方向结束则中止另一方向
        _ = &mut outbound => inbound.abort(), // 出站结束中止入站
        _ = &mut inbound => outbound.abort(), // 入站结束中止出站
    }
}
async fn stream(State(state): State<AppState>, Query(query): Query<HashMap<String, String>>) -> Response { // SSE 兼容通道
    let id = query.get("id").cloned().unwrap_or_default().trim().to_string(); // 会话 id
    let manager = state.terminal.clone(); // 克隆管理器
    let (tx, rx) = mpsc::unbounded_channel::<Value>(); // 事件通道
    let sink: Sink = Arc::new(move |frame| { // 订阅回调
        let _ = tx.send(frame); // 投递到通道
    });
    let Some(unsubscriber) = manager.subscribe(&id, sink) else { // 订阅会话
        return fail(404, &format!("终端会话不存在: {id}")).into_response(); // 不存在
    };
    let stream = futures_util::stream::unfold((rx, unsubscriber, manager, id, false), |(mut rx, unsubscriber, manager, id, done)| async move { // 构造事件流
        if done { return None; } // 已结束
        match rx.recv().await { // 等待事件
            Some(frame) => { // 收到事件
                let exit = frame.get("type").and_then(Value::as_str) == Some("exit"); // 是否退出
                Some((Ok::<Event, std::convert::Infallible>(Event::default().data(frame.to_string())), (rx, unsubscriber, manager, id, exit))) // 生成 SSE 事件
            }
            None => { // 通道关闭
                if manager.alive_by_id(&id) { None } else { Some((Ok::<Event, std::convert::Infallible>(Event::default().data("{\"type\": \"exit\"}")), (rx, unsubscriber, manager, id, true))) } // 会话仍存活则结束流，否则补发退出
            }
        }
    });
    Sse::new(stream).into_response() // 返回 SSE 响应
}
pub fn start_reaper() { // 启动后台回收线程
    std::thread::Builder::new().name("terminal-reaper".to_string()).spawn(|| loop { // 独立线程
        std::thread::sleep(Duration::from_secs(60)); // 每分钟
        let reaped = manager().purge(); // 清理过期会话
        if reaped > 0 { logger::info("terminal_router", &format!("终端会话回收完成: {reaped} 个")); } // 记录回收
    }).ok(); // 启动失败忽略
}
#[cfg(test)]
mod tests {
    use super::*;
    fn dummy_session(id: &str, kind: &str, persistent: bool, observed: bool, created: f64) -> Arc<TerminalSession> {
        let mut subscribers: Vec<Sink> = Vec::new();
        if observed { subscribers.push(Arc::new(|_| {})); }
        Arc::new(TerminalSession {
            id: id.to_string(),
            kind: kind.to_string(),
            root: "E:\\proj".to_string(),
            cwd: "E:\\proj".to_string(),
            created: Mutex::new(created),
            detached_at: Mutex::new(0.0),
            buffer: Mutex::new(String::new()),
            subscribers: Mutex::new(subscribers),
            persistent: AtomicBool::new(persistent),
            command: Mutex::new(String::new()),
            alive: AtomicBool::new(true),
            closed: AtomicBool::new(false),
            input: Mutex::new(None),
            control: Mutex::new(None),
            killer: Mutex::new(None),
        })
    }
    #[test]
    fn evict_falls_back_to_observed_non_persistent_but_protects_persistent() {
        let mgr = TerminalManager { sessions: Mutex::new(HashMap::new()) };
        let mut map = mgr.sessions.lock();
        map.insert("bg".to_string(), dummy_session("bg", "agent", true, false, 1.0));
        map.insert("obs-old".to_string(), dummy_session("obs-old", "agent", false, true, 2.0));
        map.insert("obs-new".to_string(), dummy_session("obs-new", "agent", false, true, 3.0));
        assert_eq!(mgr.evict(&mut map, "agent").as_deref(), Some("obs-old"));
        assert_eq!(mgr.evict(&mut map, "agent").as_deref(), Some("obs-new"));
        assert_eq!(mgr.evict(&mut map, "agent"), None); // 后台 persistent 会话不受驱逐
        assert!(map.contains_key("bg"));
    }
}
