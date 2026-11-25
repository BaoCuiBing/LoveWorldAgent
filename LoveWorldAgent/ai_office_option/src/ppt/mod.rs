pub mod presentation;
pub mod slide;
pub mod read;
pub mod write;
pub mod shape;
pub mod table;
pub mod format;
pub mod chart;
pub mod media;
pub mod locator;

use serde_json::{Map, Value}; // JSON 类型
use std::time::Duration; // 超时
use crate::common::sta::{StaCell, StaHandle}; // 独立 STA
use crate::word::report::{err_json, ok_json, ToolError}; // 信封

static STA: StaCell = StaCell::new(); // PPT 独立 STA（与 Word/Excel 互不阻塞）
const TOOLS: [&str; 10] = [ // 10 个 ppt_* 工具
    "ppt_presentation", "ppt_read", "ppt_slide", "ppt_write", "ppt_shape",
    "ppt_table", "ppt_format", "ppt_chart", "ppt_media", "ppt_where_am_i",
];

pub fn is_tool(name: &str) -> bool { TOOLS.contains(&name) } // 白名单
pub fn timeout_of(cmd: &str) -> Duration { // 按族超时：读 15s / 写格式 60s / 生命周期 180s
    match cmd {
        "ppt_presentation" => Duration::from_secs(180),
        "ppt_read" | "ppt_where_am_i" => Duration::from_secs(15),
        _ => Duration::from_secs(60),
    }
}
pub fn run(cmd: &str, args: &Map<String, Value>) -> Value { // 提交到 PPT STA
    STA.handle("ppt-sta", execute).submit(cmd, args, timeout_of(cmd)) // 串行执行
}
fn execute(cmd: &str, args: &Map<String, Value>) -> Value { // STA 内实际执行
    let result = match cmd { // 分派
        "ppt_presentation" => presentation::run(args),
        "ppt_read" => read::run(args),
        "ppt_slide" => slide::run(args),
        "ppt_write" => write::run(args),
        "ppt_shape" => shape::run(args),
        "ppt_table" => table::run(args),
        "ppt_format" => format::run(args),
        "ppt_chart" => chart::run(args),
        "ppt_media" => media::run(args),
        "ppt_where_am_i" => presentation::where_am_i(args),
        other => err_json(&ToolError::new("NOT_IMPLEMENTED", format!("未知 PPT 工具 {other}"))),
    };
    result
}
pub fn handle() -> &'static StaHandle { STA.handle("ppt-sta", execute) } // 供 shutdown 使用