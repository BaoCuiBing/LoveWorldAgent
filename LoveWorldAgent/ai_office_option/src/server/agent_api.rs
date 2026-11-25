use std::time::Duration; // 超时
use crate::tools::is_tool; // 工具白名单
pub fn is_known(cmd: &str) -> bool { // 已知工具判定
    is_tool(cmd) || cmd == "shutdown" // 白名单 + 关闭命令
}
pub fn timeout_of(cmd: &str) -> Duration { // 按工具选超时：读 15s / 写格式 60s / 打开保存导出批量 180s
    if cmd.starts_with("ppt_") { return crate::ppt::timeout_of(cmd); } // PPT 族
    if cmd.starts_with("excel_") { return crate::excel::timeout_of(cmd); } // Excel 族
    match cmd { // Word 分派
        "word_document" => Duration::from_secs(180), // 打开/保存/导出
        "word_media" | "word_batch" | "word_variable" => Duration::from_secs(60), // 媒体/批量/变量
        "word_write" | "word_table" | "word_format" | "word_structure" => Duration::from_secs(60), // 写操作
        _ => Duration::from_secs(15), // 读操作
    }
}
