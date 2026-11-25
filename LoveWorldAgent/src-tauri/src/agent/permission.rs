use once_cell::sync::Lazy; // 权限策略：四组定义与逐条开关、custom 白名单、审批判定与中文理由
use parking_lot::Mutex; // 写操作互斥锁
use serde_json::{json, Map, Value}; // JSON 构造与对象映射
use crate::config; // 权限文件路径与默认配置
use crate::state::as_bool; // 布尔容错读取
pub const MODES: [&str; 3] = ["manual", "auto", "full"]; // 三种审批模式
pub const BROWSER_TOOLS: [&str; 21] = ["browser_open", "browser_navigate", "browser_navigate_back", "browser_tabs", "browser_snapshot", "browser_read_text", "browser_take_screenshot", "browser_click", "browser_hover", "browser_type", "browser_select_option", "browser_press_key", "browser_get_attribute", "browser_scroll", "browser_wait_for", "browser_waiting_for_user_interaction", "browser_evaluate", "browser_handle_dialog", "browser_console_messages", "browser_network_requests", "browser_state"]; // 内置浏览器工具清单
pub const POWERSHELL_RISKS: [&str; 19] = ["format C:", "diskpart", "del /f /s /q", "rm -rf *", "git push --force", "IEX DownloadString", "Add-MpPreference -ExclusionPath", "taskkill", "reg delete", "Invoke-Expression DownloadString", "Remove-Item -Recurse -Force", "Set-ExecutionPolicy Bypass", "Start-Process Hidden", "vssadmin delete shadows", "wbadmin delete catalog", "bcdedit /delete", "net user /add", "cipher /w", "takeown /f"]; // 高风险命令默认需批准
pub const COMPUTER_TOOLS: [&str; 23] = ["computer_click", "computer_double_click", "computer_right_click", "computer_drag", "computer_type_text", "computer_press_key", "computer_hotkey", "computer_scroll", "computer_set_value", "computer_move_cursor", "computer_list_apps", "computer_list_windows", "computer_launch_app", "computer_kill_app", "computer_bring_to_front", "computer_set_window_frame", "computer_invoke_menu", "computer_get_window_state", "computer_get_desktop_state", "computer_debug_window_info", "computer_get_accessibility_tree", "computer_get_screen_size", "computer_get_cursor_position"]; // 桌面自动化工具清单（对齐 23 工具）
pub const WORD_TOOLS: [&str; 12] = ["word_document", "word_read", "word_write", "word_table", "word_format", "word_structure", "word_find", "word_edit", "word_media", "word_variable", "word_batch", "word_where_am_i"]; // Word 操作工具清单（对齐 12 工具）
pub const PPT_TOOLS: [&str; 10] = ["ppt_presentation", "ppt_read", "ppt_slide", "ppt_write", "ppt_shape", "ppt_table", "ppt_format", "ppt_chart", "ppt_media", "ppt_where_am_i"]; // PPT 操作工具清单
pub const EXCEL_TOOLS: [&str; 10] = ["excel_workbook", "excel_read", "excel_write", "excel_sheet", "excel_format", "excel_formula", "excel_table", "excel_chart", "excel_sort_filter", "excel_where_am_i"]; // Excel 操作工具清单
pub const MOBILE_TOOLS: [&str; 28] = ["mobile_list_available_devices", "mobile_list_apps", "mobile_get_foreground_app", "mobile_launch_app", "mobile_terminate_app", "mobile_install_app", "mobile_uninstall_app", "mobile_get_screen_size", "mobile_click_on_screen_at_coordinates", "mobile_double_tap_on_screen", "mobile_long_press_on_screen_at_coordinates", "mobile_list_elements_on_screen", "mobile_press_button", "mobile_open_url", "mobile_swipe_on_screen", "mobile_type_keys", "mobile_save_screenshot", "mobile_take_screenshot", "mobile_set_orientation", "mobile_set_location", "mobile_clipboard", "mobile_get_device_logs", "mobile_get_orientation", "mobile_start_screen_recording", "mobile_stop_screen_recording", "mobile_list_crashes", "mobile_get_crash", "mobile_batch_commands"]; // 移动端工具清单
pub const GROUP_ORDER: [&str; 8] = ["mcp", "powershell", "browser", "computer", "word", "ppt", "excel", "mobile"]; // 权限组展示顺序
static LOCK: Lazy<Mutex<()>> = Lazy::new(|| Mutex::new(())); // 写权限文件的全局锁
fn fallback_groups() -> Value { // 无配置时的兜底权限组定义
    let mut powershell = Map::new(); // PowerShell 命令表
    for item in POWERSHELL_RISKS { powershell.insert(item.to_string(), json!(false)); } // 高风险命令默认不批准
    let mut browser = Map::new(); // 浏览器工具表
    let mut computer = Map::new(); // 电脑控制工具表
    let mut word = Map::new(); // Word 操作工具表
    let mut ppt = Map::new(); // PPT 操作工具表
    let mut excel = Map::new(); // Excel 操作工具表
    let mut mobile = Map::new(); // 移动端工具表
    for item in COMPUTER_TOOLS { computer.insert(item.to_string(), json!(true)); } // 电脑控制工具默认允许
    for item in BROWSER_TOOLS { browser.insert(item.to_string(), json!(true)); } // 浏览器工具默认允许
    for item in WORD_TOOLS { word.insert(item.to_string(), json!(true)); } // Word 工具默认允许
    for item in PPT_TOOLS { ppt.insert(item.to_string(), json!(true)); } // PPT 工具默认允许
    for item in EXCEL_TOOLS { excel.insert(item.to_string(), json!(true)); } // Excel 工具默认允许
    for item in MOBILE_TOOLS { mobile.insert(item.to_string(), json!(true)); } // 移动端工具默认允许
    json!({
        "mcp": {"label": "MCP工具", "enabled": true, "commands": {}}, // MCP 组默认启用
        "powershell": {"label": "PowerShell", "enabled": true, "commands": powershell}, // 命令组含高风险项
        "browser": {"label": "内置浏览器", "enabled": true, "commands": browser}, // 浏览器组默认允许
        "computer": {"label": "电脑控制", "enabled": true, "commands": computer}, // 电脑控制组默认允许 23 个 computer_*
        "word": {"label": "Word操作", "enabled": true, "commands": word}, // Word 组默认允许 12 个 word_*
        "ppt": {"label": "PPT操作", "enabled": true, "commands": ppt}, // PPT 组默认允许 10 个 ppt_*
        "excel": {"label": "Excel操作", "enabled": true, "commands": excel}, // Excel 组默认允许 10 个 excel_*
        "mobile": {"label": "移动端", "enabled": true, "commands": mobile}, // 移动端组默认允许 28 个 mobile_*
    })
}
pub fn squash(value: &str) -> String { // 压缩空白为单空格
    value.split_whitespace().collect::<Vec<&str>>().join(" ") // 按空白切分后拼接
}
fn read_raw() -> Value { // 读取权限配置原文
    let Some(path) = config::resolve_permission_file() else { return json!({}); }; // 路径解析失败按空处理
    let Ok(text) = std::fs::read_to_string(path) else { return json!({}); }; // 读取失败按空处理
    match serde_json::from_str::<Value>(&text) { Ok(value) if value.is_object() => value, _ => json!({}) } // 仅接受对象，否则空
}
fn definition() -> Map<String, Value> { // 读取权限组定义
    let raw = read_raw(); // 原始配置
    let mut defs: Map<String, Value> = raw.get("groups").and_then(Value::as_object).map(|map| map.iter().filter(|(_, value)| value.is_object()).map(|(key, value)| (key.clone(), value.clone())).collect()).unwrap_or_default(); // 只保留对象型组定义
    let mut computer = Map::new(); // computer 组强制按 23 工具重建
    for item in COMPUTER_TOOLS { computer.insert(item.to_string(), json!(true)); } // 23 项默认全开
    defs.insert("computer".to_string(), json!({"label": "电脑控制", "enabled": true, "commands": computer})); // 覆盖文件中的旧 computer 定义
    let mut word = Map::new(); // word 组强制按 12 工具重建，防止旧配置缺条目
    for item in WORD_TOOLS { word.insert(item.to_string(), json!(true)); } // 12 项默认全开
    defs.insert("word".to_string(), json!({"label": "Word操作", "enabled": true, "commands": word})); // 覆盖文件中的旧 word 定义
    let mut ppt = Map::new(); // ppt 组按 10 工具重建
    for item in PPT_TOOLS { ppt.insert(item.to_string(), json!(true)); } // 10 项默认全开
    defs.insert("ppt".to_string(), json!({"label": "PPT操作", "enabled": true, "commands": ppt})); // 覆盖旧定义
    let mut excel = Map::new(); // excel 组按 10 工具重建
    for item in EXCEL_TOOLS { excel.insert(item.to_string(), json!(true)); } // 10 项默认全开
    defs.insert("excel".to_string(), json!({"label": "Excel操作", "enabled": true, "commands": excel})); // 覆盖旧定义
    let mut mobile = Map::new(); // mobile 组按 28 工具重建
    for item in MOBILE_TOOLS { mobile.insert(item.to_string(), json!(true)); } // 28 项默认全开
    defs.insert("mobile".to_string(), json!({"label": "移动端", "enabled": true, "commands": mobile})); // 覆盖旧定义
    let mut mcp_cmds = Map::new(); // MCP 服务级开关表（按 agent_mcp.json 全部服务生成，默认全部放行）
    for srv in crate::agent::mcp::load_servers() {
        let srv_name = crate::agent::mcp::normalize(&srv).get("name").and_then(Value::as_str).unwrap_or_default().trim().to_string();
        if !srv_name.is_empty() { mcp_cmds.insert(srv_name, json!(true)); }
    }
    defs.insert("mcp".to_string(), json!({"label": "MCP工具", "enabled": true, "commands": mcp_cmds})); // 按服务名聚合 MCP 权限
    defs // 返回组定义
}
fn entries_of(definition: &Value) -> Vec<String> { // 提取组定义的条目名
    definition.get("commands").and_then(Value::as_object).map(|map| map.keys().cloned().collect()).unwrap_or_default() // 取命令键列表
}
pub fn labels() -> Value { // 生成组名到显示标签的映射
    let data = load(); // 加载权限配置
    let mut map = Map::new(); // 标签映射
    if let Some(groups) = data.get("groups").and_then(Value::as_object) { // 遍历权限组
        for (key, group) in groups { map.insert(key.clone(), json!(group.get("label").and_then(Value::as_str).unwrap_or(key))); } // 缺省用键名
    }
    Value::Object(map) // 返回映射
}
fn normalize_group(raw: Option<&Value>, definition: &Value, label: &str) -> Value { // 归一化单个权限组配置
    let default_enabled = as_bool(definition.get("enabled"), true); // 组默认启用状态
    let defaults = definition.get("commands").and_then(Value::as_object); // 默认条目开关
    let entries = entries_of(definition); // 条目名列表
    let mut commands = Map::new(); // 归一化后的条目开关
    if let Some(map) = raw.and_then(Value::as_object) { // 新格式：对象含 enabled/commands
        let enabled = as_bool(map.get("enabled"), default_enabled); // 组启用状态
        let raw_commands = map.get("commands").and_then(Value::as_object); // 用户配置的条目开关
        for entry in entries { // 逐条目归一化
            let fallback = defaults.and_then(|values| values.get(&entry)).map(|value| as_bool(Some(value), true)).unwrap_or(true); // 缺省用定义值
            let value = raw_commands.and_then(|values| values.get(&entry)).map(|value| as_bool(Some(value), fallback)).unwrap_or(fallback); // 用户值优先
            commands.insert(entry, json!(value)); // 写入条目开关
        }
        return json!({"label": label, "enabled": enabled, "commands": commands}); // 返回新格式
    }
    let legacy = raw.map(|value| as_bool(Some(value), default_enabled)).unwrap_or(default_enabled); // 旧格式：布尔启用值
    for entry in entries { // 逐条目归一化
        let base = defaults.and_then(|values| values.get(&entry)).map(|value| as_bool(Some(value), true)).unwrap_or(true); // 缺省用定义值
        commands.insert(entry, json!(legacy && base)); // 组禁用则条目全禁用
    }
    json!({"label": label, "enabled": legacy, "commands": commands}) // 返回旧格式归一化结果
}
pub fn normalize(raw: &Value) -> Value { // 归一化整份权限配置
    let mode = raw.get("mode").and_then(Value::as_str).map(|text| text.trim().to_lowercase()).unwrap_or_default(); // 审批模式
    let groups_raw = raw.get("groups").and_then(Value::as_object); // 用户配置的权限组
    let mut definitions = definition(); // 组定义
    if definitions.is_empty() { // 配置无定义时从原始组推断
        if let Some(map) = groups_raw { definitions = map.iter().filter(|(_, value)| value.is_object()).map(|(key, value)| (key.clone(), value.clone())).collect(); }
    }
    if definitions.is_empty() { definitions = fallback_groups().as_object().cloned().unwrap_or_default(); } // 仍为空用兜底定义
    let mut groups = Map::new(); // 归一化后的权限组
    for (key, definition) in definitions { // 逐组归一化
        let label = definition.get("label").and_then(Value::as_str).filter(|text| !text.is_empty()).unwrap_or(&key).to_string(); // 显示标签
        groups.insert(key.clone(), normalize_group(groups_raw.and_then(|map| map.get(&key)), &definition, &label)); // 写入归一化组
    }
    let mut custom: Vec<Value> = Vec::new(); // 自定义白名单
    if let Some(items) = raw.get("custom").and_then(Value::as_array) { // 遍历白名单条目
        for item in items { // 逐条压缩空白
            let text = squash(&crate::state::json_str(Some(item))); // 转为文本
            if !text.is_empty() { custom.push(Value::String(text)); } // 非空才保留
        }
    }
    let default_mode = config::DEFAULT_PERMISSION.get("mode").and_then(Value::as_str).unwrap_or("auto"); // 缺省模式
    json!({"mode": if MODES.contains(&mode.as_str()) { mode } else { default_mode.to_string() }, "groups": groups, "custom": custom}) // 组装最终配置
}
pub fn load() -> Value { // 加载权限配置，缺失用默认
    let raw = read_raw(); // 读取原文
    if raw.as_object().map(|map| !map.is_empty()).unwrap_or(false) { normalize(&raw) } else { normalize(&config::DEFAULT_PERMISSION) } // 非空归一化，否则用默认
}
fn write(permission: &Value) -> Result<(), String> { // 原子写入权限配置
    let path = &*config::WRITABLE_PERMISSION_FILE; // 可写配置路径
    if let Some(parent) = path.parent() { std::fs::create_dir_all(parent).map_err(|err| err.to_string())?; } // 确保父目录存在
    config::atomic_write_json(path, permission).map_err(|err| err.to_string()) // 原子写 JSON
}
pub fn save(input: &Value) -> Result<Value, String> { // 保存权限配置
    let data = normalize(input); // 先归一化
    let _guard = LOCK.lock(); // 持锁写盘
    write(&data)?; // 落盘
    Ok(data) // 返回归一化结果
}
fn wildcard_match(pattern: &str, text: &str) -> bool { // 支持 * 通配符的命令匹配
    let pattern_chars: Vec<char> = pattern.chars().collect(); // 模式字符
    let text_chars: Vec<char> = text.chars().collect(); // 文本字符
    let (mut pi, mut ti, mut star, mut mark) = (0usize, 0usize, None, 0usize); // 双指针与回溯位置
    while ti < text_chars.len() { // 遍历文本
        if pi < pattern_chars.len() && pattern_chars[pi] == text_chars[ti] { pi += 1; ti += 1; } // 字符相等同时前进
        else if pi < pattern_chars.len() && pattern_chars[pi] == '*' { star = Some(pi); pi += 1; mark = ti; } // 记录星号位置
        else if let Some(position) = star { pi = position + 1; mark += 1; ti = mark; } // 回溯尝试更长匹配
        else { return false; } // 无星号可回溯则失配
    }
    while pi < pattern_chars.len() && pattern_chars[pi] == '*' { pi += 1; } // 跳过尾部星号
    pi == pattern_chars.len() // 模式是否完全消耗
}
pub fn entry_matches(entry: &str, command: &str) -> bool { // 判断白名单条目是否命中命令
    let item = squash(entry).to_lowercase(); // 归一化条目
    if item.is_empty() { return false; } // 空条目不命中
    if item.contains('*') { return wildcard_match(&item, command); } // 含星号走通配匹配
    if item.chars().any(|ch| matches!(ch, ';' | '|' | '&' | '>' | '<')) { return command.contains(&item); } // 含 shell 元字符时子串匹配
    command == item || command.starts_with(&format!("{item} ")) || command.split(' ').next().unwrap_or_default() == item // 全等/前缀+空格/首词匹配
}
pub fn match_cmd(command: &str, state: &Value) -> Value { // 查找命令命中的权限组条目
    let text = squash(command).to_lowercase(); // 归一化命令
    let mut hits: Vec<Value> = Vec::new(); // 命中项
    if let Some(groups) = state.get("groups").and_then(Value::as_object) { // 遍历权限组
        for (key, group) in groups { // 逐组
            if let Some(commands) = group.get("commands").and_then(Value::as_object) { // 取条目表
                for entry in commands.keys() { // 逐条目
                    if entry_matches(entry, &text) { hits.push(json!({"key": key, "entry": entry})); } // 命中则记录组与条目
                }
            }
        }
    }
    let custom = state.get("custom").and_then(Value::as_array).map(|items| items.iter().any(|item| entry_matches(item.as_str().unwrap_or_default(), &text))).unwrap_or(false); // 是否命中自定义白名单
    json!({"command": text, "hits": hits, "custom": custom}) // 返回匹配结果
}
fn entry_allowed(state: &Value, key: &str, entry: &str) -> bool { // 检查组内条目是否允许
    let group = state.get("groups").and_then(|groups| groups.get(key)); // 取权限组
    let enabled = group.and_then(|value| value.get("enabled")).and_then(Value::as_bool).unwrap_or(false); // 组启用状态
    let command = group.and_then(|value| value.get("commands")).and_then(|commands| commands.get(entry)).and_then(Value::as_bool).unwrap_or(false); // 条目开关
    enabled && command // 组与条目同时启用才允许
}
pub fn whitelisted(command: &str, state: Option<&Value>) -> bool { // 判断命令是否在白名单中
    let owned; // 持有临时加载的配置
    let data = match state { Some(value) => value, None => { owned = load(); &owned } }; // 未传状态则临时加载
    let found = match_cmd(command, data); // 匹配命令
    if found.get("custom").and_then(Value::as_bool).unwrap_or(false) { return true; } // 命中自定义白名单直接允许
    found.get("hits").and_then(Value::as_array).map(|hits| hits.iter().any(|hit| entry_allowed(data, hit.get("key").and_then(Value::as_str).unwrap_or_default(), hit.get("entry").and_then(Value::as_str).unwrap_or_default()))).unwrap_or(false) // 任一命中条目允许即为真
}
fn strip_stderr(text: &str) -> String { // 去掉命令中的 2>N 重定向以便判断输出重定向
    let chars: Vec<char> = text.chars().collect(); // 命令字符
    let mut output = String::new(); // 结果缓冲
    let mut index = 0usize; // 遍历位置
    while index < chars.len() { // 逐字符扫描
        if chars[index] == '>' && index + 2 < chars.len() && chars[index + 1] == '&' && chars[index + 2].is_ascii_digit() { // 识别 2>&1 形式
            while output.chars().last().map(|ch| ch.is_ascii_digit()).unwrap_or(false) { output.pop(); } // 回退已写的文件描述符数字
            let mut next = index + 2; // 跳到数字后
            while next < chars.len() && chars[next].is_ascii_digit() { next += 1; } // 跳过多位数字
            index = next; // 继续扫描
            continue; // 跳过本次输出
        }
        output.push(chars[index]); // 普通字符保留
        index += 1; // 前进
    }
    output // 返回清理后命令
}
pub fn has_redirect(text: &str) -> bool { // 判断命令是否含输出重定向
    strip_stderr(text).contains('>') // 清理 stderr 重定向后仍有 > 即为输出重定向
}
pub fn needs_approval(command: &str) -> bool { // 判断命令是否需要用户批准
    let data = load(); // 加载权限配置
    if data.get("mode").and_then(Value::as_str) == Some("full") { return false; } // 完全访问模式无需批准
    let text = squash(command).to_lowercase(); // 归一化命令
    let custom_items: Vec<String> = data.get("custom").and_then(Value::as_array).map(|items| items.iter().map(|item| crate::state::json_str(Some(item))).collect()).unwrap_or_default(); // 自定义白名单
    let custom_hit = custom_items.iter().any(|entry| entry_matches(entry, &text)); // 是否命中白名单
    if data.get("mode").and_then(Value::as_str) == Some("manual") { return !custom_hit; } // 手动模式除白名单外都需批准
    if has_redirect(&text) { return !custom_items.iter().any(|entry| entry_matches(entry, &text) && has_redirect(entry)); } // 输出重定向需专门白名单
    if custom_hit { return false; } // 自动模式命中白名单放行
    let group = data.get("groups").and_then(|groups| groups.get("powershell")); // PowerShell 组
    let group_enabled = group.and_then(|value| value.get("enabled")).and_then(Value::as_bool).unwrap_or(false); // 组启用状态
    if let Some(commands) = group.and_then(|value| value.get("commands")).and_then(Value::as_object) { // 遍历命令条目
        for (entry, enabled) in commands { // 逐条目
            if entry_matches(entry, &text) && !(group_enabled && enabled.as_bool().unwrap_or(false)) { return true; } // 命中未启用条目需批准
        }
    }
    false // 其余放行
}
pub fn tool_needs_approval(group_key: &str, tool_name: &str) -> bool { // 判断浏览器/MCP 工具是否需批准
    let data = load(); // 加载权限配置
    let mode = data.get("mode").and_then(Value::as_str).unwrap_or("auto"); // 审批模式
    if mode == "full" { return false; } // 完全访问模式无需批准
    if mode == "manual" { return true; } // 手动模式全部需批准
    let name = squash(tool_name).to_lowercase(); // 归一化工具名
    let custom_hit = data.get("custom").and_then(Value::as_array).map(|items| items.iter().any(|item| entry_matches(item.as_str().unwrap_or_default(), &name))).unwrap_or(false); // 是否命中白名单
    if custom_hit { return false; } // 命中白名单放行
    let group = data.get("groups").and_then(|groups| groups.get(group_key)); // 取工具组
    let commands = group.and_then(|value| value.get("commands")).and_then(Value::as_object); // 取工具开关表
    let Some(commands) = commands else { return false; }; // 无工具开关表则放行
    let group_enabled = group.and_then(|value| value.get("enabled")).and_then(Value::as_bool).unwrap_or(false); // 组启用状态
    if group_key == "mcp" { // MCP 按整个服务名管理：匹配 mcp_<sanitized_server>__<tool>
        for (server, val) in commands {
            let prefix = format!("mcp_{}__", crate::agent::mcp::sanitize_name(server)).to_lowercase();
            if name.starts_with(&prefix) { return !(group_enabled && val.as_bool().unwrap_or(false)); }
        }
        return !group_enabled;
    }
    if !commands.contains_key(tool_name) { return false; } // 工具未登记则放行
    let enabled = commands.get(tool_name).and_then(Value::as_bool).unwrap_or(false); // 工具开关
    !(group_enabled && enabled) // 组与工具都启用才放行
}
pub fn reason(command: &str) -> String { // 生成命令需批准的中文理由
    let data = load(); // 加载权限配置
    let mode = data.get("mode").and_then(Value::as_str).unwrap_or("auto"); // 审批模式
    if mode == "full" { return "完全访问：不检查白名单，直接在你的电脑上执行。".to_string(); } // 完全访问说明
    let prefix = if mode == "auto" { "自动审批" } else { "手动审批" }; // 理由前缀
    let text = squash(command).to_lowercase(); // 归一化命令
    if has_redirect(&text) { return format!("{prefix}：该命令含输出重定向（会写文件/改配置），需要你的批准。"); } // 重定向理由
    let group = data.get("groups").and_then(|groups| groups.get("powershell")); // PowerShell 组
    let group_enabled = group.and_then(|value| value.get("enabled")).and_then(Value::as_bool).unwrap_or(false); // 组启用状态
    if let Some(commands) = group.and_then(|value| value.get("commands")).and_then(Value::as_object) { // 遍历命令条目
        for (entry, enabled) in commands { // 逐条目
            if entry_matches(entry, &text) && !(group_enabled && enabled.as_bool().unwrap_or(false)) { // 命中未启用条目
                return format!("{prefix}：该命令命中 PowerShell 高风险项「{entry}」，需要你的批准。"); // 高风险理由
            }
        }
    }
    if mode == "manual" { return format!("{prefix}：手动审批下每一步操作都需要你的批准。"); } // 手动模式理由
    format!("{prefix}：该操作需要你的批准。") // 兜底理由
}
pub fn tool_reason(group_key: &str, tool_name: &str) -> String { // 生成工具需批准的中文理由
    let data = load(); // 加载权限配置
    let mode = data.get("mode").and_then(Value::as_str).unwrap_or("auto"); // 审批模式
    let subject = match group_key { "mcp" => "MCP 工具", "computer" => "电脑控制工具", "word" => "Word 操作工具", "ppt" => "PPT 操作工具", "excel" => "Excel 操作工具", "mobile" => "移动端工具", _ => "内置浏览器工具" }; // 主体名称
    if mode == "full" { return format!("完全访问：不检查白名单，直接执行{subject}。"); } // 完全访问说明
    let prefix = if mode == "auto" { "自动审批" } else { "手动审批" }; // 理由前缀
    if mode == "manual" { return format!("{prefix}：{subject}「{tool_name}」需要你的批准。"); } // 手动模式理由
    let label = data.get("groups").and_then(|groups| groups.get(group_key)).and_then(|group| group.get("label")).and_then(Value::as_str).unwrap_or(group_key); // 组显示名
    format!("{prefix}：{subject}「{tool_name}」在「{label}」组内被设为需批准。") // 组内需批准理由
}
pub fn blocked_in(data: &Value, group_key: &str, tool_name: &str) -> Option<String> { // 纯函数：组关闭硬拦截
    if !["computer", "word", "ppt", "excel", "mobile"].contains(&group_key) { return None; } // 仅电脑/Office/移动端组
    let enabled = data.get("groups").and_then(|groups| groups.get(group_key)).and_then(|group| group.get("enabled")).and_then(Value::as_bool).unwrap_or(true); // 组启用
    if enabled { return None; } // 开启放行
    let label = match group_key { "computer" => "电脑控制", "word" => "Word操作", "ppt" => "PPT操作", "mobile" => "移动端", _ => "Excel操作" }; // 组中文名
    Some(format!("{label}已关闭：{tool_name} 被拒绝执行。请到「设置 → 内置工具 → {label}」开启，或在权限页启用该组。")) // 拒绝文案
}
pub fn tool_blocked(group_key: &str, tool_name: &str) -> Option<String> { // 组级硬拦截：computer/word 组关闭时直接拒绝，不弹审批
    blocked_in(&load(), group_key, tool_name) // 读配置后判定
}
pub fn append_custom(command: &str) -> Value { // 追加命令到自定义白名单
    let text = squash(command); // 归一化命令
    if text.is_empty() { return load(); } // 空命令直接返回当前配置
    let _guard = LOCK.lock(); // 持锁写盘
    let mut data = load(); // 加载配置
    let existed = data.get("custom").and_then(Value::as_array).map(|items| items.iter().any(|item| item.as_str().map(|value| value.to_lowercase() == text.to_lowercase()).unwrap_or(false))).unwrap_or(false); // 忽略大小写去重
    if !existed { // 不存在才追加
        if let Some(items) = data.get_mut("custom").and_then(Value::as_array_mut) { items.push(Value::String(text)); } // 写入白名单
        let _ = write(&data); // 落盘
    }
    load() // 返回最新配置
}
pub fn snapshot() -> Value { // 生成权限配置快照，附带组元数据供前端渲染
    let data = load(); // 加载配置
    let mut meta: Vec<Value> = Vec::new(); // 组元数据
    if let Some(groups) = data.get("groups").and_then(Value::as_object) { // 遍历权限组
        let mut keys: Vec<String> = GROUP_ORDER.iter().filter(|key| groups.contains_key(**key)).map(|key| key.to_string()).collect(); // 按预设顺序排列
        keys.extend(groups.keys().filter(|key| !GROUP_ORDER.contains(&key.as_str())).cloned()); // 追加自定义组
        for key in keys { // 逐组生成元数据
            if let Some(group) = groups.get(&key) { // 取组
                meta.push(json!({"key": key, "label": group.get("label").and_then(Value::as_str).unwrap_or(&key), "commands": group.get("commands").and_then(Value::as_object).map(|commands| commands.keys().cloned().collect::<Vec<String>>()).unwrap_or_default()})); // 记录键/标签/条目
            }
        }
    }
    let mut result = data; // 结果以配置为底
    result["groups_meta"] = Value::Array(meta); // 注入组元数据
    result // 返回快照
}

