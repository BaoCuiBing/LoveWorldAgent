use serde_json::{json, Map, Value}; // JSON 构造
use crate::com::dispatch::{attach_app, get, ComResult}; // COM 工具
use crate::com::constants::EXCEL_PROGID; // ProgID
use crate::excel::locator::{self, str_arg}; // 上下文
use crate::word::report::{err_json, ok_json}; // 信封

pub fn run(args: &Map<String, Value>) -> Value { // excel_where_am_i
    let result: ComResult<Value> = (|| { // 错误闭包
        let path = str_arg(args, "path"); // 路径
        if !path.is_empty() { // 指定工作簿
            let ctx = locator::open(&path)?; // 打开
            let sheet = get(&ctx.book, "ActiveSheet")?.as_dispatch()?; // 活动工作表
            return Ok(json!({
                "activeWorkbook": ctx.path.to_string_lossy(),
                "activeSheet": get(&sheet, "Name").map(|v| v.as_string()).unwrap_or_default(),
                "selection": get(&sheet, "Names").ok().map(|_| "A1").unwrap_or("A1"),
                "family": "excel",
            })); // 返回
        }
        let app = attach_app(EXCEL_PROGID)?; // 附着
        let book = get(&app, "ActiveWorkbook")?.as_dispatch()?; // 活动工作簿
        let sheet = get(&book, "ActiveSheet")?.as_dispatch()?; // 活动工作表
        let selection = get(&app, "Selection")?.as_dispatch()?; // 选区
        Ok(json!({
            "activeWorkbook": get(&book, "FullName").map(|v| v.as_string()).unwrap_or_default(),
            "activeSheet": get(&sheet, "Name").map(|v| v.as_string()).unwrap_or_default(),
            "selection": get(&selection, "Address").map(|v| v.as_string()).unwrap_or_default(),
            "family": "excel",
        })) // 返回
    })();
    match result { Ok(value) => ok_json(value), Err(err) => err_json(&err) } // 返回
}