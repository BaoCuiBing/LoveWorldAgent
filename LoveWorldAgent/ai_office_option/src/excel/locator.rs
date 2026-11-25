use serde_json::{json, Map, Value}; // JSON 构造
use std::path::{Path, PathBuf}; // 路径类型
use crate::com::constants::*; // Office 常量
use crate::com::dispatch::{call, get, set, ComObj, ComResult, Variant}; // COM 工具
use crate::common::paths::Paths; // 路径白名单
use crate::common::progid::ensure_app; // 应用附着/创建
use crate::word::instances::same_path; // 路径等价
use crate::word::report::ToolError; // 错误类型

pub struct BookCtx { // 工作簿上下文
    pub app: ComObj, // Application
    pub book: ComObj, // Workbook
    pub path: PathBuf, // 完整路径
}

pub fn arg<'a>(args: &'a Map<String, Value>, key: &str) -> Option<&'a Value> { args.get(key) } // 取参数
pub fn str_arg(args: &Map<String, Value>, key: &str) -> String { arg(args, key).and_then(|v| v.as_str()).unwrap_or("").to_string() } // 字符串
pub fn i32_arg(args: &Map<String, Value>, key: &str, d: i32) -> i32 { arg(args, key).and_then(|v| v.as_i64()).unwrap_or(d as i64) as i32 } // 整数
pub fn f64_arg(args: &Map<String, Value>, key: &str, d: f64) -> f64 { arg(args, key).and_then(|v| v.as_f64()).unwrap_or(d) } // 浮点

pub fn workbooks(app: &ComObj) -> ComResult<ComObj> { get(app, "Workbooks")?.as_dispatch() } // 工作簿集合

pub fn book_of(app: &ComObj, full: &Path) -> ComResult<Option<ComObj>> { // 按路径查已打开工作簿
    let collection = workbooks(app)?; // 集合
    let count = get(&collection, "Count")?.as_i32(); // 数量
    for index in 1..=count { // 逐个
        if let Ok(book) = call(&collection, "Item", &[Variant::I4(index)]).and_then(|v| v.as_dispatch()) { // 工作簿
            let name = get(&book, "FullName").map(|v| v.as_string()).unwrap_or_default(); // 路径
            if same_path(&name, &full.to_string_lossy()) { let _ = call(&book, "Activate", &[]); return Ok(Some(book)); } // 命中
        }
    }
    Ok(None) // 未命中
}

pub fn open(path: &str) -> ComResult<BookCtx> { // 打开工作簿
    let full = Paths::load().resolve(path)?; // 校验
    if !full.is_file() { return Err(ToolError::new("FILE_NOT_FOUND", format!("工作簿不存在：{}", full.display()))); } // 不存在
    let full_str = full.to_string_lossy().to_string(); // 字符串路径
    if let Some(entry) = crate::common::registry::get(&full_str, EXCEL_PROGID) { // 路径已登记
        let _ = call(&entry.doc, "Activate", &[]); // 激活
        return Ok(BookCtx { app: entry.app, book: entry.doc, path: full }); // 复用
    }
    let app = ensure_app(EXCEL_PROGID, true)?; // 附着或创建
    if let Some(book) = book_of(&app, &full)? { crate::common::registry::put(&full_str, EXCEL_PROGID, &app, &book); return Ok(BookCtx { app, book, path: full }); } // 已打开
    let collection = workbooks(&app)?; // 集合
    let book = call(&collection, "Open", &[Variant::Str(full_str.clone())])?.as_dispatch()?; // 打开
    let _ = call(&book, "Activate", &[]); // 激活
    crate::common::registry::put(&full_str, EXCEL_PROGID, &app, &book); // 登记路径
    Ok(BookCtx { app, book, path: full }) // 返回
}

pub fn create(path: &str) -> ComResult<BookCtx> { // 新建工作簿
    let full = Paths::load().resolve(path)?; // 校验
    if full.exists() { return Err(ToolError::new("FILE_EXISTS", format!("文件已存在：{}", full.display()))); } // 已存在
    if let Some(parent) = full.parent() { if !parent.is_dir() { return Err(ToolError::new("INVALID_ARGUMENT", format!("父目录不存在：{}", parent.display()))); } } // 父目录
    let app = ensure_app(EXCEL_PROGID, true)?; // 附着或创建
    let collection = workbooks(&app)?; // 集合
    let book = call(&collection, "Add", &[])?.as_dispatch()?; // 新建
    let sheets = get(&book, "Worksheets")?.as_dispatch()?; // 工作表集
    if let Ok(first) = call(&sheets, "Item", &[Variant::I4(1)]).and_then(|v| v.as_dispatch()) { if get(&first, "Name").map(|v| v.as_string()).unwrap_or_default() != "Sheet1" { let _ = set(&first, "Name", Variant::Str("Sheet1".to_string())); } } // 统一首表名
    call(&book, "SaveAs", &[Variant::Str(full.to_string_lossy().to_string()), Variant::I4(XL_OPEN_XML_WORKBOOK)])?; // 保存为 xlsx
    crate::common::registry::put(&full.to_string_lossy(), EXCEL_PROGID, &app, &book); // 登记路径
    Ok(BookCtx { app, book, path: full }) // 返回
}

