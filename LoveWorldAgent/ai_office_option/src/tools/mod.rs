pub mod batch; // word_batch + word_variable
pub mod document; // word_document + word_where_am_i
pub mod edit; // word_edit + word_find
pub mod format; // word_format
pub mod media; // word_media
pub mod read; // word_read
pub mod structure; // word_structure
pub mod table; // word_table
pub mod write; // word_write
use serde_json::{Map, Value}; // JSON 类型
use crate::word::report::ok_json; // 成功信封
const TOOLS: [&str; 12] = [ // 12 个工具白名单
    "word_document", "word_read", "word_write", "word_table", "word_format", "word_structure", // 前 6
    "word_find", "word_edit", "word_media", "word_variable", "word_batch", "word_where_am_i", // 后 6
];
pub fn is_tool(name: &str) -> bool { // 工具名校验（三族并集）
    TOOLS.contains(&name) || crate::ppt::is_tool(name) || crate::excel::is_tool(name) // 判定
}
pub fn dispatch(cmd: &str, args: &Map<String, Value>) -> Option<Value> { // 工具分派（返回 None 表示未知）
    match cmd { // 分派
        "word_document" => Some(document::run(args)), // 文档
        "word_read" => Some(read::run(args)), // 读取
        "word_write" => Some(write::run(args)), // 写入
        "word_table" => Some(table::run(args)), // 表格
        "word_format" => Some(format::run(args)), // 格式
        "word_structure" => Some(structure::run(args)), // 结构
        "word_find" => Some(edit::find(args)), // 查找
        "word_edit" => Some(edit::edit(args)), // 编辑
        "word_media" => Some(media::run(args)), // 媒体
        "word_variable" => Some(batch::variable(args)), // 变量
        "word_batch" => Some(batch::batch(args)), // 批量
        "word_where_am_i" => Some(document::where_am_i(args)), // 位置
        _ => None, // 未知
    }
}
pub fn not_implemented(cmd: &str) -> Value { // 占位实现（阶段实现中逐个替换）
    ok_json(serde_json::json!({"tool": cmd, "pending": true})) // 占位
}
