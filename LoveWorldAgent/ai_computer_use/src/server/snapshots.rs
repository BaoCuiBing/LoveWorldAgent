use serde_json::{json, Value}; // 窗口 UIA 快照登记表：生成 sXXXXXXXX:N 令牌、支持 query 祖先投影与跨调用失效追踪
use std::collections::{HashMap, HashSet}; // 快照索引映射
use std::sync::atomic::{AtomicU32, Ordering}; // 递增快照序号
use std::sync::Mutex; // 快照全局互斥锁
use std::time::{Duration, Instant}; // UIA 遍历预算
#[derive(Clone, Debug)] // 快照中的单个可交互元素缓存
pub struct CachedElement { // 供动作工具反查的缓存项
    pub index: usize, // snapshot 内 element_index
    pub element_id: i64, // UiaWorker 内部稳定元素 ID
    pub role: String, // UIA role
    pub label: String, // 元素显示标签
    pub value: Option<String>, // 当前值
    pub actions: Vec<String>, // 支持的语义动作
    pub rect: (i32, i32, i32, i32), // 屏幕绝对矩形 (x, y, w, h)
    pub center: (i32, i32), // 屏幕绝对中心坐标
    pub parent_index: Option<usize>, // 父元素索引
    pub depth: usize, // 树深度
    pub is_password: bool, // 是否密码框
    pub hwnd: i64, // 原生句柄或所属窗口句柄
}
#[derive(Clone, Debug)] // 单次窗口快照记录
pub struct WindowSnapshot { // 快照档案
    pub snapshot_id: String, // 形态 s00000001
    pub pid: u32, // 所属进程
    pub hwnd: i64, // 所属窗口句柄
    pub elements: Vec<CachedElement>, // 扁平元素表（index 即 element_index）
    pub invalidated: bool, // 是否已被同窗口更新快照取代
}
#[derive(Default)] // 快照注册表内部状态
struct StoreState { // 注册表状态
    by_id: HashMap<String, WindowSnapshot>, // snapshot_id → 快照
    latest_by_window: HashMap<(u32, i64), String>, // (pid, hwnd) → 最新 snapshot_id
    order: Vec<String>, // 淘汰顺序
}
static SNAP_SEQ: AtomicU32 = AtomicU32::new(1); // 快照序列号
static STORE: std::sync::OnceLock<Mutex<StoreState>> = std::sync::OnceLock::new(); // 全局快照仓储
fn store() -> &'static Mutex<StoreState> { STORE.get_or_init(|| Mutex::new(StoreState::default())) } // 获取仓储锁
pub fn register_snapshot(pid: u32, hwnd: i64, elements: Vec<CachedElement>) -> (String, Vec<String>) { // 登记新快照并返回 (snapshot_id, invalidated_ids)
    let id_num = SNAP_SEQ.fetch_add(1, Ordering::Relaxed); // 递增序号
    let snapshot_id = format!("s{id_num:08x}"); // 8 位十六进制标识
    let mut state = store().lock().expect("snapshot store poisoned"); // 加锁
    let mut invalidated_ids = Vec::new(); // 被替换的旧快照
    if let Some(prev_id) = state.latest_by_window.insert((pid, hwnd), snapshot_id.clone()) { // 替换同窗口旧快照
        if let Some(prev) = state.by_id.get_mut(&prev_id) { prev.invalidated = true; } // 标为失效
        invalidated_ids.push(prev_id); // 记录替换列表
    }
    state.by_id.insert(snapshot_id.clone(), WindowSnapshot { snapshot_id: snapshot_id.clone(), pid, hwnd, elements, invalidated: false }); // 写入新快照
    state.order.push(snapshot_id.clone()); // 记录顺序
    while state.order.len() > 64 { // 保留最近 64 份快照防内存膨胀
        let oldest = state.order.remove(0); // 移除最旧
        state.by_id.remove(&oldest); // 清理正文
    }
    (snapshot_id, invalidated_ids) // 返回 ID 与失效列表
}
#[derive(Clone, Debug)] // 解析后的令牌目标
pub struct ResolvedToken { // 令牌解析结果
    pub snapshot_id: String, // 快照 ID
    pub pid: u32, // 进程 ID
    pub hwnd: i64, // 窗口 HWND
    pub element: CachedElement, // 缓存元素
}
pub fn parse_token(token: &str) -> Result<(String, usize), String> { // 解析 sXXXXXXXX:N 形式令牌
    let trimmed = token.trim(); // 去空白
    let Some((snap, idx_str)) = trimmed.split_once(':') else { return Err(format!("element_token 格式非法（应为 <snapshot_id>:<index>）：{trimmed}")); }; // 拆冒号
    let idx: usize = idx_str.parse().map_err(|_| format!("element_token 下标非法：{trimmed}"))?; // 解析下标
    if snap.is_empty() { return Err("element_token 缺少 snapshot_id".to_string()); } // 空 ID 拒绝
    Ok((snap.to_string(), idx)) // 返回拆分结果
}
pub fn resolve_token(token: &str, expected_pid: Option<u32>) -> Result<ResolvedToken, String> { // 根据 element_token 反查有效快照元素
    let (snap_id, idx) = parse_token(token)?; // 解析令牌
    let state = store().lock().expect("snapshot store poisoned"); // 加锁读取
    let snap = state.by_id.get(&snap_id).ok_or_else(|| format!("未找到快照 {snap_id}（可能已过期，请重新调用 computer_get_window_state）"))?; // 查找快照
    if snap.invalidated { return Err(format!("element_token {token} 已失效：窗口已有更新的快照，请重新调用 computer_get_window_state")); } // 已被同窗新快照替代
    if let Some(pid) = expected_pid { if snap.pid != pid { return Err(format!("element_token {token} 属于 pid {}，与传入 pid {pid} 不一致", snap.pid)); } } // 校验 pid
    let elem = snap.elements.get(idx).cloned().ok_or_else(|| format!("element_token {token} 元素索引 {idx} 越界"))?; // 读取元素
    Ok(ResolvedToken { snapshot_id: snap_id, pid: snap.pid, hwnd: snap.hwnd, element: elem }) // 返回解析目标
}
fn collect_nodes(worker: &mut crate::uia::worker::UiaWorker, node: &crate::uia::snapshot::Node, parent_idx: Option<usize>, depth: usize, win_hwnd: i64, out: &mut Vec<CachedElement>) { // 将递归 Node 树扁平化并补全屏幕矩形
    let idx = out.len(); // 分配顺序索引
    let info = worker.get(node.id).ok().map(|el| crate::uia::element::element_info(node.id, &el)); // 补读矩形与密码属性
    let rect = info.as_ref().map(|i| (i.rect.x, i.rect.y, i.rect.width, i.rect.height)).unwrap_or((0, 0, 0, 0)); // 矩形
    let center = (rect.0 + rect.2 / 2, rect.1 + rect.3 / 2); // 中心点
    let is_password = node.traits.iter().any(|t| t == "password"); // 密码标记
    let value = if is_password { None } else { node.value.clone() }; // 密码字段永不回显明文
    let label = if !node.name.is_empty() { node.name.clone() } else { value.clone().unwrap_or_default() }; // 显示标签
    out.push(CachedElement { index: idx, element_id: node.id, role: node.role.clone(), label, value, actions: node.actions.clone(), rect, center, parent_index: parent_idx, depth, is_password, hwnd: info.map(|i| i.native_window_handle).filter(|h| *h != 0).unwrap_or(win_hwnd) }); // 写入元素
    for child in &node.children { collect_nodes(worker, child, Some(idx), depth + 1, win_hwnd, out); } // 递归子节点
}
pub struct WalkedWindowTree { // UIA 遍历产物
    pub total_count: usize, // 遍历节点总数
    pub elements: Vec<CachedElement>, // 扁平元素表
    pub raw_markdown: String, // 原始树文本
    pub truncated: bool, // 是否触及上限截断
}
pub fn walk_window(worker: &mut crate::uia::worker::UiaWorker, hwnd: i64, max_depth: usize, max_elements: usize, timeout_ms: u64) -> Result<WalkedWindowTree, String> { // 在指定预算内抓取窗口 UIA 树并扁平化
    let root_el = worker.element_from_handle(hwnd).map_err(|e| format!("获取窗口 UIA 根元素失败：{e}"))?; // 窗口根元素
    let deadline = Instant::now() + Duration::from_millis(timeout_ms.clamp(500, 20_000)); // 截止时间
    let snap = crate::uia::snapshot::snapshot(worker, &root_el, max_depth.clamp(1, 30), max_elements.clamp(1, 5000), deadline).map_err(|e| format!("遍历 UIA 树失败：{e}"))?; // 生成快照
    let mut flat = Vec::with_capacity(snap.nodes.max(1)); // 扁平容器
    collect_nodes(worker, &snap.root, None, 0, hwnd, &mut flat); // 扁平化填充矩形
    let truncated = flat.len() >= max_elements || snap.text.contains("截断"); // 截断判定
    Ok(WalkedWindowTree { total_count: flat.len(), elements: flat, raw_markdown: snap.text, truncated }) // 返回遍历结果
}
pub fn project_elements(elements: &[CachedElement], query: Option<&str>) -> (Vec<usize>, String) { // 按 query（支持 a|b）过滤并保留祖先链，保持原始索引
    let Some(raw_q) = query.map(str::trim).filter(|q| !q.is_empty()) else { // 无过滤则全量
        let indices: Vec<usize> = (0..elements.len()).collect(); // 全部下标
        return (indices, String::new()); // 由调用方拼装
    };
    let needles: Vec<String> = raw_q.split('|').map(|s| s.trim().to_ascii_lowercase()).filter(|s| !s.is_empty()).collect(); // 拆分关键词
    let mut keep: HashSet<usize> = HashSet::new(); // 保留索引集
    for el in elements { // 检查匹配项
        let hay = format!("{} {} {}", el.role, el.label, el.value.as_deref().unwrap_or("")).to_ascii_lowercase(); // 拼接检索文本
        if needles.iter().any(|n| hay.contains(n)) { // 命中任一关键词
            let mut cur = Some(el.index); // 向上追溯祖先链
            while let Some(id) = cur { // 逐级纳入祖先
                if !keep.insert(id) { break; } // 已存在则停止
                cur = elements.get(id).and_then(|n| n.parent_index); // 取父下标
            }
        }
    }
    let mut indices: Vec<usize> = keep.into_iter().collect(); // 收集下标
    indices.sort_unstable(); // 恢复文档顺序
    let hint = if indices.is_empty() { format!("0 of {} elements match query {:?}", elements.len(), raw_q) } else { String::new() }; // 空匹配提示
    (indices, hint) // 返回投影结果
}
pub fn format_markdown_and_json(snapshot_id: &str, elements: &[CachedElement], indices: &[usize], origin: (i32, i32)) -> (String, Vec<Value>) { // 同时生成紧凑 Markdown 行与结构化 elements 数组
    let mut md_lines = Vec::with_capacity(indices.len()); // Markdown 行集合
    let mut json_rows = Vec::with_capacity(indices.len()); // 结构化数组
    for &idx in indices { // 逐个输出保留元素
        let el = &elements[idx]; // 取出元素
        let wx = el.rect.0 - origin.0; let wy = el.rect.1 - origin.1; // 窗口局部坐标
        let cx = el.center.0 - origin.0; let cy = el.center.1 - origin.1; // 窗口局部中心
        let indent = "  ".repeat(el.depth.min(12)); // 树缩进
        let token = format!("{snapshot_id}:{}", el.index); // 令牌格式 sXXXXXXXX:N
        let mut line = format!("{indent}[{}] {} {:?}", el.index, el.role, el.label); // 角色与标签
        if let Some(val) = &el.value { if !val.is_empty() && val != &el.label { line.push_str(&format!(" val={val:?}")); } } // 附加值
        if el.is_password { line.push_str(" password"); } // 密码提示
        if !el.actions.is_empty() { line.push_str(&format!(" [{}]", el.actions.join(","))); } // 语义动作
        line.push_str(&format!(" @({cx},{cy})")); // 窗口局部中心
        md_lines.push(line); // 加入 Markdown
        let mut obj = json!({"element_index": el.index, "element_token": token, "role": el.role, "depth": el.depth, "label": el.label, "frame": {"x": wx, "y": wy, "w": el.rect.2, "h": el.rect.3}, "screen_center": {"x": el.center.0, "y": el.center.1}}); // 构建结构化项
        if let Some(val) = &el.value { if !val.is_empty() { obj["value"] = json!(val); } } // 附加值
        if !el.actions.is_empty() { obj["actions"] = json!(el.actions); } // 动作列表
        if el.is_password { obj["is_password"] = json!(true); } // 密码标记
        if let Some(p) = el.parent_index { obj["parent_index"] = json!(p); } // 父下标
        json_rows.push(obj); // 写入数组
    }
    (md_lines.join("\n"), json_rows) // 返回正文与数组
}