pub fn ensure(path: &str) -> ComResult<BookCtx> { // 打开或新建
    let full = Paths::load().resolve(path)?; // 路径
    if full.is_file() { open(path) } else { create(path) } // 分派
}

pub fn collection_item(collection: &ComObj, index: Variant) -> ComResult<ComObj> { // 集合 Item（Excel 优先方法，再默认成员）
    let args = [index]; // 参数
    crate::com::dispatch::call(collection, "Item", &args).ok().and_then(|v| v.as_dispatch().ok()) // Item 方法
        .or_else(|| crate::com::dispatch::get_args(collection, "Item", &args).ok().and_then(|v| v.as_dispatch().ok())) // Item 属性
        .or_else(|| crate::com::dispatch::get_args(collection, "", &args).ok().and_then(|v| v.as_dispatch().ok())) // 默认成员属性
        .or_else(|| crate::com::dispatch::call(collection, "", &args).ok().and_then(|v| v.as_dispatch().ok())) // 默认成员方法
        .ok_or_else(|| ToolError::new("COM_MEMBER_NOT_FOUND", "集合不支持 Item 访问"))
}
fn cells_item(cells: &ComObj, row: i32, column: i32) -> ComResult<ComObj> { // 单元格集 Item(row,col)
    let args = [Variant::I4(row), Variant::I4(column)]; // 参数
    crate::com::dispatch::get_args(cells, "Item", &args).ok().and_then(|v| v.as_dispatch().ok()) // 属性
        .or_else(|| crate::com::dispatch::call(cells, "Item", &args).ok().and_then(|v| v.as_dispatch().ok())) // 方法
        .ok_or_else(|| ToolError::new("INVALID_ARGUMENT", format!("单元格无效：({row},{column})")))
}
pub fn sheet(book: &ComObj, name: &str) -> ComResult<ComObj> { // 按名称/活动工作表
    if name.trim().is_empty() { return get(book, "ActiveSheet")?.as_dispatch(); } // 活动工作表
    let sheets = get(book, "Worksheets")?.as_dispatch()?; // 工作表集
    if let Ok(sheet) = collection_item(&sheets, Variant::Str(name.to_string())) { let got = get(&sheet, "Name").map(|v| v.as_string()).unwrap_or_default(); if got.is_empty() || got.eq_ignore_ascii_case(name) { return Ok(sheet); } } // 按名称且校验类型
    if name.eq_ignore_ascii_case("Sheet1") { // 首表兜底
        if let Ok(active) = get(book, "ActiveSheet").and_then(|v| v.as_dispatch()) { if get(&active, "Name").map(|v| v.as_string()).unwrap_or_default().eq_ignore_ascii_case(name) { return Ok(active); } } // 活动工作表
        if let Ok(sheet) = collection_item(&sheets, Variant::I4(1)) { return Ok(sheet); } // 第一张表
    }
    Err(ToolError::new("INVALID_ARGUMENT", format!("工作表不存在：{name}"))) // 返回
}

pub fn range(sheet: &ComObj, address: &str) -> ComResult<ComObj> { // 按 A1 地址取区域
    let address = if address.trim().is_empty() { "A1" } else { address.trim() }; // 默认 A1
    let args = [Variant::Str(address.to_string())]; // 参数
    crate::com::dispatch::get_args(sheet, "Range", &args).ok().and_then(|v| v.as_dispatch().ok()) // 属性
        .or_else(|| call(sheet, "Range", &args).ok().and_then(|v| v.as_dispatch().ok())) // 方法回退
        .ok_or_else(|| ToolError::new("INVALID_ARGUMENT", format!("区域地址无效：{address}")))
}
pub fn cell(sheet: &ComObj, row: i32, column: i32) -> ComResult<ComObj> { // 按行列取单元格
    let cells = get(sheet, "Cells")?.as_dispatch()?; // 单元格集
    cells_item(&cells, row, column) // 统一 Item 回退
}


pub fn read_cell(value: &ComObj) -> Value { get(value, "Value2").map(|item| item.to_json()).unwrap_or(Value::Null) } // 读单元格值

pub fn write_cell(target: &ComObj, value: &Value) -> ComResult<()> { // 写单元格值
    let variant = Variant::from_json(value)?; // JSON → VARIANT
    set(target, "Value2", variant) // 写入
}