#[cfg(test)] // 单元测试
mod tests { // 权限组硬拦截
    use super::*; // 引入
    fn data(word: bool, computer: bool) -> Value { json!({"groups": {"word": {"enabled": word}, "computer": {"enabled": computer}}}) } // 构造
    #[test] // Word 关闭拦截
    fn blocks_word_when_disabled() { assert!(blocked_in(&data(false, true), "word", "word_write").is_some()); } // 拦截
    #[test] // Word 开启放行
    fn allows_word_when_enabled() { assert!(blocked_in(&data(true, true), "word", "word_write").is_none()); } // 放行
    #[test] // 电脑控制关闭拦截
    fn blocks_computer_when_disabled() { assert!(blocked_in(&data(true, false), "computer", "computer_click").is_some()); } // 拦截
    #[test] // 临时关闭组并恢复
    fn tool_blocked_reads_saved_config() {
        let path = &*crate::config::WRITABLE_PERMISSION_FILE; // 可写配置
        let original = std::fs::read_to_string(path).ok(); // 备份
        let mut data = load(); // 当前配置
        data["groups"]["word"]["enabled"] = json!(false); // 关闭 word 组
        save(&data).unwrap(); // 落盘
        let blocked = tool_blocked("word", "word_write"); // 读取拦截
        if let Some(text) = original { std::fs::write(path, text).unwrap(); } else { let _ = std::fs::remove_file(path); } // 恢复
        assert!(blocked.is_some()); // 已拦截
        assert!(tool_blocked("word", "word_write").is_none()); // 恢复后放行
    }
    #[test] // 12 工具清单
    fn word_tools_exactly_twelve() { assert_eq!(WORD_TOOLS.len(), 12); }
    #[test] // 其他组不受影响
    fn ignores_other_groups() { assert!(blocked_in(&data(false, false), "browser", "browser_open").is_none()); } // 放行
}
