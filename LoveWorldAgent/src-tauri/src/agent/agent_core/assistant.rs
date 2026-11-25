use std::collections::BTreeMap; // 主智能体装配：全局提示词 + 模型补丁 + 工具纪律 + 规则/技能/摘要注入 + 子智能体派发工具
use std::path::PathBuf; // 规则目录路径类型
use std::sync::Arc; // 工具处理器跨线程共享所有权
use serde_json::{json, Value}; // JSON 构造与动态值读取
use goose_agent::events::AgentEvent; // 子智能体事件转发的 SSE 事件类型
use rusqlite::Connection; // 主会话数据库连接
use tokio::sync::mpsc::UnboundedSender; // 主会话 SSE 通道
use crate::agent::{agents_repo, chat_prefs, models, rules, skills}; // 智能体仓库/会话偏好/模型/规则/技能
use crate::agent::agent_core::context; // 历史摘要与关键事实注入
use crate::agent::settings::AgentSettings; // 智能体设置入口
use crate::agent::tools::{params, ToolDef, ToolHandler}; // 工具定义、参数与异步处理器
use crate::agent::tools_details::ask_tools::build_ask_tool; // 提问工具构造器
use crate::agent::tools_details::browser_tools::build_browser_tools; // 浏览器工具构造器
use crate::agent::tools_details::computer_use_tools::build_computer_use_tools; // 桌面自动化工具构造器
use crate::agent::tools_details::mobile_tools::build_mobile_tools; // 移动端工具构造器
use crate::agent::tools_details::office_tools::build_office_tools; // Office 操作工具构造器（word/ppt/excel）
use crate::agent::tools_details::command_tools::build_command_tools; // 命令工具构造器
use crate::agent::tools_details::common::remember_tool_meta; // 工具元数据旁路写入
use crate::agent::tools_details::file_tools::build_file_tools; // 文件工具构造器
use crate::agent::tools_details::rule_tools::build_rule_tools; // 规则工具构造器
use crate::agent::tools_details::skill_tools::build_skill_tools; // 技能工具构造器
use crate::agent::tools_details::terminal_tools::build_terminal_tools; // 终端工具构造器
use crate::agent::tools_details::todo_tools::build_todo_tool; // 待办工具构造器
use crate::config; // 项目路径与配置读取
use crate::agent::permission; // 电脑控制组开关
fn computer_enabled() -> bool { // 读取权限组 computer.enabled 作为电脑控制开关
    permission::load().get("groups").and_then(|groups| groups.get("computer")).and_then(|group| group.get("enabled")).and_then(Value::as_bool).unwrap_or(true) // 缺省启用
}
fn word_enabled() -> bool { // 读取权限组 word.enabled 作为 Word 操作开关
    permission::load().get("groups").and_then(|groups| groups.get("word")).and_then(|group| group.get("enabled")).and_then(Value::as_bool).unwrap_or(true) // 缺省启用
}
fn ppt_enabled() -> bool { // 读取权限组 ppt.enabled
    permission::load().get("groups").and_then(|groups| groups.get("ppt")).and_then(|group| group.get("enabled")).and_then(Value::as_bool).unwrap_or(true) // 缺省启用
}
fn excel_enabled() -> bool { // 读取权限组 excel.enabled
    permission::load().get("groups").and_then(|groups| groups.get("excel")).and_then(|group| group.get("enabled")).and_then(Value::as_bool).unwrap_or(true) // 缺省启用
}
fn mobile_enabled() -> bool { // 读取权限组 mobile.enabled 作为移动端开关
    permission::load().get("groups").and_then(|groups| groups.get("mobile")).and_then(|group| group.get("enabled")).and_then(Value::as_bool).unwrap_or(true) // 缺省启用
}
pub const TOOL_DISCIPLINE_HEAD: [&str; 4] = [
    "每次写操作后回报文件名与变更摘要，不要重复粘贴全文；收到工具结果先判断是否达成目标，再决定下一步。",
    "通用文件工具（file_list/file_read/file_write/file_delete）只操作项目目录内的文本文件：写前先读，路径用项目相对路径；src-tauri/static/config.json 与 .venv、node_modules、dist、webdist 一律禁止访问；file_delete 仅在用户明确要求删除时调用。",
    "规则只读：系统提示已注入生效规则；需要补充上下文时用 rule_list 了解现状、rule_read 读取正文，不要尝试写入或删除规则。",
    "规则用简洁中文 Markdown：一级标题说明主题，正文用编号条目；一条规则只表达一个约束，避免空话。",
];
pub const DISCIPLINE_SKILL: &str = "技能只读：系统提示已列出启用技能的名称与用途，需要细节时调用 skill_read 读取正文；不要尝试修改技能。";
pub const TOOL_DISCIPLINE_TAIL: [&str; 3] = [
    "执行命令前用一句话说明目的；command_run 返回码非零时如实转述错误，不掩饰也不重复同一条失败命令。",
    "命令被用户拒绝后，不要重试同一条命令，改换思路或向用户提一个精确的问题。",
    "需要让用户看到命令执行过程时用终端工具族：terminal_open 建会话（拿 session_id，同目录会复用）→ terminal_send 发命令（submit 默认 true，返回 wait_reason 与本次新增输出）。terminal_send 已经带回本次输出，不要紧接着调 terminal_read。启动 npm run dev / uvicorn --reload 等长驻开发服务时必须传 background=true，之后用 terminal_read(since) 读增量日志、terminal_list 查存活；普通一次性命令不要关闭终端会话。只要结果时可用 command_run；两者同样受权限白名单约束，被拒后不要重试同一条命令。",
];
pub const DISCIPLINE_BROWSER: &str = "内置浏览器工具驱动独立的 ai_browser 窗口：导航与等待用 browser_open/navigate/navigate_back/wait_for；交互前先 browser_snapshot 取元素 ref，再 click/hover/type/select_option/press_key/scroll；页面结构变化后旧 ref 失效，必须重新 snapshot。取证与排障用 browser_read_text/take_screenshot/console_messages/network_requests；登录或验证码用 browser_waiting_for_user_interaction 交还用户，弹窗用 browser_handle_dialog，页面逻辑用 browser_evaluate，标签页用 browser_tabs，状态用 browser_state。"; // 浏览器纪律，仅浏览器启用时注入
pub const TOOL_LIST_DISCIPLINE: &str = "# 工具清单纪律\n1. 工具（function/MCP，可直接调用）、技能（skill_*，按需读取的说明文档）、子智能体（agent_call 派发）三者不同；用户问\"有哪些工具/能力\"时指可直接调用的工具，不要把技能或子智能体当成工具清单。\n2. 回答工具清单时严格依据下方「当前实际注册的工具清单」与你的工具 schema 逐项枚举，可归类但不得遗漏，也不得编造清单之外的工具。\n3. 清单随对话开关与 MCP 连接动态变化，以本次注入的清单为准。"; // 工具清单纪律，防止模型把技能/子智能体误报为工具
pub const DISCIPLINE_ASK: &str = "仅在任务目标、范围或交付形式不明确时才用 ask_user 向用户提问（一次可提多个问题，options 可为空）；能自行判断或已问过的不要重复提问。"; // 提问纪律，按需注入
pub const DISCIPLINE_TODO: &str = "多步任务（≥ 3 步）必须用 todo_write 维护本次任务的待办清单：动手前先提交一份全量清单（首条置 in_progress、其余 pending），每完成一步就立即重新提交全量；每次提交都是全量覆盖，status 只能取 pending/in_progress/completed。"; // 待办纪律，按需注入
pub const DISCIPLINE_COMPUTER: &str = "电脑控制（computer_*，23 个对齐工具）是兜底手段：能用 command_run、file_*、browser_* 完成的不要用桌面操作。先用 computer_list_apps 或 computer_list_windows（或 computer_get_accessibility_tree）确认 pid 与 window_id，必要时用 computer_launch_app 启动；再用 computer_get_window_state(pid, window_id) 一次性获取 UIA 树（含 snapshot_id 与 element_token）及内存画面经 omni_parser 解析出的 omni_elements（OCR 文本与图标框中心坐标，永不返回截图图片），无窗口场景用 computer_get_desktop_state。操作优先传 element_token（click/double_click/right_click/scroll/type_text/set_value），仅无 UIA 节点时用 omni_elements/窗口局部坐标 (x,y) 或 scope='desktop'；单左键默认后台 delivery_mode='background'，失败再改 'foreground'；组合键用 computer_hotkey，菜单路径用 computer_invoke_menu，几何调整用 computer_set_window_frame，置前用 computer_bring_to_front，强杀用 computer_kill_app；每次操作后重新调用 computer_get_window_state 复核结果。"; // 电脑控制纪律，对齐 23 工具与 omni_parser 契约
pub const SUBAGENT_DISCIPLINE_CORE: [&str; 6] = [
    "每次写操作后回报文件名与变更摘要，不要重复粘贴全文。",
    "通用文件工具只操作项目目录内的文本文件：写前先读，路径用项目相对路径。",
    "规则只读：需要时用 rule_list 了解现状、rule_read 读取正文，不要尝试写入或删除规则。",
    "规则用简洁中文 Markdown：一级标题说明主题，正文用编号条目。",
    "只使用实际授予你的工具；未出现在工具列表中的能力不要假设可用，禁止调用 agent_list、agent_call、mcp_* 等主智能体工具。",
    "执行命令或终端操作前用一句话说明目的；返回码非零时如实转述错误。",
];
pub const SUBAGENT_DISCIPLINE_SKILL: &str = "技能只读：需要细节时调用 skill_read 读取正文。";
pub const SUBAGENT_DISCIPLINE_BROWSER: &str = "浏览器控制（browser_*）：先 browser_snapshot 取元素 ref 再点击/输入，结构变化后重新 snapshot。";
pub const SUBAGENT_DISCIPLINE_COMPUTER: &str = "电脑控制（23 工具）是兜底手段：先 computer_list_apps/computer_list_windows 获取 pid 与 window_id，再用 computer_get_window_state 取 UIA element_token 与 omni_parser 解析结果（不返回截图）；无窗目标用 computer_get_desktop_state。点击/输入/滚动优先 element_token，其次窗口局部 (x,y) 或 scope='desktop'；组合键用 computer_hotkey，菜单用 computer_invoke_menu；动作后重调 computer_get_window_state 复核。"; // 子智能体电脑纪律
pub const DISCIPLINE_WORD: &str = "Word 操作（word_*）纪律：所有工具必须传本机盘符开头的完整绝对路径 path（禁止传相对路径；status/quit 除外），不使用临时 id；复杂文档先用 word_document status 与 word_where_am_i 确认状态与光标；写入长文分 3~8 次 word_write 追加，单次 ≤100k 字符；写入前确认 path 指向目标文档，返回 DOC_CHANGED 时先重新打开；保存用 word_document save（自动备份），读文用 word_read offset/limit 分页；涉及写/格式/关闭的操作会按权限逐项审批，组关闭则硬拦截。"; // Word 操作纪律，仅 Word 启用时注入
pub const DISCIPLINE_PPT: &str = "PPT 操作（ppt_*）纪律：所有工具必须传本机盘符开头的完整绝对路径 path（禁止传相对路径；status/quit 除外）；操作前先用 ppt_presentation status 与 ppt_where_am_i 确认演示文稿与当前幻灯片；修改后及时保存，组关闭则硬拦截。"; // PPT 操作纪律，仅 PPT 启用时注入
pub const DISCIPLINE_EXCEL: &str = "Excel 操作（excel_*）纪律：所有工具必须传本机盘符开头的完整绝对路径 path（禁止传相对路径；status/quit 除外）；操作前先用 excel_workbook status 与 excel_where_am_i 确认工作簿、工作表与选区；公式与格式修改后及时保存，组关闭则硬拦截。"; // Excel 操作纪律，仅 Excel 启用时注入
pub const SUBAGENT_DISCIPLINE_WORD: &str = "Word 操作：所有 word_* 工具必须传本机盘符开头的完整绝对路径 path（禁止相对路径）；先 word_document status 与 word_where_am_i 确认状态，长文分多次 word_write 追加，写入前确认目标 path，保存用 word_document save。"; // 子智能体 Word 纪律
pub const SUBAGENT_DISCIPLINE_PPT: &str = "PPT 操作：所有 ppt_* 工具必须传本机盘符开头的完整绝对路径 path（禁止相对路径）；先 ppt_presentation status 与 ppt_where_am_i 确认状态，修改后保存。";
pub const SUBAGENT_DISCIPLINE_EXCEL: &str = "Excel 操作：所有 excel_* 工具必须传本机盘符开头的完整绝对路径 path（禁止相对路径）；先 excel_workbook status 与 excel_where_am_i 确认状态，修改后保存。";
pub const DISCIPLINE_MOBILE: &str = "移动端（mobile_*）纪律：先用 mobile_list_available_devices 确认设备，多设备时必须显式指定 device；交互前用 mobile_list_elements_on_screen 取元素 @eN 引用，点击/长按/双击优先传 ref，其次传 x/y（截图坐标会自动换算到设备分辨率）；输入用 mobile_type_keys，非 ASCII 依赖设备剪贴板能力，失败会返回 UNSUPPORTED_INPUT；导航用 mobile_open_url 或按 home/back/enter 等按键，不要假设上一次的 ref 仍然有效；截图与录屏落盘到 static/data/mobile_screen。"; // 移动端纪律，仅移动端启用时注入
pub const SUBAGENT_DISCIPLINE_MOBILE: &str = "移动端：先 mobile_list_available_devices 确认设备；交互前 mobile_list_elements_on_screen 取 ref，点击优先传 ref；输入用 mobile_type_keys；旧 ref 失效后重新取元素树。"; // 子智能体移动端纪律
pub fn resolve_provider(model_name: &str) -> String { // 按模型名解析供应商，空名取首个启用模型
    let llm = config::load_agent_config().get("llm").cloned().unwrap_or(Value::Null); // 读取 llm 配置节，缺失按空处理
    let items = models::normalize_llm(&llm); // 归一化 llm 配置为条目数组
    for item in &items { // 遍历所有模型配置
        let config = models::parse_model(item); // 解析单条模型配置
        if config.enabled && !config.model.is_empty() && (model_name.is_empty() || config.model == model_name) { return config.provider; } // 命中启用且模型名匹配的配置则返回供应商
    }
    models::active_model(&llm).map(|config| config.provider).unwrap_or_default() // 无显式匹配时回退到当前激活模型
}
pub fn build_instructions(provider: &str, ask: bool, todo: bool, browser: bool, computer: bool, word: bool, ppt: bool, excel: bool, mobile: bool, has_skills: bool) -> String { // 组装全局纪律提示词，严格按全部开关动态编号拼接
    let mut entries: Vec<&str> = TOOL_DISCIPLINE_HEAD.to_vec();
    if has_skills { entries.push(DISCIPLINE_SKILL); }
    entries.extend(TOOL_DISCIPLINE_TAIL);
    if browser { entries.push(DISCIPLINE_BROWSER); }
    if computer { entries.push(DISCIPLINE_COMPUTER); }
    if word { entries.push(DISCIPLINE_WORD); }
    if ppt { entries.push(DISCIPLINE_PPT); }
    if excel { entries.push(DISCIPLINE_EXCEL); }
    if mobile { entries.push(DISCIPLINE_MOBILE); }
    if ask { entries.push(DISCIPLINE_ASK); }
    if todo { entries.push(DISCIPLINE_TODO); }
    let lines = entries.iter().enumerate().map(|(idx, text)| format!("{}. {text}", idx + 1)).collect::<Vec<String>>().join("\n");
    let body = format!("# 工具与写操作纪律\n{lines}");
    let provider = if provider.is_empty() { resolve_provider("") } else { provider.to_string() }; // 未提供供应商时按配置解析
    format!("{}\n\n{body}", agents_repo::global_prompt(&provider)) // 全局角色提示词 + 纪律正文
}
fn shared_instructions(settings: &AgentSettings, session_id: &str) -> String { // 组装项目/规则/技能/历史上下文的共享提示词
    let rule_text = rules::load_rules(&settings.rule_dir); // 从规则目录读取规则正文
    let rule_text = if rule_text.is_empty() { "（暂无规则）".to_string() } else { rule_text }; // 无规则时给占位说明
    let rule_index = rules::model_decision_index(&settings.rule_dir); // 读取规则索引供模型按需选择
    let index_text = if rule_index.is_empty() { String::new() } else { format!("\n## 按需读取的规则（按 description 判断是否 rule_read）\n{rule_index}\n") }; // 索引非空才生成按需读取段落
    let project_text = config::project_path().map(|path| path.to_string_lossy().to_string()).unwrap_or_else(|| "（尚未选择项目文件夹：file_* 与 command_run 不可用，请提示用户先在顶栏选择项目文件夹）".to_string()); // 未选项目目录时提示用户
    let context_text = if session_id.is_empty() { String::new() } else { context::inject_text(session_id) }; // 有会话才注入历史摘要与关键事实
    let skill_text = if skills::has_enabled(&settings.skill_dir) { format!("\n## 可用技能（仅启用项，正文按需 skill_read）\n{}\n", skills::skill_index_text(&settings.skill_dir)) } else { String::new() }; // 技能全部关闭时整段（标题与占位）不出现
    format!("\n## 当前项目目录（file_* 与 command_run 的根目录）\n{project_text}\n\n## 规则（必须遵守）\n{rule_text}\n{index_text}{skill_text}{context_text}") // 拼装最终共享上下文
}
fn family_of(name: &str) -> &'static str { // 按工具名前缀归类，用于工具清单分组展示
    for (prefix, label) in [("browser_", "浏览器"), ("computer_", "电脑"), ("mobile_", "移动端"), ("word_", "Word"), ("ppt_", "PPT"), ("excel_", "Excel"), ("file_", "文件"), ("rule_", "规则"), ("skill_", "技能"), ("terminal_", "终端"), ("command_", "命令"), ("agent_", "子智能体")] { // 前缀与中文类别映射表
        if name.starts_with(prefix) { return label; } // 命中前缀立即返回类别
    }
    if name == "ask_user" || name == "todo_write" { "交互" } else { "其他" } // 交互类单列，其余归为其他
}
pub fn tool_catalog_text(tools: &[ToolDef], mcp_catalog: &BTreeMap<String, Vec<String>>) -> String { // 生成实际注册工具清单，供模型回答"有哪些工具"
    let mut grouped: BTreeMap<&'static str, Vec<String>> = BTreeMap::new(); // 按类别聚合工具名
    for tool in tools { grouped.entry(family_of(&tool.name)).or_default().push(tool.name.clone()); } // 逐工具归入对应类别
    let mut lines = vec!["\n## 当前实际注册的工具清单（回答\"有哪些工具\"时据此逐项枚举）".to_string(), format!("内置工具（{}）：", tools.len())]; // 清单标题与内置工具计数
    for (label, items) in &grouped { lines.push(format!("- {label}：{}", items.join(", "))); } // 逐类别输出工具名
    if !mcp_catalog.is_empty() { // 仅当 MCP 已连接且有工具时列出
        let total: usize = mcp_catalog.values().map(Vec::len).sum(); // 统计 MCP 工具总数
        lines.push(format!("MCP 工具（{total}，调用名为 mcp_<服务>__<工具>）：")); // 说明 MCP 工具调用命名规则
        for (server, items) in mcp_catalog { if !items.is_empty() { lines.push(format!("- {server}（{}）：{}", items.len(), items.join(", "))); } } // 跳过无工具的服务，只列非空项
    }
    format!("{}\n", lines.join("\n")) // 拼接成最终清单文本
}
pub fn base_tools(settings: &AgentSettings, browser: bool, computer: bool, word: bool, ppt: bool, excel: bool, mobile: bool) -> Vec<ToolDef> { // 装配主智能体基础工具集，按浏览器/电脑/Office/移动端开关注册
    let root = config::project_path(); // 项目根目录作为文件/命令工具的白名单根
    let mut tools = build_rule_tools(settings.rule_dir.clone()); // 规则读写工具
    tools.extend(build_skill_tools(settings.skill_dir.clone())); // 技能只读工具
    tools.extend(build_file_tools(root.clone())); // 文件读写工具
    tools.extend(build_command_tools(root.clone(), settings.command_timeout_seconds)); // 命令工具受根目录与超时限制
    tools.extend(build_terminal_tools(root, settings.command_timeout_seconds)); // 终端工具复用同一根目录与超时
    if browser { tools.extend(build_browser_tools()); } // 浏览器开关启用才注册 browser_* 工具
    if computer { tools.extend(build_computer_use_tools()); } // 电脑开关启用才注册 computer_* 工具
    if mobile { tools.extend(build_mobile_tools()); } // 移动端开关启用才注册 mobile_* 工具
    if word || ppt || excel { // 任一 Office 族启用
        let mut office = build_office_tools(); // 32 个 Office 工具
        office.retain(|tool| match tool.group.as_str() { "ppt" => ppt, "excel" => excel, _ => word }); // 按族裁剪
        tools.extend(office); // 注入
    }
    tools // 返回基础工具集
}
pub fn agent_instructions(agent_def: &Value, has_skills: bool) -> String { // 生成子智能体角色提示词：严格按「全局开关 && 自身 tools 白名单」拼接
    let name = agent_def.get("name").and_then(Value::as_str).unwrap_or_default(); // 读取子智能体名称
    let body = agent_def.get("body").and_then(Value::as_str).unwrap_or_default().trim(); // 读取角色说明正文
    let wanted: Vec<String> = agent_def.get("tools").and_then(Value::as_array).map(|items| items.iter().map(|item| crate::state::json_str(Some(item)).trim().to_lowercase()).collect()).unwrap_or_default(); // 工具白名单
    let allows = |key: &str| wanted.is_empty() || wanted.iter().any(|item| item == key); // 空数组继承全部
    let mut entries: Vec<&str> = SUBAGENT_DISCIPLINE_CORE.to_vec();
    if has_skills && allows("skill") { entries.push(SUBAGENT_DISCIPLINE_SKILL); }
    if allows("browser") && crate::utils::browser::enabled() { entries.push(SUBAGENT_DISCIPLINE_BROWSER); }
    if allows("computer") && computer_enabled() { entries.push(SUBAGENT_DISCIPLINE_COMPUTER); }
    if allows("word") && word_enabled() { entries.push(SUBAGENT_DISCIPLINE_WORD); }
    if allows("ppt") && ppt_enabled() { entries.push(SUBAGENT_DISCIPLINE_PPT); }
    if allows("excel") && excel_enabled() { entries.push(SUBAGENT_DISCIPLINE_EXCEL); }
    if allows("mobile") && mobile_enabled() { entries.push(SUBAGENT_DISCIPLINE_MOBILE); }
    let lines = entries.iter().enumerate().map(|(idx, text)| format!("{}. {text}", idx + 1)).collect::<Vec<String>>().join("\n");
    let discipline = format!("# 工具与写操作纪律\n{lines}");
    format!("你是运行在 LoveWorldAgent 桌面端的子智能体（自定义智能体）：能独立完成自己范围内的任务，但主要由主智能体 Agent 调用；调用方只给一段任务描述，请直接产出结果，不要把问题抛回给调用方。\n\n你是「{name}」。{body}\n\n{discipline}\n\n纪律：只操作任务范围内的文件；不编造不存在的规则或技能；不确定时先读取再行动。") // 角色说明 + 纪律正文
}
pub fn agent_tools(settings: &AgentSettings, agent_def: &Value) -> Vec<ToolDef> { // 按子智能体声明的白名单裁剪工具（电脑开关由 router 停用子智能体保证不派发）
    let wanted: Vec<String> = agent_def.get("tools").and_then(Value::as_array).map(|items| items.iter().map(|item| crate::state::json_str(Some(item)).trim().to_lowercase()).collect()).unwrap_or_default(); // 解析 tools 白名单并统一小写
    if wanted.is_empty() { return base_tools(settings, crate::utils::browser::enabled(), computer_enabled(), word_enabled(), ppt_enabled(), excel_enabled(), mobile_enabled()); } // 未声明工具时继承全部基础工具（按开关）
    let root = config::project_path(); // 项目根目录，供文件/命令工具使用
    let mut tools: Vec<ToolDef> = Vec::new(); // 裁剪后的工具集
    if wanted.iter().any(|item| item == "rule") { tools.extend(build_rule_tools(settings.rule_dir.clone())); } // 白名单含 rule 时注入规则工具
    if wanted.iter().any(|item| item == "skill") { tools.extend(build_skill_tools(settings.skill_dir.clone())); } // 白名单含 skill 时注入技能工具
    if wanted.iter().any(|item| item == "file") { tools.extend(build_file_tools(root.clone())); } // 白名单含 file 时注入文件工具
    if wanted.iter().any(|item| item == "command") { // command 权限同时放开一次性命令与终端会话
        tools.extend(build_command_tools(root.clone(), settings.command_timeout_seconds)); // 一次性命令工具
        tools.extend(build_terminal_tools(root.clone(), settings.command_timeout_seconds)); // 终端 PTY 会话工具
    }
    if wanted.iter().any(|item| item == "browser") && crate::utils::browser::enabled() { tools.extend(build_browser_tools()); } // 白名单含 browser 且浏览器启用时注入
    if wanted.iter().any(|item| item == "computer") && computer_enabled() { tools.extend(build_computer_use_tools()); } // 白名单含 computer 且电脑启用时注入
    if wanted.iter().any(|item| item == "word") && word_enabled() { tools.extend(build_office_tools().into_iter().filter(|tool| tool.group == "word")); } // 白名单含 word 且 Word 启用
    if wanted.iter().any(|item| item == "ppt") && ppt_enabled() { tools.extend(build_office_tools().into_iter().filter(|tool| tool.group == "ppt")); } // 白名单含 ppt 且 PPT 启用
    if wanted.iter().any(|item| item == "excel") && excel_enabled() { tools.extend(build_office_tools().into_iter().filter(|tool| tool.group == "excel")); } // 白名单含 excel 且 Excel 启用
    if wanted.iter().any(|item| item == "mobile") && mobile_enabled() { tools.extend(build_mobile_tools()); } // 白名单含 mobile 且移动端启用
    tools // 返回裁剪后的工具集
}
#[derive(Clone)] // 主会话桥接上下文可克隆
pub struct ParentContext { // 主会话桥接：子智能体事件写回主会话并实时推送
    pub db: Arc<parking_lot::Mutex<Connection>>, // 主会话数据库连接
    pub session_id: String, // 主会话 id
    pub tx: UnboundedSender<AgentEvent>, // 主会话 SSE 事件通道
}
#[derive(Clone)] // 子智能体运行描述可克隆
pub struct SubagentRun { // 一次子智能体运行：桥接上下文 + 派发标识
    pub parent: ParentContext, // 主会话桥接上下文
    pub call_id: String, // 本次派发的唯一调用 id
    pub agent_id: String, // 子智能体 id
    pub agent_name: String, // 子智能体显示名
}
pub fn build_subagent_tools(settings: &AgentSettings, provider: &str, parent: &ParentContext) -> Vec<ToolDef> { // 构造 agent_list 与 agent_call 派发工具
    let list = ToolDef::new("agent_list", "列出可调用的自定义子智能体（id/name/when）；停用或不可派发的不在列表中", params(&[]), { // 列出可派发子智能体的工具定义
        crate::agent::tools::blocking(|_args: String| { // 同步处理器包装
            let items: Vec<Value> = agents_repo::list_agents().into_iter().filter(|item| item.get("enabled").and_then(Value::as_bool).unwrap_or(false) && item.get("callable").and_then(Value::as_bool).unwrap_or(false)).map(|item| json!({"id": item.get("id").cloned().unwrap_or(Value::Null), "name": item.get("name").cloned().unwrap_or(Value::Null), "when": item.get("when").cloned().unwrap_or(Value::Null)})).collect(); // 只列启用且可派发的子智能体
            if items.is_empty() { return "（当前没有可调用的子智能体）".to_string(); } // 无可调用项时给占位提示
            serde_json::to_string(&items).unwrap_or_else(|_| "（当前没有可调用的子智能体）".to_string()) // 序列化失败时降级为空提示
        })
    });
    let settings = settings.clone(); // 捕获设置到异步闭包
    let provider = provider.to_string(); // 捕获供应商到异步闭包
    let parent = parent.clone(); // 捕获主会话桥接上下文
    let call: ToolHandler = Arc::new(move |args: String| { // 构造异步派发处理器
        let settings = settings.clone(); // 每次调用克隆设置
        let provider = provider.clone(); // 每次调用克隆供应商
        let parent = parent.clone(); // 每次调用克隆桥接上下文
        Box::pin(async move { // 固定异步句柄供工具层 await
            let value: Value = serde_json::from_str(&args).unwrap_or_else(|_| json!({})); // 解析派发参数，非法 JSON 按空对象处理
            let agent_id = value.get("agent_id").and_then(Value::as_str).unwrap_or_default().to_string(); // 目标子智能体 id
            let task = value.get("task").and_then(Value::as_str).unwrap_or_default().trim().to_string(); // 派发任务描述
            let Some(agent_def) = agents_repo::load_agent(&agent_id) else { return format!("调用失败：子智能体不存在、已停用或不可派发: {agent_id}"); }; // 不存在/停用时立即返回错误
            if !agent_def.get("callable").and_then(Value::as_bool).unwrap_or(false) { return format!("调用失败：子智能体不存在、已停用或不可派发: {agent_id}"); } // 二次校验可派发标志，防越权
            if task.is_empty() { return "调用失败：任务描述不能为空".to_string(); } // 任务描述为空时拒绝派发
            let name = agent_def.get("name").and_then(Value::as_str).unwrap_or_default().to_string(); // 子智能体显示名
            let tools = agent_tools(&settings, &agent_def); // 按白名单裁剪工具
            let instructions = agent_instructions(&agent_def, skills::has_enabled(&settings.skill_dir)); // 生成子智能体系统提示词
            let call_id = format!("agent_{}", uuid::Uuid::new_v4().simple()); // 本次派发的唯一调用 id
            let run = SubagentRun { parent: parent.clone(), call_id: call_id.clone(), agent_id: agent_id.clone(), agent_name: name.clone() }; // 组装子智能体运行描述
            let begin = std::time::Instant::now(); // 记录派发起始时间
            let result = crate::agent::agent_core::state_machine::run_task(&settings, &provider, &instructions, tools, &task, settings.max_turns, Some(run)).await; // await 运行子智能体状态机，受 max_turns 限制
            let (ok, summary) = match &result { Ok(text) => (true, crate::state::truncate_chars(text, 400)), Err(err) => (false, crate::state::truncate_chars(err, 400)) }; // 成功与失败均截断摘要
            remember_tool_meta("agent_call", json!({"callId": call_id, "agentId": agent_id, "agentName": name, "task": task, "ok": ok, "summary": summary, "error": if ok { String::new() } else { summary.clone() }, "durationMs": begin.elapsed().as_millis() as i64})); // 写回派发元数据供前端展示
            match result { // 按执行结果返回正文或错误
                Ok(text) => if text.trim().is_empty() { "（子智能体没有返回内容）".to_string() } else { text.trim().to_string() }, // 空正文给占位提示
                Err(err) => format!("调用失败：{err}"), // 失败返回错误文案
            }
        })
    });
    let call = ToolDef::new("agent_call", "把一段任务派发给指定子智能体并返回其最终答复；agent_id 取自 agent_list", params(&[("agent_id", "string", "子智能体 id"), ("task", "string", "任务描述")]), call); // 派发工具定义
    vec![list, call] // 返回列表与派发两个工具
}
pub fn build_tools(settings: &AgentSettings, agent_def: Option<&Value>, switches: &Value, mcp_defs: Vec<ToolDef>, parent: &ParentContext) -> Vec<ToolDef> { // 统一工具装配入口，子智能体优先
    if let Some(agent_def) = agent_def { return agent_tools(settings, agent_def); } // 子智能体模式只用自身白名单
    let provider = resolve_provider(""); // 解析当前模型供应商
    let browser = crate::utils::browser::enabled(); // 浏览器总开关
    let computer = computer_enabled(); // 电脑控制总开关
    let word = word_enabled(); // Word 操作总开关
    let ppt = ppt_enabled(); // PPT 操作总开关
    let excel = excel_enabled(); // Excel 操作总开关
    let mobile = mobile_enabled(); // 移动端总开关
    let mut tools = base_tools(settings, browser, computer, word, ppt, excel, mobile); // 先装配基础工具（按开关）
    tools.extend(build_subagent_tools(settings, &provider, parent)); // 追加子智能体派发工具
    if switches.get("ask").and_then(Value::as_bool).unwrap_or(true) { tools.extend(build_ask_tool(true)); } // 会话开关决定是否注入提问工具
    if switches.get("todo").and_then(Value::as_bool).unwrap_or(true) { tools.extend(build_todo_tool(true)); } // 会话开关决定是否注入待办工具
    tools.extend(mcp_defs); // 追加 MCP 工具定义
    tools // 返回最终工具集
}
pub fn build_system_prompt(settings: &AgentSettings, agent_def: Option<&Value>, tools: &[ToolDef], mcp_catalog: &BTreeMap<String, Vec<String>>, provider: &str, session_id: &str) -> String { // 组装最终系统提示词
    let has_skills = skills::has_enabled(&settings.skill_dir); // 是否有启用的技能
    if let Some(agent_def) = agent_def { return format!("{}{}", agent_instructions(agent_def, has_skills), shared_instructions(settings, session_id)); } // 子智能体走按开关与白名单裁剪分支
    let switches = chat_prefs::flags(); // 读取会话级提问/待办开关
    let browser = crate::utils::browser::enabled(); // 浏览器总开关
    let computer = computer_enabled(); // 电脑控制总开关
    let word = word_enabled(); // Word 操作总开关
    let ppt = ppt_enabled(); // PPT 操作总开关
    let excel = excel_enabled(); // Excel 操作总开关
    let mobile = mobile_enabled(); // 移动端总开关
    let instructions = build_instructions(provider, switches.get("ask").and_then(Value::as_bool).unwrap_or(true), switches.get("todo").and_then(Value::as_bool).unwrap_or(true), browser, computer, word, ppt, excel, mobile, has_skills); // 生成纪律提示词（含全部开关）
    format!("{instructions}\n\n{TOOL_LIST_DISCIPLINE}{}{}", tool_catalog_text(tools, mcp_catalog), shared_instructions(settings, session_id)) // 纪律 + 工具清单 + 共享上下文
}
pub fn build_base_system_prompt(settings: &AgentSettings, agent_def: Option<&Value>, tools: &[ToolDef], mcp_catalog: &BTreeMap<String, Vec<String>>, provider: &str) -> String { // 构建不含会话动态摘要的基础系统提示词，由状态机每步动态追加最新摘要
    build_system_prompt(settings, agent_def, tools, mcp_catalog, provider, "") // 传空 session_id 延迟注入摘要与事实
}
pub fn rule_dir(settings: &AgentSettings) -> PathBuf { // 暴露规则目录路径给调用方
    settings.rule_dir.clone() // 返回规则目录副本
}
