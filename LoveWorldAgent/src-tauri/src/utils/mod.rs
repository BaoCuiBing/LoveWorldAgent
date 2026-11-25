pub mod browser; // 工具模块入口：图标、日志、帮助文档打开、本机浏览器 Cookie 读取、原生菜单与标题栏主题
pub mod computer_use; // 桌面自动化 sidecar 管道服务端（21 个 computer_*）
pub mod icon; // 窗口图标资源保障
pub mod logger; // 文件日志
pub mod menu; // 原生菜单
pub mod mobile; // 移动端 sidecar 管道服务端（28 个 mobile_*）
pub mod open_file; // 帮助文档打开
pub mod sidecar_pipe; // sidecar 通用管道（PeekNamedPipe）与步骤日志
pub mod systeam_browser; // 本机浏览器 Cookie 读取
pub mod titlebar; // 标题栏主题
pub mod office_option; // Office 工具 sidecar 管道服务端（32 个 word_/ppt_/excel_）
