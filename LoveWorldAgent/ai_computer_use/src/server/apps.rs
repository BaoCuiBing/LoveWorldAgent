use serde_json::{json, Map, Value}; // 应用与窗口管理、几何、进程控制及桌面信息探针工具
use std::collections::{HashMap, HashSet}; // 去重与映射
use std::os::windows::process::CommandExt; // CREATE_NO_WINDOW / DETACHED_PROCESS
use std::process::Command; // 启动外部应用与查询开始菜单应用
use std::sync::OnceLock; // 安装应用索引缓存
use windows::Win32::Foundation::CloseHandle; // 内核句柄释放
use windows::Win32::System::Threading::{OpenProcess, TerminateProcess, PROCESS_TERMINATE}; // 强杀进程
use windows::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, IsIconic, IsWindow, IsZoomed, SetWindowPos, ShowWindowAsync, HWND_TOP, SWP_NOACTIVATE, SWP_NOZORDER, SW_SHOWMINNOACTIVE}; // 窗口定位与激活
use crate::sys::screen; // 屏幕指标与 DWM 边框
use crate::uia::{desktop, element, input, worker::UiaHandle}; // UIA 窗口/元素工具
const CREATE_NO_WINDOW: u32 = 0x0800_0000; // 隐藏 PowerShell 辅助窗口
const DETACHED_PROCESS: u32 = 0x0000_0008; // 独立进程启动，不共享控制台
#[derive(Clone)] // 已安装开始菜单应用条目
struct StartApp { name: String, app_id: String, kind: String, launch_path: String } // 名称、AppID、类型与启动串
static START_APPS: OnceLock<Vec<StartApp>> = OnceLock::new(); // 启动清单缓存
fn query_start_apps() -> Vec<StartApp> { // 枚举开始菜单应用与 UWP AUMID（带 3 秒超时容错）
    let child = Command::new("powershell.exe").creation_flags(CREATE_NO_WINDOW).args(["-NoProfile", "-Command", "[Console]::OutputEncoding=[System.Text.Encoding]::UTF8; Get-StartApps | Select-Object Name,AppID | ConvertTo-Json -Compress"]).stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::null()).spawn(); // 启动轻量查询
    let Ok(out) = child.and_then(|c| c.wait_with_output()) else { return Vec::new(); }; // 失败返回空表
    let text = String::from_utf8_lossy(&out.stdout); // 解析 UTF-8 JSON
    let Ok(val) = serde_json::from_str::<Value>(text.trim()) else { return Vec::new(); }; // 解析 JSON
    let items = match val { Value::Array(arr) => arr, Value::Object(_) => vec![val], _ => Vec::new() }; // 兼容单元素对象
    let mut out_apps = Vec::new(); // 结果集
    for item in items { // 遍历条目
        let name = item.get("Name").and_then(Value::as_str).unwrap_or_default().trim().to_string(); // 显示名
        let app_id = item.get("AppID").and_then(Value::as_str).unwrap_or_default().trim().to_string(); // AppID 或路径
        if name.is_empty() || app_id.is_empty() { continue; } // 跳过空项
        let is_uwp = app_id.contains('!'); // 含 ! 判定为 MSIX/UWP AUMID
        let kind = if is_uwp { "uwp".to_string() } else { "desktop".to_string() }; // 分类
        let launch_path = if is_uwp { format!("shell:appsFolder\\{app_id}") } else { app_id.clone() }; // 启动串
        out_apps.push(StartApp { name, app_id, kind, launch_path }); // 收入列表
    }
    out_apps // 返回结果
}
fn cached_start_apps() -> &'static [StartApp] { START_APPS.get_or_init(query_start_apps) } // 惰性缓存开始菜单应用
pub fn resolve_pid_hwnd(pid: Option<u32>, hwnd_arg: Option<i64>) -> Result<(u32, i64), String> { // 按传入 pid/window_id 校验并解析归属窗口
    if let Some(h) = hwnd_arg { // 显式传了 window_id
        if h <= 0 || !unsafe { IsWindow(Some(desktop::hwnd_of(h))).as_bool() } { return Err(format!("window_id {h} 无效或已关闭")); } // 句柄校验
        let owner = desktop::pid_of_window(h); // 反查真实属主 pid
        if let Some(p) = pid { if owner != p { return Err(format!("window_id {h} 属于 pid {owner}，不属于 pid {p}")); } } // 严禁跨进程错绑
        return Ok((owner, h)); // 返回校验后的 (pid, hwnd)
    }
    let Some(p) = pid else { return Err("缺少 pid 或 window_id 参数".to_string()); }; // 二者不能全空
    let win = desktop::main_window_of(p).ok_or_else(|| format!("未找到 pid {p} 的可见窗口，请先用 computer_list_windows 确认或调用 computer_launch_app"))?; // 取主窗口
    Ok((p, win.hwnd)) // 返回主窗口
}
fn force_foreground_win(hwnd: i64) -> Result<(i64, i64), String> { // 恢复最小化窗并通过 Alt 焦点解锁将窗口置前
    let target = desktop::hwnd_of(hwnd); // 转 HWND
    if !unsafe { IsWindow(Some(target)).as_bool() } { return Err(format!("window_id {hwnd} 无效或已关闭")); } // 校验有效
    let prev = unsafe { GetForegroundWindow() }.0 as isize as i64; // 记录旧前台窗
    if !desktop::activate(hwnd) { return Err(format!("激活窗口 window_id {hwnd} 失败")); } // 调用复用激活逻辑
    std::thread::sleep(std::time::Duration::from_millis(80)); // 略等状态稳定
    let now_fg = unsafe { GetForegroundWindow() }.0 as isize as i64; // 读取当前前台窗
    Ok((prev, now_fg)) // 返回前后句柄
}
fn window_json(w: &desktop::WindowInfo, z_index: usize) -> Value { // 统一构造单条窗口对象
    json!({"window_id": w.hwnd, "pid": w.process_id, "app_name": w.process_name, "title": w.title, "bounds": {"x": w.x, "y": w.y, "width": w.width, "height": w.height}, "layer": 0, "z_index": z_index, "is_on_screen": w.visible, "minimized": desktop::is_minimized(w.hwnd)}) // 对齐参考结构
}
pub fn list_apps(args: &Map<String, Value>) -> Value { // computer_list_apps：合并运行中 GUI 应用与开始菜单已安装应用
    if !args.is_empty() { return json!({"ok": false, "error": "computer_list_apps 不接受未定义的额外参数"}); } // 严格校验 additionalProperties=false
    let wins = desktop::list_windows(&desktop::WindowFilter { only_visible: true, ..Default::default() }); // 全部可见顶层窗
    let self_pid = std::process::id(); // 跳过自身 sidecar
    let fg_pid = desktop::pid_of_window(unsafe { GetForegroundWindow() }.0 as isize as i64); // 当前前台进程
    let mut running_by_pid: HashMap<u32, String> = HashMap::new(); // pid -> exe
    for w in &wins { if w.process_id != self_pid && !w.process_name.is_empty() { running_by_pid.entry(w.process_id).or_insert_with(|| w.process_name.clone()); } } // 汇总运行进程
    let mut apps: Vec<Value> = Vec::new(); // 结果列表
    let mut matched_pids: HashSet<u32> = HashSet::new(); // 已折叠到已安装项的运行进程
    for sa in cached_start_apps() { // 先匹配已安装应用
        let base = sa.launch_path.rsplit(['\\', '/']).next().unwrap_or(&sa.name).trim_end_matches(".exe").to_ascii_lowercase(); // 推算 exe 基名
        let hit_pid = running_by_pid.iter().find(|(_, pname)| { let pn = pname.trim_end_matches(".exe").to_ascii_lowercase(); !pn.is_empty() && (pn == base || pn.eq_ignore_ascii_case(&sa.name)) }).map(|(p, _)| *p); // 查找同进程
        if let Some(p) = hit_pid { matched_pids.insert(p); } // 标记已折叠
        apps.push(json!({"name": sa.name, "pid": hit_pid.unwrap_or(0), "running": hit_pid.is_some(), "active": hit_pid == Some(fg_pid), "kind": sa.kind, "launch_path": sa.launch_path, "aumid": if sa.kind == "uwp" { Value::String(sa.app_id.clone()) } else { Value::Null }})); // 插入条目
    }
    for (pid, pname) in running_by_pid { // 追加未在开始菜单登记的运行中进程
        if matched_pids.contains(&pid) { continue; } // 已匹配跳过
        apps.push(json!({"name": pname, "pid": pid, "running": true, "active": pid == fg_pid, "kind": "desktop", "launch_path": Value::Null})); // 运行中无启动串
    }
    json!({"ok": true, "data": {"count": apps.len(), "apps": apps}}) // 返回结果
}
pub fn list_windows(args: &Map<String, Value>) -> Value { // computer_list_windows：枚举顶层窗口，支持 pid/app/on_screen_only 过滤
    let filter_pid = args.get("pid").and_then(Value::as_u64).map(|v| v as u32); // 可选 pid 过滤
    let app_filter = args.get("app").and_then(Value::as_str).map(|s| s.trim().to_ascii_lowercase()).filter(|s| !s.is_empty()); // 可选应用名过滤
    let on_screen_only = args.get("on_screen_only").and_then(Value::as_bool).unwrap_or(false); // 仅在屏窗口
    let self_pid = std::process::id(); // 自身 pid
    let all = desktop::list_windows(&desktop::WindowFilter { only_visible: on_screen_only, ..Default::default() }); // 枚举顶层窗（按 Z 序）
    let total = all.len(); // 总数用于计算自前向后权重
    let mut rows = Vec::new(); // 过滤后列表
    for (idx, w) in all.iter().enumerate() { // 逐窗检查
        if w.process_id == self_pid { continue; } // 过滤 sidecar 自身覆盖层
        if let Some(fp) = filter_pid { if w.process_id != fp { continue; } } // pid 过滤
        if let Some(ref f) = app_filter { if !w.process_name.to_ascii_lowercase().contains(f) && !w.title.to_ascii_lowercase().contains(f) { continue; } } // app 过滤
        rows.push(window_json(w, total.saturating_sub(idx))); // 较大 z_index 表示越靠前
    }
    let mut data = json!({"count": rows.len(), "windows": rows}); // 基础返回体
    if let Some(fp) = filter_pid { // 若指定 pid 无窗口给出当前前台提示
        if rows.is_empty() { let fg_pid = desktop::pid_of_window(unsafe { GetForegroundWindow() }.0 as isize as i64); data["warning"] = json!(format!("未找到 pid {fp} 的窗口；当前前台进程为 \"{}\" (pid {fg_pid})", desktop::process_name_of(fg_pid))); } // 告警提示
    }
    json!({"ok": true, "data": data}) // 返回响应
}
pub fn launch_app(args: &Map<String, Value>) -> Value { // computer_launch_app：按 launch_path/path/aumid/bundle_id/name/urls 启动应用并尽量不抢焦点
    let prev_fg = unsafe { GetForegroundWindow() }; // 记录原有前台窗口以便恢复
    let launch_path = args.get("launch_path").and_then(Value::as_str).map(str::trim).filter(|s| !s.is_empty()); // 最高优先级
    let path = args.get("path").and_then(Value::as_str).map(str::trim).filter(|s| !s.is_empty()); // 可执行路径
    let aumid = args.get("aumid").and_then(Value::as_str).map(str::trim).filter(|s| !s.is_empty()); // 显式 UWP AUMID
    let bundle_id = args.get("bundle_id").and_then(Value::as_str).map(str::trim).filter(|s| !s.is_empty()); // macOS/UWP 兼容字段
    let name = args.get("name").and_then(Value::as_str).map(str::trim).filter(|s| !s.is_empty()); // 应用名称
    let urls: Vec<String> = args.get("urls").and_then(Value::as_array).map(|arr| arr.iter().filter_map(Value::as_str).map(str::to_string).collect()).unwrap_or_default(); // 待打开网址
    let extra: Vec<String> = args.get("additional_arguments").and_then(Value::as_array).map(|arr| arr.iter().filter_map(Value::as_str).map(str::to_string).collect()).unwrap_or_default(); // 额外启动参数
    let start_minimized = args.get("start_minimized").and_then(Value::as_bool).unwrap_or(false); // 是否最小化启动
    let target = launch_path.or(path).or(aumid).or(bundle_id).or(name).map(|s| s.to_string()).or_else(|| urls.first().cloned()); // 决定目标
    let Some(target_raw) = target else { return json!({"ok": false, "error": "launch_app 需要提供 launch_path / path / aumid / bundle_id / name / urls 之一"}); }; // 校验必填
    let urls_only = launch_path.is_none() && path.is_none() && aumid.is_none() && bundle_id.is_none() && name.is_none() && !urls.is_empty(); // 仅打开网址时不强切回原前台
    let mut resolved_cmd = target_raw.clone(); // 最终命令或壳目标
    let mut resolved_uwp = None::<String>; // 使用的 AUMID
    if resolved_cmd.starts_with("shell:appsFolder\\") || (resolved_cmd.contains('!') && path.is_none() && launch_path.is_none()) { // UWP 形态
        let id = resolved_cmd.trim_start_matches("shell:appsFolder\\").to_string(); // 提取 AUMID
        resolved_uwp = Some(id.clone()); // 记录 bundle_id
        resolved_cmd = format!("shell:appsFolder\\{id}"); // 完整壳路径
    } else if launch_path.is_none() && path.is_none() && !urls_only { // 按 name 从已安装表反查精确启动串
        let q = resolved_cmd.to_ascii_lowercase(); // 小写关键词
        if let Some(hit) = cached_start_apps().iter().find(|a| a.name.eq_ignore_ascii_case(&target_raw)).or_else(|| cached_start_apps().iter().find(|a| a.name.to_ascii_lowercase().contains(&q))) {
            resolved_cmd = hit.launch_path.clone(); // 改用命中启动串
            if hit.kind == "uwp" { resolved_uwp = Some(hit.app_id.clone()); } // 记录 AUMID
        }
    }
    let mut spawned_pid = 0u32; // 新进程 pid（若直接 spawn 可获得）
    let spawn_res = if resolved_cmd.starts_with("shell:appsFolder\\") || resolved_cmd.starts_with("http://") || resolved_cmd.starts_with("https://") {
        Command::new("explorer.exe").creation_flags(DETACHED_PROCESS).arg(&resolved_cmd).spawn() // UWP 或 URL 交由 explorer 打开
    } else if std::path::Path::new(&resolved_cmd).is_file() {
        let mut cmd = Command::new(&resolved_cmd); // 直接启动可执行文件
        cmd.creation_flags(DETACHED_PROCESS).args(&extra); // 传附加参数
        cmd.spawn() // 启动
    } else {
        let mut cmd = Command::new("cmd.exe"); // 经 cmd /c start 按 PATH 启动（允许带工作目录或隐藏焦点）
        let flag = if start_minimized { "/min" } else { "" }; // 最小化标志
        let args_line = if extra.is_empty() { String::new() } else { format!(" {}", extra.join(" ")) }; // 拼参数
        cmd.creation_flags(CREATE_NO_WINDOW).args(["/c", &format!("start \"\" {flag} \"{resolved_cmd}\"{args_line}")]); // 启动命令
        cmd.spawn() // 执行
    };
    if let Err(err) = spawn_res.map(|c| { spawned_pid = c.id(); }) { return json!({"ok": false, "error": format!("启动应用失败：{err}")}); } // 启动错误返回
    if !urls_only && urls.len() > 1 { for u in urls { let _ = Command::new("explorer.exe").creation_flags(DETACHED_PROCESS).arg(&u).spawn(); } } // 其余 URL 一并打开
    let deadline = std::time::Instant::now() + std::time::Duration::from_millis(2500); // 最多等 2.5s 让主窗出现
    let needle = target_raw.rsplit(['\\', '/']).next().unwrap_or(&target_raw).trim_end_matches(".exe").to_ascii_lowercase(); // 窗口匹配关键字
    let mut matched_wins: Vec<desktop::WindowInfo> = Vec::new(); // 捕获到的窗口
    while std::time::Instant::now() < deadline { // 轮询新窗口
        std::thread::sleep(std::time::Duration::from_millis(200)); // 轮询步长
        let wins = desktop::list_windows(&desktop::WindowFilter { only_visible: true, ..Default::default() }); // 扫当前窗口
        let hits: Vec<desktop::WindowInfo> = wins.into_iter().filter(|w| (spawned_pid != 0 && w.process_id == spawned_pid) || (!needle.is_empty() && (w.process_name.to_ascii_lowercase().contains(&needle) || w.title.to_ascii_lowercase().contains(&needle)))).collect(); // 匹配 pid 或应用关键字
        if !hits.is_empty() { matched_wins = hits; break; } // 找到即停
    }
    if start_minimized { for w in &matched_wins { unsafe { let _ = ShowWindowAsync(desktop::hwnd_of(w.hwnd), SW_SHOWMINNOACTIVE); } } } // 若要求最小化启动
    if !urls_only && !prev_fg.0.is_null() { let _ = force_foreground_win(prev_fg.0 as isize as i64); } // 遵守后台契约：把用户原前台窗还原
    let final_pid = matched_wins.first().map(|w| w.process_id).unwrap_or(spawned_pid); // 解析最终 pid
    let final_name = matched_wins.first().map(|w| w.process_name.clone()).filter(|s| !s.is_empty()).unwrap_or(target_raw); // 进程名
    let win_list: Vec<Value> = matched_wins.iter().enumerate().map(|(i, w)| window_json(w, matched_wins.len() - i)).collect(); // 构造 windows 数组
    json!({"ok": true, "data": {"pid": final_pid, "name": final_name, "active": false, "bundle_id": resolved_uwp, "windows": win_list}}) // 返回结构化启动结果
}
pub fn kill_app(args: &Map<String, Value>) -> Value { // computer_kill_app：按 pid 强制终止进程
    let Some(pid) = args.get("pid").and_then(Value::as_u64).filter(|p| *p > 0 && *p <= u32::MAX as u64).map(|p| p as u32) else { return json!({"ok": false, "error": "kill_app 缺少合法正整数 pid"}); }; // 校验 pid
    if pid == std::process::id() { return json!({"ok": false, "error": "禁止终止 sidecar 自身进程"}); } // 安全自保
    unsafe { // 调用 OpenProcess + TerminateProcess
        let Ok(handle) = OpenProcess(PROCESS_TERMINATE, false, pid) else { return json!({"ok": false, "error": format!("无法打开进程 pid={pid}（进程可能已退出或权限不足）")}); }; // 打开句柄
        let terminated = TerminateProcess(handle, 1).is_ok(); // 强制终止
        let _ = CloseHandle(handle); // 立即关闭句柄
        if !terminated { return json!({"ok": false, "error": format!("终止进程 pid={pid} 失败")}); } // 返回失败
    }
    json!({"ok": true, "data": {"pid": pid, "terminated": true}}) // 返回终止回执
}
pub fn bring_to_front(args: &Map<String, Value>) -> Value { // computer_bring_to_front：恢复并激活指定进程窗口
    let pid = args.get("pid").and_then(Value::as_u64).map(|p| p as u32); // 读取 pid
    let wid = args.get("window_id").and_then(Value::as_i64); // 可选 window_id
    let (resolved_pid, hwnd) = match resolve_pid_hwnd(pid, wid) { Ok(v) => v, Err(err) => return json!({"ok": false, "error": err}) }; // 校验解析目标窗
    match force_foreground_win(hwnd) { // 置前并记录前后句柄与命中状态
        Ok((previous_fg_hwnd, now_fg_hwnd)) => {
            let landed = now_fg_hwnd == hwnd || (now_fg_hwnd != 0 && desktop::pid_of_window(now_fg_hwnd) == resolved_pid); // 判断前台是否已落到目标窗口/进程
            json!({"ok": true, "data": {"pid": resolved_pid, "window_id": hwnd, "previous_fg_hwnd": previous_fg_hwnd, "now_fg_hwnd": now_fg_hwnd, "landed_on_target": landed}}) // 返回置前结果
        }
        Err(err) => json!({"ok": false, "error": err}), // 失败
    }
}
pub fn set_window_frame(args: &Map<String, Value>) -> Value { // computer_set_window_frame：移动并调整目标窗口可见边界
    let Some(pid) = args.get("pid").and_then(Value::as_u64).map(|v| v as u32) else { return json!({"ok": false, "error": "set_window_frame 缺少 pid"}); }; // 必填 pid
    let Some(hwnd) = args.get("window_id").and_then(Value::as_i64) else { return json!({"ok": false, "error": "set_window_frame 缺少 window_id"}); }; // 必填 HWND
    let (Some(x), Some(y), Some(w), Some(h)) = (args.get("x").and_then(Value::as_f64), args.get("y").and_then(Value::as_f64), args.get("width").and_then(Value::as_f64), args.get("height").and_then(Value::as_f64)) else { return json!({"ok": false, "error": "set_window_frame 必须提供数值型 x/y/width/height"}); }; // 必填矩形
    if !x.is_finite() || !y.is_finite() || !w.is_finite() || !h.is_finite() || w <= 0.0 || h <= 0.0 { return json!({"ok": false, "error": "x/y 需为有限数值且 width/height 必须大于 0"}); } // 几何合法性
    if let Err(err) = resolve_pid_hwnd(Some(pid), Some(hwnd)) { return json!({"ok": false, "error": err}); } // 校验属主
    let target = desktop::hwnd_of(hwnd); // 转 HWND
    if unsafe { IsIconic(target) }.as_bool() { return json!({"ok": false, "error": format!("window_id {hwnd} 当前处于最小化状态，请先调用 computer_bring_to_front 恢复后再调尺寸")}); } // 最小化拒绝
    if unsafe { IsZoomed(target) }.as_bool() { return json!({"ok": false, "error": format!("window_id {hwnd} 当前处于最大化状态，无法直接修改边框")}); } // 最大化拒绝
    let outer = screen::window_bounds_outer(hwnd).unwrap_or((x.round() as i32, y.round() as i32, w.round() as i32, h.round() as i32)); // 当前外框
    let visible = screen::window_bounds_visible(hwnd).unwrap_or(outer); // 当前可见框
    let dx_left = visible.0 - outer.0; let dy_top = visible.1 - outer.1; // DWM 阴影左边距/顶边距
    let dw_extra = outer.2 - visible.2; let dh_extra = outer.3 - visible.3; // 宽高阴影补偿
    let req_x = x.round() as i32 - dx_left; let req_y = y.round() as i32 - dy_top; // 反推所需外框原点
    let req_w = (w.round() as i32 + dw_extra).max(1); let req_h = (h.round() as i32 + dh_extra).max(1); // 反推所需外框尺寸
    let moved = unsafe { SetWindowPos(target, Some(HWND_TOP), req_x, req_y, req_w, req_h, SWP_NOACTIVATE | SWP_NOZORDER) }; // 无焦点定位与缩放
    if let Err(err) = moved { return json!({"ok": false, "error": format!("SetWindowPos 失败：{err}")}); } // 失败返回
    let actual = screen::window_bounds_visible(hwnd).unwrap_or((x.round() as i32, y.round() as i32, w.round() as i32, h.round() as i32)); // 回读可见边界
    json!({"ok": true, "data": {"pid": pid, "window_id": hwnd, "bounds": {"x": actual.0, "y": actual.1, "width": actual.2, "height": actual.3}}}) // 返回实测矩形
}
pub fn get_accessibility_tree() -> Value { // computer_get_accessibility_tree：快速返回桌面进程与可见窗口列表
    let self_pid = std::process::id(); // 跳过自身
    let wins: Vec<desktop::WindowInfo> = desktop::list_windows(&desktop::WindowFilter { only_visible: true, ..Default::default() }).into_iter().filter(|w| w.process_id != self_pid).collect(); // 可见顶层窗
    let mut procs_map: HashMap<u32, String> = HashMap::new(); // 进程表
    for w in &wins { if !w.process_name.is_empty() { procs_map.entry(w.process_id).or_insert_with(|| w.process_name.clone()); } } // 汇总进程
    let procs: Vec<Value> = procs_map.iter().map(|(pid, name)| json!({"pid": pid, "name": name})).collect(); // 序列化进程
    let win_vals: Vec<Value> = wins.iter().map(|w| json!({"window_id": w.hwnd, "pid": w.process_id, "app_name": w.process_name, "title": w.title, "x": w.x, "y": w.y, "width": w.width, "height": w.height})).collect(); // 序列化窗口
    let mut lines = vec![format!("{} running GUI process(es), {} visible window(s)", procs.len(), win_vals.len())]; // 文本摘要头
    for w in &wins { lines.push(format!("- pid={} ({}) {:?} [window_id: {}] {}x{}+{}+{}", w.process_id, w.process_name, w.title, w.hwnd, w.width, w.height, w.x, w.y)); } // 逐窗行
    json!({"ok": true, "data": {"summary": lines.join("\n"), "processes": procs, "windows": win_vals}}) // 返回发现态快照
}
pub async fn debug_window_info(uia: &UiaHandle, args: &Map<String, Value>) -> Value { // computer_debug_window_info：诊断 pid 全部顶层窗、类名、UIA 焦点与 Pattern
    let Some(pid) = args.get("pid").and_then(Value::as_u64).filter(|p| *p > 0).map(|p| p as u32) else { return json!({"ok": false, "error": "debug_window_info 缺少正整数 pid"}); }; // 校验 pid
    let exe_name = desktop::process_name_of(pid); // 读取 exe 名
    let wins: Vec<Value> = desktop::list_windows(&desktop::WindowFilter::default()).into_iter().filter(|w| w.process_id == pid).map(|w| json!({"window_id": w.hwnd, "title": w.title, "class_name": w.class_name, "visible": w.visible, "minimized": desktop::is_minimized(w.hwnd), "bounds": {"x": w.x, "y": w.y, "width": w.width, "height": w.height}})).collect(); // 枚举全部窗口含隐藏
    let focus = uia.call(move |worker| { // 查询 UIA 焦点控件与可用 Pattern
        let focused = unsafe { worker.automation().GetFocusedElement() }; // 取全局焦点元素
        match focused { // 判读焦点所属
            Ok(el) => {
                let id = worker.store(&el); // 暂存便于读属性
                let info = element::element_info(id, &el); // 基础属性
                let patterns = element::supported_patterns(&el); // 探测支持的 UIA Pattern
                Ok(json!({"matches_pid": info.process_id == pid, "process_id": info.process_id, "name": info.name, "role": info.control_type, "automation_id": info.automation_id, "class_name": info.class_name, "supported_patterns": patterns})) // 返回焦点详情
            }
            Err(err) => Ok(json!({"error": format!("GetFocusedElement 失败：{err}")})), // 读取焦点失败
        }
    }).await.unwrap_or_else(|e| json!({"error": e.to_string()})); // 执行并兜底
    json!({"ok": true, "data": {"pid": pid, "exe_name": exe_name, "window_count": wins.len(), "windows": wins, "focused_element": focus}}) // 返回诊断负载
}
pub fn get_screen_size() -> Value { // computer_get_screen_size：返回主显示器物理分辨率与缩放因子
    let (w, h, scale) = screen::primary_screen_metrics(); // 查询主屏
    json!({"ok": true, "data": {"width": w, "height": h, "scale_factor": scale}}) // 返回宽高与 DPI 倍率
}
pub fn get_cursor_position() -> Value { // computer_get_cursor_position：返回系统当前光标屏幕像素坐标
    let (x, y) = input::cursor_position(); // 读取 GetCursorPos
    json!({"ok": true, "data": {"x": x, "y": y}}) // 返回坐标
}