pub mod progid; // 三族 ProgID 与实例解析
pub mod registry; // 路径→文档复用登记
pub mod report; // 统一错误/成功信封
pub mod paths; // 本地路径解析与安全校验
pub mod backup; // 保存前备份
pub mod screen; // Office 窗口自动截图采集
pub mod sta; // 通用 STA 串行队列（PPT/Excel）
pub mod session; // 共享 Word STA 会话
pub use report::*; // 常用信封符号
pub use paths::Paths; // 路径策略