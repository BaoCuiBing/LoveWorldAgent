use serde::{Deserialize, Serialize}; // 序列化支持
#[derive(Debug, Clone, Serialize, Deserialize)] // 单个 UI 元素
pub struct Element { // 元素结构
    #[serde(rename = "type")] // JSON 字段名为 type
    pub kind: String, // text / icon
    pub bbox: [f32; 4], // 归一化 x1,y1,x2,y2
    pub interactivity: bool, // 是否可交互
    pub content: Option<String>, // 文本或图标描述
    #[serde(skip_serializing_if = "Option::is_none")] // 空值不输出
    pub source: Option<String>, // 来源标记
}