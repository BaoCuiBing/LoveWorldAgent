pub mod backup; // 备份与轮转

pub mod instances; // ROT 实例与文档查重
pub mod locator; // 语义定位
pub mod markdown; // Markdown 解析与渲染
pub mod paths; // 路径白名单与校验
pub mod report; // 错误码、警告与结果聚合
pub use crate::common::session; // 共享 STA 会话（原 word/session.rs 已迁移至 common）

