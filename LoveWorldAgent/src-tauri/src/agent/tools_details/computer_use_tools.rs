use serde_json::{json, Value}; // 桌面自动化工具：统一走 utils/computer_use.rs 的命名管道，名称与 sidecar 23 个对齐工具完全一致
use crate::agent::tools::{blocking, ToolDef}; // 工具定义与同步包装
use crate::utils::computer_use; // 管道客户端
fn spec_defs() -> Vec<(&'static str, &'static str, Value, f64)> { // 23 工具规格（对齐 参考工具.md 与 platform-windows，状态感知仅用 omni_parser，不回传直接截图）
    let delivery = json!({"type": "string", "enum": ["background", "foreground"], "default": "background", "description": "投递模式：background 后台优先；仅当明确被拒再改 foreground"}); // 投递模式
    let scope = json!({"type": "string", "enum": ["window", "desktop"], "default": "window", "description": "坐标域：window 为窗口局部像素，desktop 为全桌面屏幕坐标"}); // 坐标作用域
    vec![
        ("computer_click", "点击目标（优先 element_token，或窗口相对 x,y；desktop 域用屏幕坐标）；action='expand' 可展开控件", json!({"type": "object", "properties": {"pid": {"type": "integer"}, "window_id": {"type": "integer"}, "element_token": {"type": "string"}, "action": {"type": "string"}, "x": {"type": "number"}, "y": {"type": "number"}, "button": {"type": "string", "enum": ["left", "right", "middle"], "default": "left"}, "count": {"type": "integer", "minimum": 1, "maximum": 3, "default": 1}, "modifier": {"type": "array", "items": {"type": "string"}}, "from_zoom": {"type": "boolean"}, "scope": scope.clone(), "delivery_mode": delivery.clone()}}), 25.0),
        ("computer_double_click", "双击目标元素（element_token）或窗口相对像素 (x,y)", json!({"type": "object", "properties": {"pid": {"type": "integer"}, "window_id": {"type": "integer"}, "element_token": {"type": "string"}, "x": {"type": "number"}, "y": {"type": "number"}, "modifier": {"type": "array", "items": {"type": "string"}}, "from_zoom": {"type": "boolean"}, "delivery_mode": delivery.clone()}}), 25.0),
        ("computer_right_click", "右键点击目标元素（element_token）或窗口相对像素 (x,y)", json!({"type": "object", "properties": {"pid": {"type": "integer"}, "window_id": {"type": "integer"}, "element_token": {"type": "string"}, "x": {"type": "number"}, "y": {"type": "number"}, "modifier": {"type": "array", "items": {"type": "string"}}, "from_zoom": {"type": "boolean"}, "delivery_mode": delivery.clone()}}), 25.0),
        ("computer_drag", "从 (from_x,from_y) 拖拽至 (to_x,to_y)，支持 duration_ms/steps/button/modifier 及 scope='desktop'", json!({"type": "object", "properties": {"scope": scope.clone(), "pid": {"type": "integer"}, "window_id": {"type": "integer"}, "from_x": {"type": "number"}, "from_y": {"type": "number"}, "to_x": {"type": "number"}, "to_y": {"type": "number"}, "duration_ms": {"type": "integer", "default": 500}, "steps": {"type": "integer", "default": 20}, "modifier": {"type": "array", "items": {"type": "string"}}, "button": {"type": "string", "enum": ["left", "right", "middle"], "default": "left"}, "from_zoom": {"type": "boolean"}, "delivery_mode": delivery.clone()}, "required": ["from_x", "from_y", "to_x", "to_y"]}), 30.0),
        ("computer_type_text", "输入文本（支持 scope='desktop'、element_token 或先点窗口内 x,y 聚焦，delay_ms 控制键间间隔 0..200）", json!({"type": "object", "properties": {"scope": scope.clone(), "pid": {"type": "integer"}, "window_id": {"type": "integer"}, "element_token": {"type": "string"}, "x": {"type": "number"}, "y": {"type": "number"}, "text": {"type": "string"}, "delay_ms": {"type": "integer", "minimum": 0, "maximum": 200, "default": 0}, "delivery_mode": delivery.clone()}, "required": ["text"]}), 30.0),
        ("computer_press_key", "按下并释放单个按键及可选修饰键 modifiers（ctrl/shift/alt/win）", json!({"type": "object", "properties": {"scope": scope.clone(), "pid": {"type": "integer"}, "window_id": {"type": "integer"}, "element_token": {"type": "string"}, "x": {"type": "number"}, "y": {"type": "number"}, "key": {"type": "string"}, "modifiers": {"type": "array", "items": {"type": "string"}}, "delivery_mode": delivery.clone()}, "required": ["key"]}), 20.0),
        ("computer_hotkey", "同时按下组合键 keys（至少 2 键，如 ['ctrl','c']）", json!({"type": "object", "properties": {"scope": scope.clone(), "pid": {"type": "integer"}, "window_id": {"type": "integer"}, "element_token": {"type": "string"}, "x": {"type": "number"}, "y": {"type": "number"}, "keys": {"type": "array", "items": {"type": "string"}, "minItems": 2}, "delivery_mode": delivery.clone()}, "required": ["keys"]}), 20.0),
        ("computer_scroll", "滚动窗口、element_token 或桌面坐标；direction=up/down/left/right，by=line/page，amount=步数", json!({"type": "object", "properties": {"scope": scope.clone(), "pid": {"type": "integer"}, "window_id": {"type": "integer"}, "element_token": {"type": "string"}, "x": {"type": "number"}, "y": {"type": "number"}, "direction": {"type": "string", "enum": ["up", "down", "left", "right"]}, "by": {"type": "string", "enum": ["line", "page"], "default": "line"}, "amount": {"type": "integer", "minimum": 1, "maximum": 50, "default": 3}, "delivery_mode": delivery.clone()}, "required": ["direction"]}), 25.0),
        ("computer_set_value", "通过 UIA ValuePattern 对 element_token 元素设置文本并回读校验", json!({"type": "object", "properties": {"pid": {"type": "integer"}, "window_id": {"type": "integer"}, "element_token": {"type": "string"}, "value": {"type": "string"}, "delivery_mode": delivery.clone()}, "required": ["element_token", "value"]}), 20.0),
        ("computer_move_cursor", "移动光标（scope='window' 仅移动代理覆盖层，scope='desktop' 移动系统真实指针）", json!({"type": "object", "properties": {"scope": scope.clone(), "x": {"type": "number"}, "y": {"type": "number"}, "cursor_id": {"type": "string"}}, "required": ["x", "y"]}), 15.0),
        ("computer_list_apps", "列出运行中与已安装的桌面/UWP 应用（name、pid、running、active、kind、launch_path）", json!({"type": "object", "properties": {}}), 30.0),
        ("computer_list_windows", "列出全部顶层窗口（window_id、pid、app_name、title、bounds、z_index、is_on_screen、minimized），支持 pid/app/on_screen_only 过滤", json!({"type": "object", "properties": {"pid": {"type": "integer"}, "app": {"type": "string"}, "on_screen_only": {"type": "boolean", "default": false}}}), 20.0),
        ("computer_launch_app", "按 pid/launch_path/path/aumid/bundle_id/name/urls 启动应用，支持 additional_arguments 与 start_minimized", json!({"type": "object", "properties": {"launch_path": {"type": "string"}, "path": {"type": "string"}, "aumid": {"type": "string"}, "bundle_id": {"type": "string"}, "name": {"type": "string"}, "urls": {"type": "array", "items": {"type": "string"}}, "additional_arguments": {"type": "array", "items": {"type": "string"}}, "start_minimized": {"type": "boolean", "default": false}}}), 95.0),
        ("computer_kill_app", "按 pid 强制结束进程（taskkill /F 等效）", json!({"type": "object", "properties": {"pid": {"type": "integer"}}, "required": ["pid"]}), 20.0),
        ("computer_bring_to_front", "恢复最小化窗口并置前激活，返回 previous_fg_hwnd 与 now_fg_hwnd", json!({"type": "object", "properties": {"pid": {"type": "integer"}, "window_id": {"type": "integer"}}, "required": ["pid"]}), 20.0),
        ("computer_set_window_frame", "设置非最小化/最大化窗口的位置与尺寸（x、y、width、height）", json!({"type": "object", "properties": {"pid": {"type": "integer"}, "window_id": {"type": "integer"}, "x": {"type": "number"}, "y": {"type": "number"}, "width": {"type": "number"}, "height": {"type": "number"}}, "required": ["pid", "window_id", "x", "y", "width", "height"]}), 20.0),
        ("computer_invoke_menu", "沿 path 路径逐层触发生效的原生 UIA 菜单项（如 ['文件','另存为...']）", json!({"type": "object", "properties": {"pid": {"type": "integer"}, "window_id": {"type": "integer"}, "path": {"type": "array", "items": {"type": "string"}, "minItems": 1, "maxItems": 16}}, "required": ["pid", "window_id", "path"]}), 25.0),
        ("computer_get_window_state", "获取目标窗口 UIA 树（含 snapshot_id 与 element_token）及内存画面经 omni_parser 识别的结构化文字/图标框坐标（不回传图片）", json!({"type": "object", "properties": {"pid": {"type": "integer"}, "window_id": {"type": "integer"}, "tree_format": {"type": "string", "enum": ["markdown", "elements", "both"], "default": "markdown"}, "include_accessibility_tree": {"type": "boolean", "default": true}, "include_vision": {"type": "boolean", "default": true}, "query": {"type": "string"}, "max_elements": {"type": "integer", "default": 250}, "max_depth": {"type": "integer", "default": 25}, "full_output": {"type": "boolean", "default": false}, "timeout_ms": {"type": "integer", "default": 4000}}, "required": ["pid", "window_id"]}), 95.0),
        ("computer_get_desktop_state", "获取主显示器尺寸、可见窗口摘要及全桌面经 omni_parser 识别的结构化文字/图标框屏幕坐标（不回传图片）", json!({"type": "object", "properties": {}}), 95.0),
        ("computer_debug_window_info", "诊断指定 pid 全部顶层窗类名、标题、可见性及 UIA 焦点控件与支持的 Pattern", json!({"type": "object", "properties": {"pid": {"type": "integer"}}, "required": ["pid"]}), 20.0),
        ("computer_get_accessibility_tree", "快速获取当前桌面运行中 GUI 进程与可见顶层窗口的轻量发现快照", json!({"type": "object", "properties": {}}), 20.0),
        ("computer_get_screen_size", "返回主显示器物理分辨率 width/height 与 DPI 缩放 scale_factor", json!({"type": "object", "properties": {}}), 15.0),
        ("computer_get_cursor_position", "返回系统当前鼠标指针屏幕像素坐标 (x,y)", json!({"type": "object", "properties": {}}), 15.0),
    ]
}
pub(crate) fn strip_screenshot_fields(value: Value) -> Value { // 纵深防御：递归剥离截图路径与图像字节字段，绝不回传给大模型
    computer_use::strip_screenshot_fields(value)
}
pub fn build_computer_use_tools() -> Vec<ToolDef> { // 构造 23 个对齐桌面自动化工具，全部归入 computer 审批组
    spec_defs().into_iter().map(|(name, description, parameters, timeout)| { // 逐规格生成
        let tool_name = name.to_string(); // 克隆工具名
        ToolDef::new(name, description, parameters, blocking(move |args: String| { // 构造处理器
            let value: Value = serde_json::from_str(&args).unwrap_or_else(|_| json!({})); // 解析参数
            let result = computer_use::call(&tool_name, &value, timeout); // 调用 sidecar
            if result.get("ok").and_then(Value::as_bool).unwrap_or(false) { // 成功
                let clean = strip_screenshot_fields(result.get("data").cloned().unwrap_or(Value::Null));
                clean.to_string() // 返回剥离截图字段后的裸 data 文本
            } else { // 失败
                json!({"ok": false, "error": result.get("error").and_then(Value::as_str).unwrap_or("桌面自动化调用失败")}).to_string() // 返回错误
            }
        })).with_group("computer") // 归入电脑控制审批组
    }).collect() // 汇总
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exposes_exact_twenty_three_tools_without_screenshots() {
        let tools = build_computer_use_tools();
        assert_eq!(tools.len(), 23);
        let serialized: String = tools.iter().map(|t| format!("{} {}", t.name, t.parameters)).collect::<Vec<_>>().join("\n");
        assert!(!serialized.contains(&format!("{}{}", "screenshot_out", "_file")));
        for legacy in ["grounding", "get_app_state"] {
            assert!(!serialized.contains(&format!("computer_{legacy}")));
        }
    }
    #[test]
    fn strip_screenshot_fields_removes_paths_and_image_bytes() {
        let raw = json!({
            "window_id": 123,
            "tree_markdown": "[0] Button \"OK\"",
            "screenshot": "C:\\tmp\\a.png",
            "screenshot_path": "C:\\tmp\\b.png",
            "screenshot_file_path": "C:\\tmp\\c.png",
            "image_png": "abc",
            "image_base64": "def",
            "elements": [
                {"name": "ok", "screenshot": "C:\\tmp\\nested.png", "image_base64": "xyz"}
            ],
            "omni_elements": [{"text": "Save", "bbox": [1, 2, 3, 4]}]
        });
        let clean = strip_screenshot_fields(raw);
        let text = clean.to_string();
        assert!(!text.contains("screenshot"));
        assert!(!text.contains("image_png"));
        assert!(!text.contains("image_base64"));
        assert_eq!(clean.get("window_id").and_then(Value::as_i64), Some(123));
        assert_eq!(clean.get("tree_markdown").and_then(Value::as_str), Some("[0] Button \"OK\""));
        assert_eq!(clean["elements"][0]["name"].as_str(), Some("ok"));
        assert_eq!(clean["omni_elements"][0]["text"].as_str(), Some("Save"));
    }
}