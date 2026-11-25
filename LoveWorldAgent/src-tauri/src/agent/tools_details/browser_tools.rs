use serde_json::{json, Map, Value}; // 内置浏览器工具：统一走 utils/browser.rs 的管道客户端，名称与参数对齐 Python 版 browser_*
use crate::agent::tools::{params, blocking, ToolDef}; // 工具定义与同步包装
use crate::utils::browser; // 浏览器管道客户端
fn put_snapshot(map: &mut Map<String, Value>, value: &Value) { // 映射可选 include_snapshot 参数
    if let Some(item) = value.get("include_snapshot").or_else(|| value.get("includeSnapshot")) { map.insert("includeSnapshot".to_string(), item.clone()); }
}
fn mapping(name: &str, value: &Value) -> Value { // 把工具参数名映射为 Python 后端字段名
    let mut map = Map::new(); // 映射结果
    let put = |map: &mut Map<String, Value>, key: &str, source: &str| { if let Some(item) = value.get(source) { map.insert(key.to_string(), item.clone()); } }; // 存在则复制字段
    match name { // 按工具名分派映射规则
        "browser_navigate" => { put(&mut map, "url", "url"); put_snapshot(&mut map, value); } // 导航
        "browser_tabs" => { put(&mut map, "action", "action"); put(&mut map, "index", "index"); put(&mut map, "url", "url"); } // 标签页操作
        "browser_take_screenshot" => { put(&mut map, "type", "type"); put(&mut map, "fullPage", "full_page"); put(&mut map, "filename", "filename"); put(&mut map, "ref", "ref"); } // 截图
        "browser_click" => { put(&mut map, "ref", "ref"); put(&mut map, "doubleClick", "double_click"); put(&mut map, "button", "button"); put(&mut map, "modifiers", "modifiers"); put_snapshot(&mut map, value); } // 点击
        "browser_type" => { put(&mut map, "ref", "ref"); put(&mut map, "text", "text"); put(&mut map, "slowly", "slowly"); put(&mut map, "submit", "submit"); put_snapshot(&mut map, value); } // 输入
        "browser_select_option" => { put(&mut map, "ref", "ref"); put(&mut map, "values", "values"); put_snapshot(&mut map, value); } // 选择下拉项
        "browser_press_key" => { put(&mut map, "key", "key"); put_snapshot(&mut map, value); } // 按键
        "browser_get_attribute" => { put(&mut map, "ref", "ref"); put(&mut map, "attribute", "attribute"); } // 取属性
        "browser_scroll" => { put(&mut map, "direction", "direction"); put(&mut map, "amount", "amount"); put(&mut map, "ref", "ref"); } // 滚动
        "browser_wait_for" => { put(&mut map, "time", "time"); put(&mut map, "text", "text"); put(&mut map, "textGone", "text_gone"); } // 等待
        "browser_evaluate" => { put(&mut map, "function", "function"); put(&mut map, "ref", "ref"); } // 执行 JS
        "browser_handle_dialog" => { put(&mut map, "accept", "accept"); put(&mut map, "promptText", "prompt_text"); } // 处理弹窗
        "browser_console_messages" => { put(&mut map, "level", "level"); put(&mut map, "onlyErrors", "only_errors"); } // 控制台日志
        "browser_network_requests" => { put(&mut map, "includeStatic", "include_static"); put(&mut map, "filter", "filter"); } // 网络请求
        _ => { if let Some(entries) = value.as_object() { for (key, item) in entries { map.insert(key.clone(), item.clone()); } } } // 其他工具原样透传
    }
    Value::Object(map) // 返回映射后的参数
}
pub fn build_browser_tools() -> Vec<ToolDef> { // 构造全部浏览器工具
    let specs: Vec<(&str, &str, Value, f64)> = vec![ // 工具规格：名称/描述/参数/超时
        ("browser_open", "在内置浏览器打开一个网址（启动或复用内置浏览器窗口），返回启动与页面状态；后续交互前先用 browser_snapshot 取 ref", params(&[("url", "string", "要打开的网址")]), 25.0), // 打开网址
        ("browser_navigate", "导航当前标签页到指定网址（http/https/file 均可），返回页面状态", params(&[("url", "string", "目标网址"), ("include_snapshot", "boolean", "可选，操作后是否等待页面稳定并附带最新快照（默认 false）")]), 25.0), // 导航
        ("browser_navigate_back", "后退到上一个页面", params(&[]), 20.0), // 后退
        ("browser_tabs", "标签页操作：list/new/close/select；new 可带 url，close/select 带 index", params(&[("action", "string", "list/new/close/select"), ("index", "integer", "可选，标签下标"), ("url", "string", "可选，新建标签的网址")]), 20.0), // 标签页管理
        ("browser_snapshot", "获取当前页面结构化快照（含可交互元素 ref）；点击/输入/悬停前必须先取快照，页面结构变化后旧 ref 失效", params(&[]), 25.0), // 页面快照
        ("browser_read_text", "读取当前页面的纯文本内容", params(&[]), 20.0), // 读取文本
        ("browser_take_screenshot", "截取当前页面（或指定 ref 元素）并落盘，返回图片绝对路径", params(&[("filename", "string", "可选，文件名"), ("type", "string", "可选，png/jpeg"), ("full_page", "boolean", "可选，整页截图"), ("ref", "string", "可选，元素 ref")]), 25.0), // 截图
        ("browser_click", "按快照中的元素 ref 点击（ref 形如 e12，来自 browser_snapshot）；页面变化后旧 ref 失效需重新快照", params(&[("ref", "string", "元素 ref"), ("double_click", "boolean", "可选，双击"), ("button", "string", "可选，left/right/middle"), ("modifiers", "string_array", "可选，修饰键"), ("include_snapshot", "boolean", "可选，操作后是否等待页面稳定并附带最新快照（默认 false）")]), 20.0), // 点击元素
        ("browser_hover", "悬停到快照中的元素 ref；需先 browser_snapshot", params(&[("ref", "string", "元素 ref")]), 20.0), // 悬停
        ("browser_type", "向快照中的输入框 ref 写入文本；submit 为输入后回车提交，slowly 为逐字符输入", params(&[("ref", "string", "元素 ref"), ("text", "string", "输入文本"), ("slowly", "boolean", "可选，逐字输入"), ("submit", "boolean", "可选，输入后回车"), ("include_snapshot", "boolean", "可选，操作后是否等待页面稳定并附带最新快照（默认 false）")]), 20.0), // 输入文本
        ("browser_select_option", "选择下拉框 ref 的选项，values 为选项文本或 value 列表", params(&[("ref", "string", "元素 ref"), ("values", "string_array", "选项列表"), ("include_snapshot", "boolean", "可选，操作后是否等待页面稳定并附带最新快照（默认 false）")]), 20.0), // 选择选项
        ("browser_press_key", "向页面发送一个按键（如 Enter/Escape/ArrowUp/Tab 等）", params(&[("key", "string", "按键名"), ("include_snapshot", "boolean", "可选，操作后是否等待页面稳定并附带最新快照（默认 false）")]), 20.0), // 发送按键
        ("browser_get_attribute", "读取快照中元素 ref 的属性值（如 href/class/value）", params(&[("ref", "string", "元素 ref"), ("attribute", "string", "属性名")]), 20.0), // 读属性
        ("browser_scroll", "滚动页面，direction 为 up/down/left/right，amount 为像素；带 ref 时滚动该元素内部", params(&[("direction", "string", "滚动方向"), ("amount", "integer", "可选，像素数"), ("ref", "string", "可选，元素 ref")]), 20.0), // 滚动
        ("browser_wait_for", "等待页面：time 秒数、text 出现、text_gone 消失三者择一；页面内最长等 30 秒", params(&[("time", "number", "可选，等待秒数"), ("text", "string", "可选，等待出现的文本"), ("text_gone", "string", "可选，等待消失的文本")]), 35.0), // 等待条件
        ("browser_waiting_for_user_interaction", "遇到登录、验证码等需人工判断的场景，唤起浏览器窗口并把控制权交还用户", params(&[("reason", "string", "交还用户的原因")]), 20.0), // 交还控制权
        ("browser_evaluate", "在当前页面执行一段 JS 函数体（需 return 结果）；带 ref 时在该元素上下文执行", params(&[("function", "string", "JS 函数体"), ("ref", "string", "可选，元素 ref")]), 25.0), // 执行脚本
        ("browser_handle_dialog", "应答当前弹窗：accept 为确认/取消，prompt_text 为 prompt 弹窗的输入内容", params(&[("accept", "boolean", "可选，默认 true 确认"), ("prompt_text", "string", "可选，prompt 输入")]), 20.0), // 处理弹窗
        ("browser_console_messages", "读取页面控制台日志；level 可过滤 error/warning/info/debug", params(&[("level", "string", "可选，日志级别"), ("only_errors", "boolean", "可选，只看错误")]), 20.0), // 控制台日志
        ("browser_network_requests", "读取网络请求；首次调用会开启监听，需再次调用才能取到记录", params(&[("include_static", "boolean", "可选，包含静态资源"), ("filter", "string", "可选，过滤词")]), 20.0), // 网络请求
        ("browser_state", "读取内置浏览器当前状态（运行/标题/地址/加载/可前后退/标签列表）", params(&[]), 20.0), // 浏览器状态
    ];
    specs.into_iter().map(|(name, description, parameters, timeout)| { // 逐规格生成工具
        let tool_name = name.to_string(); // 克隆工具名
        ToolDef::new(name, description, parameters, blocking(move |args: String| { // 构造工具处理器
            let value: Value = serde_json::from_str(&args).unwrap_or_else(|_| json!({})); // 解析参数
            let manager = browser::manager(); // 取浏览器管理器
            if tool_name == "browser_state" { return manager.snapshot().to_string(); } // 状态查询直接返回快照
            if tool_name == "browser_open" { // 打开网址走专用启动逻辑
                let url = value.get("url").and_then(Value::as_str).unwrap_or_default().to_string(); // 目标网址
                let result = manager.start((0.0, 0.0, 1280.0, 900.0), &url, false); // 启动内置浏览器
                let ok = result.get("ok").and_then(Value::as_bool).unwrap_or(false);
                let reused = result.get("reused").and_then(Value::as_bool).unwrap_or(false);
                if ok && !reused {
                    manager.wait_initial_ready(!url.trim().is_empty());
                }
                return json!({"ok": ok, "state": manager.snapshot()}).to_string(); // 返回结果与状态
            }
            let mapped = mapping(&tool_name, &value); // 映射参数名
            manager.tool(&tool_name, &mapped, timeout).to_string() // 调用浏览器工具并序列化结果
        })).with_group("browser") // 归入浏览器审批组
    }).collect() // 收集全部工具
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn mapping_include_snapshot_all_supported_tools() {
        for name in ["browser_navigate", "browser_click", "browser_type", "browser_select_option", "browser_press_key"] {
            let mapped = mapping(name, &json!({"url": "https://example.com", "ref": "e1", "text": "hi", "values": ["a"], "key": "Enter", "include_snapshot": true}));
            assert_eq!(mapped.get("includeSnapshot"), Some(&json!(true)), "tool={name}");
        }
        let camel = mapping("browser_click", &json!({"ref": "e2", "includeSnapshot": true}));
        assert_eq!(camel.get("includeSnapshot"), Some(&json!(true)));
    }
}
