use serde_json::{json, Map, Value}; // JSON 构造
#[derive(Debug, Clone)] // 工具错误（统一信封）
pub struct ToolError { // 结构化错误
    pub code: String, // 错误码
    pub message: String, // 人类可读消息
    pub hint: String, // 恢复建议
    pub recoverable: bool, // 是否可重试
}
pub type ToolResult<T> = std::result::Result<T, ToolError>; // 工具结果别名
impl ToolError { // 构造器
    pub fn new(code: &str, message: impl Into<String>) -> Self { // 创建错误（默认不可恢复）
        Self { code: code.to_string(), message: message.into(), hint: String::new(), recoverable: false } // 组装
    }
    pub fn recoverable(mut self, hint: impl Into<String>) -> Self { // 标记可恢复
        self.recoverable = true; self.hint = hint.into(); self // 返回
    }
    pub fn to_json(&self) -> Value { // 转错误信封
        json!({"code": self.code, "message": self.message, "hint": self.hint, "recoverable": self.recoverable}) // 信封
    }
}
#[derive(Debug, Clone, Default)] // 警告项
pub struct Warning { // 单条警告
    pub code: String, // 警告码
    pub message: String, // 消息
    pub detail: String, // 细节
}
impl Warning { // 构造器
    pub fn new(code: &str, message: impl Into<String>) -> Self { // 创建警告
        Self { code: code.to_string(), message: message.into(), detail: String::new() } // 组装
    }
}
#[derive(Debug, Clone, Default)] // 工具执行报告
pub struct Report { // 结果聚合
    pub warnings: Vec<Warning>, // 警告列表
    pub partial: bool, // 是否部分成功
}
impl Report { // 报告操作
    pub fn warn(&mut self, warning: Warning) { // 追加警告
        self.warnings.push(warning); // 收集
    }
    pub fn ok_json(&self, data: Value) -> Value { // 成功信封
        let mut map = Map::new(); // 外层
        map.insert("ok".into(), Value::Bool(true)); // 成功标记
        map.insert("data".into(), data); // 数据
        if !self.warnings.is_empty() { // 有警告
            map.insert("warnings".into(), json!(self.warnings.iter().map(|w| json!({"code": w.code, "message": w.message, "detail": w.detail})).collect::<Vec<_>>())); // 警告数组
        }
        if self.partial { map.insert("partial".into(), Value::Bool(true)); } // 部分成功标记
        Value::Object(map) // 返回
    }
}
pub fn ok_json(data: Value) -> Value { // 无警告成功信封
    Report::default().ok_json(data) // 委托
}
pub fn err_json(err: &ToolError) -> Value { // 失败信封
    json!({"ok": false, "error": err.to_json()}) // 失败
}
