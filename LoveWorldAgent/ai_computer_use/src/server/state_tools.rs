use serde_json::{json, Map, Value}; // 状态感知工具：get_window_state 与 get_desktop_state（全内存抓取 + omni_parser 识别，绝不回传/落盘截图）
use crate::{omni, sys::screen, uia::{desktop, worker::UiaHandle}}; // 视觉识别、截图与 UIA
use super::{apps, snapshots}; // 窗口校验与快照登记
pub async fn get_window_state(uia: &UiaHandle, args: &Map<String, Value>) -> Value { // computer_get_window_state：返回窗口 UIA 树（含 element_token）+ omni_parser 识别结果（绝无图片）
    let Some(pid) = args.get("pid").and_then(Value::as_u64).filter(|p| *p > 0).map(|p| p as u32) else { return json!({"ok": false, "error": "get_window_state 缺少必填整数 pid"}); }; // 校验 pid
    let Some(window_id) = args.get("window_id").and_then(Value::as_i64) else { return json!({"ok": false, "error": "get_window_state 缺少必填整数 window_id，请先用 computer_list_windows 获取"}); }; // 校验 window_id
    if let Err(err) = apps::resolve_pid_hwnd(Some(pid), Some(window_id)) { return json!({"ok": false, "error": err}); } // 严格校验窗口归属 pid
    let include_tree = args.get("include_accessibility_tree").and_then(Value::as_bool).unwrap_or(true); // 是否遍历 UIA 树（默认 true）
    let include_vision = args.get("include_vision").and_then(Value::as_bool).unwrap_or_else(|| args.get("include_screenshot").and_then(Value::as_bool).unwrap_or(true)); // 是否调用 omni_parser 解析画面（默认 true；兼容忽略截图回传请求）
    if !include_tree && !include_vision { return json!({"ok": false, "error": "include_accessibility_tree 与视觉识别不能同时关闭"}); } // 至少开启一项
    let tree_format = args.get("tree_format").and_then(Value::as_str).unwrap_or("markdown").to_ascii_lowercase(); // markdown | elements | both
    let query = args.get("query").and_then(Value::as_str).map(str::to_string); // 过滤关键词 a|b
    let full_output = args.get("full_output").and_then(Value::as_bool).unwrap_or(false); // 放宽数量上限
    let max_elements = args.get("max_elements").and_then(Value::as_u64).map(|v| v as usize).unwrap_or(if full_output { 5000 } else { 250 }); // 默认 250 节点
    let max_depth = args.get("max_depth").and_then(Value::as_u64).map(|v| v as usize).unwrap_or(25); // 默认深度 25
    let timeout_ms = args.get("timeout_ms").and_then(Value::as_u64).unwrap_or(4000); // 默认 4s 预算
    let title = desktop::window_title(window_id); // 窗口标题
    let app_name = desktop::process_name_of(pid); // 进程名
    let outer = screen::window_bounds_outer(window_id).unwrap_or((0, 0, 0, 0)); // 窗口外框（与 PrintWindow 像素同原点）
    let visible = screen::window_bounds_visible(window_id).unwrap_or(outer); // DWM 可见矩形
    let mut payload = json!({"pid": pid, "window_id": window_id, "app_name": app_name, "window_title": title, "window_bounds": {"x": visible.0, "y": visible.1, "width": visible.2, "height": visible.3}, "capture_origin": {"x": outer.0, "y": outer.1, "width": outer.2, "height": outer.3}}); // 基础元信息
    if include_tree { // 1) 遍历 UIA 树
        let walk_res = walk_uia_tree(uia, window_id, max_depth, max_elements, timeout_ms).await; // 读取强类型树
        match walk_res { // 处理 UIA 结果
            Ok(walk) => {
                let total_count = walk.total_count; // 总节点
                let (indices, query_hint) = snapshots::project_elements(&walk.elements, query.as_deref()); // 投影
                let (snapshot_id, invalidated) = snapshots::register_snapshot(pid, window_id, walk.elements.clone()); // 登记快照生成 snapshot_id
                let (md, elems) = snapshots::format_markdown_and_json(&snapshot_id, &walk.elements, &indices, (outer.0, outer.1)); // 生成 Markdown 与数组
                payload["snapshot_id"] = json!(snapshot_id); // 回传 snapshot_id
                if !invalidated.is_empty() { payload["invalidated_snapshot_ids"] = json!(invalidated); } // 失效旧快照
                payload["total_element_count"] = json!(total_count); // 总节点数
                payload["returned_element_count"] = json!(indices.len()); // 投影后节点数
                payload["truncated"] = json!(walk.truncated); // 是否截断
                if !query_hint.is_empty() { payload["query_note"] = json!(query_hint); } // 空匹配提示
                if full_output || tree_format == "both" || tree_format == "markdown" { payload["tree_markdown"] = json!(md); } // 输出紧凑树
                payload["elements"] = json!(elems); // 始终提供结构化 element_token 数组（含 both/elements/默认视图）
            }
            Err(err) => { payload["uia_error"] = json!(err); payload["truncated"] = json!(true); } // UIA 失败仍保留后续 omni_parser 结果
        }
    }
    if include_vision { // 2) 全内存抓图并交给 omni_parser（绝不返回 base64 PNG 或写文件）
        let vision_res = tokio::task::spawn_blocking(move || -> anyhow::Result<omni::VisionParseOutput> {
            let (png, ox, oy, w, h) = screen::capture_window_png(window_id)?; // PrintWindow+BitBlt 内存抓取
            omni::parse_png_region(&png, ox, oy, w, h, 400) // 送入 omni_parser 解析文字与图标框
        }).await;
        match vision_res { // 写入视觉结构化输出
            Ok(Ok(v)) => {
                payload["omni_elements"] = json!(v.elements); // 全部检测项（含无文本 icon 框 + 窗口相对/屏幕坐标）
                payload["vision_text_count"] = json!(v.text_count); // 文本数
                payload["vision_icon_count"] = json!(v.icon_count); // 图标框数
                payload["vision_ms"] = json!(v.ms); // 耗时
                payload["vision_summary"] = json!(v.summary); // 紧凑视觉文本与图标清单
            }
            Ok(Err(err)) => { payload["vision_error"] = json!(err.to_string()); } // 识别失败报错但保留 UIA 树
            Err(err) => { payload["vision_error"] = json!(format!("视觉线程异常：{err}")); } // 线程异常
        }
    }
    json!({"ok": true, "data": payload}) // 绝不含 screenshot/image_png/screenshot_file_path 字段
}
async fn walk_uia_tree(uia: &UiaHandle, window_id: i64, max_depth: usize, max_elements: usize, timeout_ms: u64) -> Result<snapshots::WalkedWindowTree, String> { // 通过 UIA 专属线程执行带预算遍历并返回结构体
    let slot = std::sync::Arc::new(std::sync::Mutex::new(None::<snapshots::WalkedWindowTree>)); // 结果传递槽
    let slot_w = slot.clone(); // 移入闭包
    let res = uia.call(move |worker| {
        let walked = snapshots::walk_window(worker, window_id, max_depth, max_elements, timeout_ms).map_err(anyhow::Error::msg)?; // 遍历
        *slot_w.lock().expect("slot lock") = Some(walked); // 写入槽
        Ok(json!({})) // 空占位返回值
    }).await;
    res.map_err(|e| e.to_string())?; // 传递错误
    let res_tree = slot.lock().expect("slot lock").take().ok_or_else(|| "UIA 遍历结果丢失".to_string()); // 取出树结构
    res_tree // 返回
}
pub async fn get_desktop_state(args: &Map<String, Value>) -> Value { // computer_get_desktop_state：全桌面抓图交 omni_parser 解析（严禁 pid/window_id 与任何图片返回/落盘）
    if args.contains_key("pid") || args.contains_key("window_id") { return json!({"ok": false, "error": "computer_get_desktop_state 仅用于全局桌面感知，严禁传 pid 或 window_id"}); } // 严格拒绝窗口参数
    let (sw, sh, scale) = screen::primary_screen_metrics(); // 主屏分辨率与倍率
    let (vx, vy, vw, vh) = screen::virtual_screen(); // 虚拟桌面边界
    let self_pid = std::process::id(); // 跳过自身 sidecar
    let wins: Vec<Value> = desktop::list_windows(&desktop::WindowFilter { only_visible: true, ..Default::default() }).into_iter().filter(|w| w.process_id != self_pid).take(30).map(|w| json!({"window_id": w.hwnd, "pid": w.process_id, "app_name": w.process_name, "title": w.title, "bounds": {"x": w.x, "y": w.y, "width": w.width, "height": w.height}})).collect(); // 当前桌面顶层窗摘要
    let vision_res = tokio::task::spawn_blocking(move || -> anyhow::Result<omni::VisionParseOutput> {
        let (png, ox, oy, w, h) = screen::capture_region_png(vx, vy, vw, vh)?; // 内存抓取虚拟桌面
        omni::parse_png_region(&png, ox, oy, w, h, 500) // 送入 omni_parser（含图标与 OCR 文字，coords 为屏幕坐标）
    }).await;
    let mut data = json!({"platform": "windows", "display": "primary", "screen_width": sw, "screen_height": sh, "scale_factor": scale, "virtual_bounds": {"x": vx, "y": vy, "width": vw, "height": vh}, "windows": wins}); // 基础负载
    match vision_res { // 填入结构化视觉结果
        Ok(Ok(v)) => {
            data["omni_elements"] = json!(v.elements); // 全量识别结果（x/y 为屏幕像素坐标）
            data["vision_text_count"] = json!(v.text_count); // 文本数量
            data["vision_icon_count"] = json!(v.icon_count); // 图标数量
            data["vision_ms"] = json!(v.ms); // 耗时
            data["vision_summary"] = json!(v.summary); // 紧凑清单
        }
        Ok(Err(err)) => return json!({"ok": false, "error": format!("桌面视觉解析失败：{err}")}), // 返回解析错误
        Err(err) => return json!({"ok": false, "error": format!("桌面视觉线程异常：{err}")}), // 线程错误
    }
    json!({"ok": true, "data": data}) // 返回纯净结构化状态（绝无 base64 PNG 或文件路径）
}