pub mod workbook;
pub mod read;
pub mod write;
pub mod sheet;
pub mod format;
pub mod formula;
pub mod table;
pub mod chart;
pub mod sort_filter;
pub mod where_am_i;
pub mod locator;

use serde_json::{Map, Value}; // JSON 类型
use std::time::Duration; // 超时
use crate::common::sta::{StaCell, StaHandle}; // 独立 STA
use crate::word::report::{err_json, ToolError}; // 信封

static STA: StaCell = StaCell::new(); // Excel 独立 STA
const TOOLS: [&str; 10] = [ // 10 个 excel_* 工具
    "excel_workbook", "excel_read", "excel_write", "excel_sheet", "excel_format",
    "excel_formula", "excel_table", "excel_chart", "excel_sort_filter", "excel_where_am_i",
];

pub fn is_tool(name: &str) -> bool { TOOLS.contains(&name) } // 白名单
pub fn timeout_of(cmd: &str) -> Duration { // 按族超时
    match cmd {
        "excel_workbook" => Duration::from_secs(180),
        "excel_read" | "excel_where_am_i" => Duration::from_secs(15),
        _ => Duration::from_secs(60),
    }
}
pub fn run(cmd: &str, args: &Map<String, Value>) -> Value { // 提交到 Excel STA
    STA.handle("excel-sta", execute).submit(cmd, args, timeout_of(cmd)) // 串行执行
}
fn execute(cmd: &str, args: &Map<String, Value>) -> Value { // STA 内实际执行
    match cmd { // 分派
        "excel_workbook" => workbook::run(args),
        "excel_read" => read::run(args),
        "excel_write" => write::run(args),
        "excel_sheet" => sheet::run(args),
        "excel_format" => format::run(args),
        "excel_formula" => formula::run(args),
        "excel_table" => table::run(args),
        "excel_chart" => chart::run(args),
        "excel_sort_filter" => sort_filter::run(args),
        "excel_where_am_i" => where_am_i::run(args),
        other => err_json(&ToolError::new("NOT_IMPLEMENTED", format!("未知 Excel 工具 {other}"))),
    }
}
pub fn handle() -> &'static StaHandle { STA.handle("excel-sta", execute) } // 供 shutdown 使用