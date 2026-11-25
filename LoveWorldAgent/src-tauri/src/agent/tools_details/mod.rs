pub mod ask_tools; // 工具实现子模块：公共层 + 规则/技能/提问/待办/文件/命令/终端/浏览器
pub mod browser_tools; // 内置浏览器工具
pub mod command_tools; // 一次性命令工具
pub mod computer_use_tools; // 桌面自动化工具（12 个 computer_*）
pub mod common; // 公共层：常量、路径门禁、diff 与元数据
pub mod file_tools; // 文件读写工具
pub mod mobile_tools; // 移动端工具（28 个 mobile_*）
pub mod rule_tools; // 规则读写工具
pub mod skill_tools; // 技能只读工具
pub mod terminal_tools; // 终端 PTY 会话工具
pub mod todo_tools; // 待办清单工具
pub mod office_tools; // Office 操作工具（32 个 word_/ppt_/excel_）
