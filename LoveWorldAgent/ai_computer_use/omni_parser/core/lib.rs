
mod bbox; // 元素结构
mod caption; // 图标描述
mod detector; // 图标检测
mod geometry; // 几何框
mod ocr; // CRAFT+CRNN 文本识别
mod ort_util; // ORT 会话与张量工具
mod parser; // 解析编排
mod server; // stdio JSON 服务
pub use bbox::Element; // 导出元素结构
pub use parser::{OmniParser, OmniParserConfig}; // 导出解析器与配置
pub use server::run_stdio_server; // 导出 stdio 服务入口