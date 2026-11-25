use serde_json::{json, Value}; // 移动端工具：28 个 mobile_* 统一走 utils/mobile.rs 命名管道
use crate::agent::tools::{blocking, params, ToolDef}; // 工具定义与同步包装
use crate::utils::mobile; // 管道客户端
fn spec_defs() -> Vec<(&'static str, &'static str, Value, f64)> { // 28 工具规格：名称/描述/参数/超时
    vec![
        ("mobile_list_available_devices", "列出已授权的 Android 设备（serial/model）", params(&[]), 30.0), // 设备列表
        ("mobile_list_apps", "列出已安装应用包名；includeSystem=true 时含系统应用", params(&[("includeSystem", "boolean", "可选，是否包含系统应用")]), 60.0), // 应用列表
        ("mobile_get_foreground_app", "读取当前前台应用组件与包名", params(&[]), 30.0), // 前台应用
        ("mobile_launch_app", "按包名启动或激活应用", params(&[("app", "string", "应用包名")]), 60.0), // 启动应用
        ("mobile_terminate_app", "按包名强制停止应用", params(&[("app", "string", "应用包名")]), 30.0), // 停止应用
        ("mobile_install_app", "安装本地 APK（完整路径）", params(&[("path", "string", "APK 文件路径")]), 180.0), // 安装
        ("mobile_uninstall_app", "卸载指定包名应用", params(&[("app", "string", "应用包名")]), 60.0), // 卸载
        ("mobile_get_screen_size", "读取设备物理分辨率与密度", params(&[]), 30.0), // 屏幕尺寸
        ("mobile_click_on_screen_at_coordinates", "点击坐标或元素引用 ref=@eN", params(&[("x", "integer", "可选，X 坐标"), ("y", "integer", "可选，Y 坐标"), ("ref", "string", "可选，元素引用 @eN")]), 30.0), // 点击
        ("mobile_double_tap_on_screen", "双击坐标或元素引用", params(&[("x", "integer", "可选，X 坐标"), ("y", "integer", "可选，Y 坐标"), ("ref", "string", "可选，元素引用 @eN")]), 30.0), // 双击
        ("mobile_long_press_on_screen_at_coordinates", "长按坐标或元素引用", params(&[("x", "integer", "可选，X 坐标"), ("y", "integer", "可选，Y 坐标"), ("ref", "string", "可选，元素引用 @eN"), ("duration", "integer", "可选，时长毫秒，默认 800")]), 30.0), // 长按
        ("mobile_list_elements_on_screen", "抓取 uiautomator 元素树，返回带 @eN 引用与 bounds", params(&[]), 60.0), // 元素树
        ("mobile_press_button", "发送按键：home/back/enter/power/menu/recent/volume_up/volume_down", params(&[("button", "string", "按键名")]), 30.0), // 按键
        ("mobile_open_url", "用系统浏览器打开网址", params(&[("url", "string", "网址")]), 30.0), // 打开网址
        ("mobile_swipe_on_screen", "滑动：可传 direction 或 x1/y1/x2/y2", params(&[("direction", "string", "可选，up/down/left/right"), ("x1", "integer", "可选，起点 X"), ("y1", "integer", "可选，起点 Y"), ("x2", "integer", "可选，终点 X"), ("y2", "integer", "可选，终点 Y"), ("duration", "integer", "可选，时长毫秒")]), 40.0), // 滑动
        ("mobile_type_keys", "输入文本；submit=true 时回车提交（非 ASCII 走剪贴板 best-effort）", params(&[("text", "string", "要输入的文本"), ("submit", "boolean", "可选，是否回车提交")]), 40.0), // 输入
        ("mobile_save_screenshot", "截图并落盘到 static/data/mobile_screen", params(&[("filename", "string", "可选，文件名")]), 60.0), // 截图落盘
        ("mobile_take_screenshot", "截图并返回 base64", params(&[]), 60.0), // 截图
        ("mobile_set_orientation", "设置屏幕方向：portrait/landscape", params(&[("orientation", "string", "portrait/landscape")]), 30.0), // 设置方向
        ("mobile_set_location", "模拟器设置经纬度（adb emu geo fix）", params(&[("latitude", "number", "纬度"), ("longitude", "number", "经度")]), 30.0), // 设置位置
        ("mobile_clipboard", "剪贴板读写：action=get/set", params(&[("action", "string", "get/set"), ("text", "string", "可选，写入文本")]), 30.0), // 剪贴板
        ("mobile_get_device_logs", "读取最近 logcat 行", params(&[("lines", "integer", "可选，行数，默认 200")]), 40.0), // 日志
        ("mobile_get_orientation", "读取当前屏幕方向", params(&[]), 30.0), // 读取方向
        ("mobile_start_screen_recording", "开始录屏（默认 180s 上限）", params(&[("timeLimit", "integer", "可选，秒数")]), 30.0), // 开始录屏
        ("mobile_stop_screen_recording", "停止录屏并拉取 mp4", params(&[]), 150.0), // 停止录屏
        ("mobile_list_crashes", "列出 crash 缓冲区中的崩溃摘要", params(&[]), 40.0), // 崩溃列表
        ("mobile_get_crash", "读取某次崩溃详情", params(&[("index", "integer", "可选，崩溃序号")]), 60.0), // 崩溃详情
        ("mobile_batch_commands", "批量执行 mobile_* 工具（≤20 条）", params(&[("commands", "string", "命令数组（JSON 字符串）")]), 180.0), // 批量
    ]
}
pub fn build_mobile_tools() -> Vec<ToolDef> { // 构造 28 个移动端工具，全部归入 mobile 审批组
    spec_defs().into_iter().map(|(name, description, parameters, timeout)| { // 逐规格生成
        let tool_name = name.to_string(); // 克隆工具名
        ToolDef::new(name, description, parameters, blocking(move |args: String| { // 构造处理器
            let value: Value = serde_json::from_str(&args).unwrap_or_else(|_| json!({})); // 解析参数
            let result = mobile::call(&tool_name, &value, timeout); // 调用 sidecar
            if result.get("ok").and_then(Value::as_bool).unwrap_or(false) { // 成功
                result.get("data").cloned().unwrap_or(Value::Null).to_string() // 返回数据
            } else { // 失败
                json!({"ok": false, "error": result.get("error").and_then(Value::as_str).unwrap_or("移动端调用失败")}).to_string() // 返回错误
            }
        })).with_group("mobile") // 归入移动端审批组
    }).collect() // 汇总
}