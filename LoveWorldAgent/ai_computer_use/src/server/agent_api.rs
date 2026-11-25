use serde_json::{json, Map, Value}; // 内置桌面自动化服务：严格导出与 参考工具.md / platform-windows 对齐的 23 个 computer_* 工具（绝不直接回传截图，状态感知一律走 omni_parser）
use crate::uia::worker::UiaHandle; // UIA 线程句柄
use super::{actions, apps, state_tools}; // 分模块执行器
pub fn definitions() -> Value { // 导出 23 个工具契约定义（与 src-tauri 注册清单完全一致）
    json!([
        {"name": "computer_click", "description": "点击目标窗口元素（优先传 element_token，或窗口相对像素 x,y，支持 desktop 作用域屏幕坐标）；可指定 button/count/modifier/action='expand'/delivery_mode", "inputSchema": {"type": "object"}},
        {"name": "computer_double_click", "description": "双击目标元素（element_token）或窗口相对坐标 (x,y)", "inputSchema": {"type": "object"}},
        {"name": "computer_right_click", "description": "右键点击目标元素（element_token）或窗口相对坐标 (x,y)", "inputSchema": {"type": "object"}},
        {"name": "computer_drag", "description": "从 (from_x,from_y) 拖拽至 (to_x,to_y)，支持 duration_ms、steps、button、modifier 与 scope='desktop'", "inputSchema": {"type": "object", "required": ["from_x", "from_y", "to_x", "to_y"]}},
        {"name": "computer_type_text", "description": "输入文本（支持 scope='desktop'、element_token、或先点击窗口内 x,y 聚焦再输入，支持 delay_ms）", "inputSchema": {"type": "object", "required": ["text"]}},
        {"name": "computer_press_key", "description": "发送单个按键及可选修饰键 modifiers（ctrl/shift/alt/win）", "inputSchema": {"type": "object", "required": ["key"]}},
        {"name": "computer_hotkey", "description": "同时发送组合键（如 ['ctrl','c']）", "inputSchema": {"type": "object", "required": ["keys"]}},
        {"name": "computer_scroll", "description": "滚动目标窗口、元素（element_token）或桌面坐标（up/down/left/right，by='line'|'page'，amount）", "inputSchema": {"type": "object", "required": ["direction"]}},
        {"name": "computer_set_value", "description": "通过 UIA ValuePattern 为 element_token 元素直接设置值并回读校验", "inputSchema": {"type": "object", "required": ["element_token", "value"]}},
        {"name": "computer_move_cursor", "description": "移动光标（scope='window' 仅移动代理覆盖提示，scope='desktop' 移动系统真实指针）", "inputSchema": {"type": "object", "required": ["x", "y"]}},
        {"name": "computer_list_apps", "description": "列出所有运行中与已安装的桌面/UWP 应用及启动路径（pid、running、active、kind、launch_path）", "inputSchema": {"type": "object"}},
        {"name": "computer_list_windows", "description": "列出顶层窗口（window_id、pid、app_name、title、bounds、z_index、is_on_screen、minimized），支持 pid/app/on_screen_only 过滤", "inputSchema": {"type": "object"}},
        {"name": "computer_launch_app", "description": "启动应用（launch_path/path/aumid/bundle_id/name/urls，支持 additional_arguments/start_minimized），尽量不抢用户焦点", "inputSchema": {"type": "object"}},
        {"name": "computer_kill_app", "description": "按 pid 强制终止进程（taskkill /F 等效）", "inputSchema": {"type": "object", "required": ["pid"]}},
        {"name": "computer_bring_to_front", "description": "恢复最小化窗口并置前激活（返回 previous_fg_hwnd 与 now_fg_hwnd）", "inputSchema": {"type": "object", "required": ["pid"]}},
        {"name": "computer_set_window_frame", "description": "移动并调整非最小化/最大化窗口的位置与尺寸，返回调整后可见边界", "inputSchema": {"type": "object", "required": ["pid", "window_id", "x", "y", "width", "height"]}},
        {"name": "computer_invoke_menu", "description": "沿 path 路径逐层触发原生窗口 UIA 菜单项", "inputSchema": {"type": "object", "required": ["pid", "window_id", "path"]}},
        {"name": "computer_get_window_state", "description": "获取目标窗口 UIA 树（含 snapshot_id 与 element_token）以及内存画面经 omni_parser 识别的结构化文字与图标框坐标（绝不返回直接截图）", "inputSchema": {"type": "object", "required": ["pid", "window_id"]}},
        {"name": "computer_get_desktop_state", "description": "获取主桌面尺寸、可见窗口摘要以及全桌面画面经 omni_parser 识别的结构化文字与图标框屏幕坐标（绝不返回直接截图）", "inputSchema": {"type": "object"}},
        {"name": "computer_debug_window_info", "description": "诊断目标 pid 全部顶层窗类名/标题/边界及 UIA 焦点控件与支持的 Pattern", "inputSchema": {"type": "object", "required": ["pid"]}},
        {"name": "computer_get_accessibility_tree", "description": "快速返回桌面运行中 GUI 进程与可见顶层窗口发现快照", "inputSchema": {"type": "object"}},
        {"name": "computer_get_screen_size", "description": "返回主显示器物理像素尺寸与 DPI 缩放倍率", "inputSchema": {"type": "object"}},
        {"name": "computer_get_cursor_position", "description": "返回当前鼠标指针的屏幕像素坐标", "inputSchema": {"type": "object"}}
    ])
}
pub async fn call(uia: &UiaHandle, name: &str, args: Map<String, Value>) -> Value { // 统一调度 23 个 computer_* 工具
    match name {
        "computer_click" => actions::click_common(uia, &args, None, None).await,
        "computer_double_click" => actions::click_common(uia, &args, Some("left"), Some(2)).await,
        "computer_right_click" => actions::click_common(uia, &args, Some("right"), Some(1)).await,
        "computer_drag" => actions::drag(&args).await,
        "computer_type_text" => actions::type_text(uia, &args).await,
        "computer_press_key" => actions::press_key(uia, &args).await,
        "computer_hotkey" => actions::hotkey(uia, &args).await,
        "computer_scroll" => actions::scroll(uia, &args).await,
        "computer_set_value" => actions::set_value(uia, &args).await,
        "computer_move_cursor" => actions::move_cursor(&args),
        "computer_list_apps" => apps::list_apps(&args),
        "computer_list_windows" => apps::list_windows(&args),
        "computer_launch_app" => apps::launch_app(&args),
        "computer_kill_app" => apps::kill_app(&args),
        "computer_bring_to_front" => apps::bring_to_front(&args),
        "computer_set_window_frame" => apps::set_window_frame(&args),
        "computer_invoke_menu" => actions::invoke_menu(uia, &args).await,
        "computer_get_window_state" => state_tools::get_window_state(uia, &args).await,
        "computer_get_desktop_state" => state_tools::get_desktop_state(&args).await,
        "computer_debug_window_info" => apps::debug_window_info(uia, &args).await,
        "computer_get_accessibility_tree" => apps::get_accessibility_tree(),
        "computer_get_screen_size" => apps::get_screen_size(),
        "computer_get_cursor_position" => apps::get_cursor_position(),
        _ => json!({"ok": false, "error": format!("未知工具（仅支持 23 个对齐工具）：{name}")}),
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;
    #[test]
    fn definitions_match_exact_twenty_three_aligned_tools() {
        let list = definitions();
        let arr = list.as_array().expect("definitions must be array");
        assert_eq!(arr.len(), 23);
        let names: HashSet<&str> = arr.iter().filter_map(|item| item.get("name").and_then(Value::as_str)).collect();
        assert_eq!(names.len(), 23);
        for old_suffix in ["grounding", "get_app_state", "perform_action", "get_value", "select_text"] {
            let old = format!("computer_{old_suffix}");
            assert!(!names.contains(old.as_str()), "legacy tool {old} must be removed");
        }
        for req in ["computer_double_click", "computer_right_click", "computer_hotkey", "computer_move_cursor", "computer_list_windows", "computer_kill_app", "computer_bring_to_front", "computer_set_window_frame", "computer_invoke_menu", "computer_get_window_state", "computer_get_desktop_state", "computer_debug_window_info", "computer_get_accessibility_tree", "computer_get_screen_size", "computer_get_cursor_position"] {
            assert!(names.contains(req), "missing aligned tool {req}");
        }
    }
    #[test]
    fn desktop_state_never_exposes_screenshot_out_file() {
        let raw = definitions().to_string();
        assert!(!raw.contains("screenshot_out_file"));
        assert!(!raw.contains("image_png"));
    }
}