pub fn matrix_of_range(target: &ComObj) -> ComResult<Value> { // 读取区域二维数组
    let rows = get(&get(target, "Rows")?.as_dispatch()?, "Count")?.as_i32(); // 行数
    let cols = get(&get(target, "Columns")?.as_dispatch()?, "Count")?.as_i32(); // 列数
    let cells = get(target, "Cells")?.as_dispatch()?; // 单元格集
    let mut data = Vec::new(); // 数据
    for row in 1..=rows { // 逐行
        let mut values = Vec::new(); // 行数据
        for column in 1..=cols { // 逐列
            let item = cells_item(&cells, row, column)?; // 单元格
            values.push(read_cell(&item)); // 值
        }
        data.push(Value::Array(values)); // 收集
    }
    let address = get(target, "Address").map(|v| v.as_string()).unwrap_or_default(); // 地址
    Ok(json!({"address": address, "rows": rows, "columns": cols, "data": data})) // 返回
}

pub fn matrix_of_sheet(sheet: &ComObj, address: &str) -> ComResult<Value> { // 按地址读区域
    matrix_of_range(&range(sheet, address)?) // 委托
}

pub fn write_matrix(sheet: &ComObj, address: &str, data: &[Value]) -> ComResult<usize> { // 按地址写二维数组
    let target = range(sheet, address)?; // 区域
    let rows = get(&get(&target, "Rows")?.as_dispatch()?, "Count")?.as_i32(); // 行
    let cols = get(&get(&target, "Columns")?.as_dispatch()?, "Count")?.as_i32(); // 列
    let cells = get(&target, "Cells")?.as_dispatch()?; // 单元格集
    let mut count = 0usize; // 计数
    for (row, values) in data.iter().enumerate() { // 逐行
        if row as i32 >= rows { break; } // 超行跳过
        if let Some(values) = values.as_array() { for (column, value) in values.iter().enumerate() { if column as i32 >= cols { break; } let item = cells_item(&cells, row as i32 + 1, column as i32 + 1)?; write_cell(&item, value)?; count += 1; } } // 逐列
    }
    Ok(count) // 返回
}

pub fn rgb(hex: &str) -> Option<i32> { // #RRGGBB → BGR
    let text = hex.trim().trim_start_matches('#'); // 去 #
    if text.len() != 6 { return None; } // 长度
    let r = i32::from_str_radix(&text[0..2], 16).ok()?; // 红
    let g = i32::from_str_radix(&text[2..4], 16).ok()?; // 绿
    let b = i32::from_str_radix(&text[4..6], 16).ok()?; // 蓝
    Some(r + g * 256 + b * 65536) // BGR
}

pub fn info_json(ctx: &BookCtx) -> ComResult<Value> { // 工作簿信息
    let sheets = get(&ctx.book, "Worksheets")?.as_dispatch()?; // 工作表集
    let count = get(&sheets, "Count")?.as_i32(); // 数量
    let name = get(&ctx.book, "Name").map(|v| v.as_string()).unwrap_or_default(); // 名称
    let saved = get(&ctx.book, "Saved").map(|v| v.as_bool()).unwrap_or(true); // 已保存
    let read_only = get(&ctx.book, "ReadOnly").map(|v| v.as_bool()).unwrap_or(false); // 只读
    Ok(json!({"path": ctx.path.to_string_lossy(), "name": name, "sheetCount": count, "saved": saved, "readOnly": read_only})) // 返回
}

pub fn save(ctx: &BookCtx) -> ComResult<Value> { // 保存并备份
    let backup = crate::word::backup::backup_before_save(&ctx.path)?; // 备份
    call(&ctx.book, "Save", &[])?; // 保存
    let bytes = std::fs::metadata(&ctx.path).map(|m| m.len() as i64).unwrap_or(0); // 字节数
    Ok(json!({"path": ctx.path.to_string_lossy(), "bytes": bytes, "backupPath": backup.map(|p| p.to_string_lossy().to_string())})) // 返回
}

pub fn close(ctx: &BookCtx, save_changes: bool) -> ComResult<()> { // 关闭工作簿
    call(&ctx.book, "Close", &[Variant::I4(if save_changes { MSO_TRUE } else { 0 })]).map(|_| ()) // 关闭
}

pub fn export_pdf(ctx: &BookCtx, target: &str) -> ComResult<Value> { // 导出 PDF
    let full = Paths::load().resolve(target)?; // 目标
    call(&ctx.book, "ExportAsFixedFormat", &[Variant::I4(XL_TYPE_PDF), Variant::Str(full.to_string_lossy().to_string())])?; // 导出
    Ok(json!({"pdfPath": full.to_string_lossy()})) // 返回
}

pub fn list_sheets(book: &ComObj) -> ComResult<Vec<Value>> { // 工作表列表
    let sheets = get(book, "Worksheets")?.as_dispatch()?; // 集合
    let count = get(&sheets, "Count")?.as_i32(); // 数量
    let mut items = Vec::new(); // 结果
    for index in 1..=count { // 逐个
        if let Ok(sheet) = call(&sheets, "Item", &[Variant::I4(index)]).and_then(|v| v.as_dispatch()) { // 工作表
            items.push(json!({
                "index": index,
                "name": get(&sheet, "Name").map(|v| v.as_string()).unwrap_or_default(),
                "visible": get(&sheet, "Visible").map(|v| v.as_i32()).unwrap_or(-1),
            })); // 收集
        }
    }
    Ok(items) // 返回
}