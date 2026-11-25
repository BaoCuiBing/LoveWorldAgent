use serde_json::{json, Map, Value}; // JSON 构造
use std::path::Path; // adb 路径
use crate::mobile::adb::{self, ToolError, ToolResult}; // adb 与错误
use crate::mobile::{apps, coords, devices, input, logs, recording, screen, settings, ui}; // 移动端能力
pub const TOOLS: [&str; 28] = ["mobile_list_available_devices", "mobile_list_apps", "mobile_get_foreground_app", "mobile_launch_app", "mobile_terminate_app", "mobile_install_app", "mobile_uninstall_app", "mobile_get_screen_size", "mobile_click_on_screen_at_coordinates", "mobile_double_tap_on_screen", "mobile_long_press_on_screen_at_coordinates", "mobile_list_elements_on_screen", "mobile_press_button", "mobile_open_url", "mobile_swipe_on_screen", "mobile_type_keys", "mobile_save_screenshot", "mobile_take_screenshot", "mobile_set_orientation", "mobile_set_location", "mobile_clipboard", "mobile_get_device_logs", "mobile_get_orientation", "mobile_start_screen_recording", "mobile_stop_screen_recording", "mobile_list_crashes", "mobile_get_crash", "mobile_batch_commands"]; // 28 个本地工具
pub fn is_known(cmd: &str) -> bool { TOOLS.contains(&cmd) } // 白名单校验
fn arg_str(args: &Map<String, Value>, key: &str) -> Option<String> { args.get(key).and_then(Value::as_str).map(|text| text.trim().to_string()).filter(|text| !text.is_empty()) } // 字符串参数
fn arg_i64(args: &Map<String, Value>, key: &str) -> Option<i64> { args.get(key).and_then(Value::as_i64) } // 整数参数
fn arg_bool(args: &Map<String, Value>, key: &str) -> Option<bool> { args.get(key).and_then(Value::as_bool) } // 布尔参数
fn need_str(args: &Map<String, Value>, key: &str) -> ToolResult<String> { arg_str(args, key).ok_or_else(|| ToolError::new("INVALID_ARGUMENT", format!("缺少参数 {key}"))) } // 必填字符串
fn success(data: Value) -> Value { json!({"ok": true, "data": data}) } // 成功信封
fn fail(err: ToolError) -> Value { json!({"ok": false, "code": err.code, "error": err.text()}) } // 失败信封
fn device() -> ToolResult<(std::path::PathBuf, String)> { crate::mobile::adb_and_device() } // 解析 adb 与设备
fn point(adb: &Path, serial: &str, args: &Map<String, Value>) -> ToolResult<(i64, i64)> { // 解析 ref 或 x/y 并换算
    let (x, y) = if let Some(reference) = arg_str(args, "ref") { // 元素引用优先
        let node = ui::node_by_ref(&reference).ok_or_else(|| ToolError::new("INVALID_ARGUMENT", format!("未知元素引用：{reference}（先调用 mobile_list_elements_on_screen）")))?; // 查找
        (node.center[0], node.center[1]) // 中心点
    } else { // 坐标
        let x = arg_i64(args, "x").ok_or_else(|| ToolError::new("INVALID_ARGUMENT", "缺少 x 或 ref"))?; // X
        let y = arg_i64(args, "y").ok_or_else(|| ToolError::new("INVALID_ARGUMENT", "缺少 y 或 ref"))?; // Y
        (x, y) // 原始坐标
    };
    let (nx, ny) = if let (Some(shot), Ok(size)) = (screen::last_size(), coords::screen_size(adb, serial)) { // 已有截图尺寸时换算
        if shot.width != size.width || shot.height != size.height { coords::scale_to_device(x as f64, y as f64, shot, size) } else { (x, y) } // 比例缩放
    } else { (x, y) }; // 无截图直接使用
    let size = coords::screen_size(adb, serial).ok(); // 设备尺寸
    match size { Some(value) => Ok((coords::clamp(nx, value.width), coords::clamp(ny, value.height))), None => Ok((nx, ny)) } // 限制屏幕内
}
pub fn definitions() -> Vec<Value> { // 28 个工具清单（MCP/外部智能体复用）
    vec![
        json!({"name": "mobile_list_available_devices", "description": "列出已授权的 Android 设备（serial/model）", "inputSchema": {"type": "object", "properties": {}, "additionalProperties": true}}),
        json!({"name": "mobile_list_apps", "description": "列出已安装应用包名；includeSystem=true 时含系统应用", "inputSchema": {"type": "object", "properties": {"includeSystem": {"type": "boolean"}}, "additionalProperties": true}}),
        json!({"name": "mobile_get_foreground_app", "description": "读取当前前台应用组件与包名", "inputSchema": {"type": "object", "properties": {}, "additionalProperties": true}}),
        json!({"name": "mobile_launch_app", "description": "按包名启动或激活应用", "inputSchema": {"type": "object", "properties": {"app": {"type": "string"}}, "required": ["app"], "additionalProperties": true}}),
        json!({"name": "mobile_terminate_app", "description": "按包名强制停止应用", "inputSchema": {"type": "object", "properties": {"app": {"type": "string"}}, "required": ["app"], "additionalProperties": true}}),
        json!({"name": "mobile_install_app", "description": "安装本地 APK", "inputSchema": {"type": "object", "properties": {"path": {"type": "string"}}, "required": ["path"], "additionalProperties": true}}),
        json!({"name": "mobile_uninstall_app", "description": "卸载指定包名应用", "inputSchema": {"type": "object", "properties": {"app": {"type": "string"}}, "required": ["app"], "additionalProperties": true}}),
        json!({"name": "mobile_get_screen_size", "description": "读取设备物理分辨率与密度", "inputSchema": {"type": "object", "properties": {}, "additionalProperties": true}}),
        json!({"name": "mobile_click_on_screen_at_coordinates", "description": "点击坐标或元素引用 ref=@eN", "inputSchema": {"type": "object", "properties": {"x": {"type": "integer"}, "y": {"type": "integer"}, "ref": {"type": "string"}}, "additionalProperties": true}}),
        json!({"name": "mobile_double_tap_on_screen", "description": "双击坐标或元素引用", "inputSchema": {"type": "object", "properties": {"x": {"type": "integer"}, "y": {"type": "integer"}, "ref": {"type": "string"}}, "additionalProperties": true}}),
        json!({"name": "mobile_long_press_on_screen_at_coordinates", "description": "长按坐标或元素引用", "inputSchema": {"type": "object", "properties": {"x": {"type": "integer"}, "y": {"type": "integer"}, "ref": {"type": "string"}, "duration": {"type": "integer"}}, "additionalProperties": true}}),
        json!({"name": "mobile_list_elements_on_screen", "description": "抓取 uiautomator 元素树，返回带 @eN 引用与 bounds", "inputSchema": {"type": "object", "properties": {}, "additionalProperties": true}}),
        json!({"name": "mobile_press_button", "description": "发送按键：home/back/enter/power/menu/recent/volume_up/volume_down", "inputSchema": {"type": "object", "properties": {"button": {"type": "string"}}, "required": ["button"], "additionalProperties": true}}),
        json!({"name": "mobile_open_url", "description": "用系统浏览器打开网址", "inputSchema": {"type": "object", "properties": {"url": {"type": "string"}}, "required": ["url"], "additionalProperties": true}}),
        json!({"name": "mobile_swipe_on_screen", "description": "滑动：可传 direction 或 x1/y1/x2/y2", "inputSchema": {"type": "object", "properties": {"direction": {"type": "string"}, "x1": {"type": "integer"}, "y1": {"type": "integer"}, "x2": {"type": "integer"}, "y2": {"type": "integer"}, "duration": {"type": "integer"}}, "additionalProperties": true}}),
        json!({"name": "mobile_type_keys", "description": "输入文本；submit=true 时回车提交", "inputSchema": {"type": "object", "properties": {"text": {"type": "string"}, "submit": {"type": "boolean"}}, "required": ["text"], "additionalProperties": true}}),
        json!({"name": "mobile_save_screenshot", "description": "截图并落盘到 static/data/mobile_screen", "inputSchema": {"type": "object", "properties": {"filename": {"type": "string"}}, "additionalProperties": true}}),
        json!({"name": "mobile_take_screenshot", "description": "截图并返回 base64", "inputSchema": {"type": "object", "properties": {}, "additionalProperties": true}}),
        json!({"name": "mobile_set_orientation", "description": "设置屏幕方向：portrait/landscape", "inputSchema": {"type": "object", "properties": {"orientation": {"type": "string"}}, "required": ["orientation"], "additionalProperties": true}}),
        json!({"name": "mobile_set_location", "description": "模拟器设置经纬度（adb emu geo fix）", "inputSchema": {"type": "object", "properties": {"latitude": {"type": "number"}, "longitude": {"type": "number"}}, "required": ["latitude", "longitude"], "additionalProperties": true}}),
        json!({"name": "mobile_clipboard", "description": "剪贴板读写：action=get/set", "inputSchema": {"type": "object", "properties": {"action": {"type": "string"}, "text": {"type": "string"}}, "required": ["action"], "additionalProperties": true}}),
        json!({"name": "mobile_get_device_logs", "description": "读取最近 logcat 行", "inputSchema": {"type": "object", "properties": {"lines": {"type": "integer"}}, "additionalProperties": true}}),
        json!({"name": "mobile_get_orientation", "description": "读取当前屏幕方向", "inputSchema": {"type": "object", "properties": {}, "additionalProperties": true}}),
        json!({"name": "mobile_start_screen_recording", "description": "开始录屏（默认 180s 上限）", "inputSchema": {"type": "object", "properties": {"timeLimit": {"type": "integer"}}, "additionalProperties": true}}),
        json!({"name": "mobile_stop_screen_recording", "description": "停止录屏并拉取 mp4", "inputSchema": {"type": "object", "properties": {}, "additionalProperties": true}}),
        json!({"name": "mobile_list_crashes", "description": "列出 crash 缓冲区中的崩溃摘要", "inputSchema": {"type": "object", "properties": {}, "additionalProperties": true}}),
        json!({"name": "mobile_get_crash", "description": "读取某次崩溃详情", "inputSchema": {"type": "object", "properties": {"index": {"type": "integer"}}, "additionalProperties": true}}),
        json!({"name": "mobile_batch_commands", "description": "批量执行 mobile_* 工具（≤20 条）", "inputSchema": {"type": "object", "properties": {"commands": {"type": "array", "items": {"type": "object"}}}, "required": ["commands"], "additionalProperties": true}}),
    ]
}
fn swipe(args: &Map<String, Value>, adb: &Path, serial: &str) -> ToolResult<Value> { // 滑动实现
    let duration = arg_i64(args, "duration").unwrap_or(300).max(100) as u64; // 时长
    let (from, to) = if let (Some(x1), Some(y1), Some(x2), Some(y2)) = (arg_i64(args, "x1"), arg_i64(args, "y1"), arg_i64(args, "x2"), arg_i64(args, "y2")) { // 显式坐标
        ((x1, y1), (x2, y2)) // 使用入参
    } else { // 方向滑动
        let size = coords::screen_size(adb, serial)?; // 设备尺寸
        let (w, h) = (size.width as i64, size.height as i64); // 宽高
        let direction = arg_str(args, "direction").unwrap_or_else(|| "up".to_string()).to_lowercase(); // 方向
        match direction.as_str() { // 分派
            "down" => ((w / 2, h / 4), (w / 2, h * 3 / 4)), // 下滑
            "left" => ((w * 3 / 4, h / 2), (w / 4, h / 2)), // 左滑
            "right" => ((w / 4, h / 2), (w * 3 / 4, h / 2)), // 右滑
            _ => ((w / 2, h * 3 / 4), (w / 2, h / 4)), // 上滑
        }
    };
    input::swipe(adb, serial, from, to, duration)?; // 执行
    Ok(json!({"from": [from.0, from.1], "to": [to.0, to.1], "duration": duration})) // 返回
}
fn batch(args: &Map<String, Value>) -> ToolResult<Value> { // 批量执行
    let Some(items) = args.get("commands").and_then(Value::as_array) else { return Err(ToolError::new("INVALID_ARGUMENT", "commands 必须为数组")); }; // 校验
    let mut results = Vec::new(); // 结果
    for item in items.iter().take(20) { // 逐条执行
        let name = item.get("name").and_then(Value::as_str).unwrap_or(""); // 工具名
        let params = item.get("args").and_then(Value::as_object).cloned().unwrap_or_default(); // 参数
        if name == "mobile_batch_commands" { results.push(json!({"name": name, "ok": false, "error": "INVALID_ARGUMENT: 不允许嵌套批量"})); continue; } // 禁止递归
        if !is_known(name) { results.push(json!({"name": name, "ok": false, "error": format!("未知工具：{name}")})); continue; } // 白名单
        let value = dispatch(name, &params); // 执行
        results.push(json!({"name": name, "result": value})); // 收集
    }
    Ok(json!({"total": results.len(), "results": results})) // 返回
}
fn dispatch(cmd: &str, args: &Map<String, Value>) -> Value { // 工具分派（返回 {ok,data|error}）
    if cmd == "mobile_batch_commands" { return match batch(args) { Ok(data) => success(data), Err(err) => fail(err) }; } // 批量特殊处理
    if cmd == "mobile_list_available_devices" { // 设备列表
        let config = settings(); // 读取设置
        return match adb::resolve_adb(Some(config.adb_path.as_str())) { Ok(adb) => match devices::list(&adb) { Ok(data) => success(data), Err(err) => fail(err) }, Err(err) => fail(err) }; // 执行
    }
    let (adb, serial) = match device() { Ok(value) => value, Err(err) => return fail(err) }; // 解析设备
    let result = match cmd { // 其余工具
        "mobile_list_apps" => apps::list_apps(&adb, &serial, arg_bool(args, "includeSystem").unwrap_or(false)), // 应用列表
        "mobile_get_foreground_app" => apps::foreground(&adb, &serial), // 前台应用
        "mobile_launch_app" => need_str(args, "app").and_then(|app| apps::launch(&adb, &serial, &app)), // 启动
        "mobile_terminate_app" => need_str(args, "app").and_then(|app| apps::terminate(&adb, &serial, &app)), // 停止
        "mobile_install_app" => need_str(args, "path").and_then(|path| apps::install(&adb, &serial, &path)), // 安装
        "mobile_uninstall_app" => need_str(args, "app").and_then(|app| apps::uninstall(&adb, &serial, &app)), // 卸载
        "mobile_get_screen_size" => screen::info(&adb, &serial).map(|info| json!(info)), // 屏幕尺寸
        "mobile_click_on_screen_at_coordinates" => point(&adb, &serial, args).and_then(|(x, y)| input::tap(&adb, &serial, x, y).map(|_| json!({"x": x, "y": y}))), // 点击
        "mobile_double_tap_on_screen" => point(&adb, &serial, args).and_then(|(x, y)| input::double_tap(&adb, &serial, x, y).map(|_| json!({"x": x, "y": y}))), // 双击
        "mobile_long_press_on_screen_at_coordinates" => point(&adb, &serial, args).and_then(|(x, y)| input::long_press(&adb, &serial, x, y, arg_i64(args, "duration").unwrap_or(800) as u64).map(|_| json!({"x": x, "y": y}))), // 长按
        "mobile_list_elements_on_screen" => ui::dump(&adb, &serial).map(|nodes| json!({"elements": nodes, "total": nodes.len()})), // 元素树
        "mobile_press_button" => need_str(args, "button").and_then(|button| input::key(&adb, &serial, &button).map(|_| json!({"button": button}))), // 按键
        "mobile_open_url" => need_str(args, "url").and_then(|url| input::open_url(&adb, &serial, &url).map(|_| json!({"url": url}))), // 打开网址
        "mobile_swipe_on_screen" => swipe(args, &adb, &serial), // 滑动
        "mobile_type_keys" => need_str(args, "text").and_then(|text| { input::text(&adb, &serial, &text)?; if arg_bool(args, "submit").unwrap_or(false) { input::key(&adb, &serial, "enter")?; } Ok(json!({"text": text})) }), // 输入
        "mobile_save_screenshot" => screen::save(&adb, &serial, arg_str(args, "filename").as_deref()), // 截图落盘
        "mobile_take_screenshot" => screen::capture(&adb, &serial), // 截图 base64
        "mobile_set_orientation" => need_str(args, "orientation").and_then(|value| screen::set_orientation(&adb, &serial, &value)), // 方向
        "mobile_set_location" => { // 模拟器经纬度
            let lat = args.get("latitude").and_then(Value::as_f64).ok_or_else(|| ToolError::new("INVALID_ARGUMENT", "缺少 latitude")); // 纬度
            let lon = args.get("longitude").and_then(Value::as_f64).ok_or_else(|| ToolError::new("INVALID_ARGUMENT", "缺少 longitude")); // 经度
            lat.and_then(|lat| lon.map(|lon| (lat, lon))).and_then(|(lat, lon)| { // 取坐标
                let longitude = lon.to_string(); // 经度文本
                let latitude = lat.to_string(); // 纬度文本
                adb::run(&adb, Some(&serial), &["emu", "geo", "fix", &longitude, &latitude], std::time::Duration::from_secs(15)).and_then(|output| if output.ok { Ok(json!({"latitude": lat, "longitude": lon})) } else { Err(ToolError::new("UNSUPPORTED_INPUT", output.error_text())) }) // 执行
            })
        }
        "mobile_clipboard" => need_str(args, "action").and_then(|action| input::clipboard(&adb, &serial, &action, arg_str(args, "text").as_deref()).map(|text| json!({"text": text}))), // 剪贴板
        "mobile_get_device_logs" => logs::device_logs(&adb, &serial, arg_i64(args, "lines").unwrap_or(200).max(1) as usize), // 日志
        "mobile_get_orientation" => screen::orientation(&adb, &serial), // 方向读取
        "mobile_start_screen_recording" => recording::start(&adb, &serial, arg_i64(args, "timeLimit").unwrap_or(180) as u64), // 开始录屏
        "mobile_stop_screen_recording" => recording::stop(&adb, &serial), // 停止录屏
        "mobile_list_crashes" => logs::list_crashes(&adb, &serial), // 崩溃列表
        "mobile_get_crash" => logs::get_crash(&adb, &serial, arg_i64(args, "index").map(|value| value.max(0) as usize)), // 崩溃详情
        _ => Err(ToolError::new("INVALID_ARGUMENT", format!("未知工具：{cmd}"))), // 未知
    };
    match result { Ok(data) => success(data), Err(err) => fail(err) } // 包装
}
pub fn call(cmd: &str, args: &Map<String, Value>) -> Value { // 对外入口
    if !is_known(cmd) { return fail(ToolError::new("INVALID_ARGUMENT", format!("未知工具：{cmd}"))); } // 白名单
    dispatch(cmd, args) // 分派
}
