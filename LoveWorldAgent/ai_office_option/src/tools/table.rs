use serde_json::{json, Map, Value}; // JSON 构造
use crate::com::dispatch::{get_args, call, get, set, ComObj, ComResult, Variant}; // COM 工具
use crate::com::document::active_document; // 文档上下文
use crate::com::constants::*; // 常量
use crate::word::paths::Paths; // 路径
use crate::word::report::{err_json, ok_json, ToolError}; // 信封
fn arg<'a>(args: &'a Map<String, Value>, key: &str) -> Option<&'a Value> { args.get(key) } // 取参数
fn str_arg(args: &Map<String, Value>, key: &str) -> String { arg(args, key).and_then(|v| v.as_str()).unwrap_or("").to_string() } // 字符串
fn i32_arg(args: &Map<String, Value>, key: &str, d: i32) -> i32 { arg(args, key).and_then(|v| v.as_i64()).unwrap_or(d as i64) as i32 } // 整数
fn f64_arg(args: &Map<String, Value>, key: &str, d: f64) -> f64 { arg(args, key).and_then(|v| v.as_f64()).unwrap_or(d) } // 浮点
pub fn run(args: &Map<String, Value>) -> Value { // word_table 17 action
    let action = str_arg(args, "action"); // 动作
    let path = str_arg(args, "path"); // 路径
    let paths = Paths::load(); // 策略
    let ctx = match active_document(&path, &paths) { Ok(c) => c, Err(e) => return err_json(&e) }; // 取文档
    if let Err(err) = crate::com::document::ensure_writable(&ctx.app) { return err_json(&err); } // 对话框拦截
    if ctx.read_only() { return err_json(&ToolError::new("READ_ONLY", "文档为只读，无法修改表格").recoverable("取消只读或另存为可写文档后重试")); } // 只读拦截
    let result: ComResult<Value> = match action.as_str() { // 分派
        "insert" => insert(&ctx, args), // 插入
        "edit_cell" => edit_cell(&ctx, args, false), // 单格
        "edit_cells" => edit_cell(&ctx, args, true), // 批量
        "add_row" => add_row(&ctx, args), // 加行
        "delete_row" => delete_row(&ctx, args), // 删行
        "add_column" => add_column(&ctx, args), // 加列
        "delete_column" => delete_column(&ctx, args), // 删列
        "set_borders" => set_borders(&ctx, args), // 边框
        "set_shading" => set_shading(&ctx, args), // 底纹
        "merge" => merge(&ctx, args), // 合并
        "set_column_width" => set_column_width(&ctx, args), // 列宽
        "set_row_height" => set_row_height(&ctx, args), // 行高
        "set_cell_font" => set_cell_font(&ctx, args), // 单元格字体
        "set_cell_alignment" => set_cell_alignment(&ctx, args), // 垂直对齐
        "apply_style" => apply_style(&ctx, args), // 样式
        "to_text" => to_text(&ctx, args), // 转文本
        "from_text" => from_text(&ctx, args), // 文本转表格
        other => Err(ToolError::new("INVALID_ARGUMENT", format!("未知 table action：{other}"))), // 未知
    };
    match result { Ok(v) => { let _ = move_after_table(&ctx); ok_json(v) }, Err(e) => err_json(&e) } // 返回
}
fn move_after_table(ctx: &crate::com::document::DocCtx) -> ComResult<()> { // 表格操作后移出表格
    let app = get(&ctx.doc, "Application")?.as_dispatch()?; // Application
    let sel = get(&app, "Selection")?.as_dispatch()?; // Selection
    let tables = get(&sel, "Tables")?.as_dispatch()?; // 当前表格集
    let count = get(&tables, "Count").map(|v| v.as_i32()).unwrap_or(0); // 数量
    if count > 0 { // 位于表格内
        let table = call(&tables, "Item", &[Variant::I4(1)])?.as_dispatch()?; // 表格
        let range = get(&table, "Range")?.as_dispatch()?; // 范围
        let end = get(&range, "End")?.as_i32(); // 表格末尾
        let position = (end + 1).max(1); // 下一段起点
        call(&sel, "SetRange", &[Variant::I4(position), Variant::I4(position)])?; // 移动光标
    }
    Ok(()) // 完成
}
fn table_at(ctx: &crate::com::document::DocCtx, args: &Map<String, Value>) -> ComResult<ComObj> { // 按序号取表格
    let index = i32_arg(args, "tableIndex", 1).max(1); // 序号
    let tables = get(&ctx.doc, "Tables")?.as_dispatch()?; // 集合
    let count = get(&tables, "Count")?.as_i32(); // 数量
    if index > count { return Err(ToolError::new("INVALID_ARGUMENT", format!("tableIndex={index} > tables={count}"))); } // 越界
    call(&tables, "Item", &[Variant::I4(index)])?.as_dispatch() // 返回
}
fn cell(table: &ComObj, row: i32, col: i32) -> ComResult<ComObj> { // 取单元格并校验越界
    let rows = get(table, "Rows")?.as_dispatch().and_then(|r| get(&r, "Count")).map(|v| v.as_i32()).unwrap_or(0); // 行
    let cols = get(table, "Columns")?.as_dispatch().and_then(|c| get(&c, "Count")).map(|v| v.as_i32()).unwrap_or(0); // 列
    if row < 1 || row > rows { return Err(ToolError::new("INVALID_ARGUMENT", format!("row={row} > rows={rows}"))); } // 行越界
    if col < 1 || col > cols { return Err(ToolError::new("INVALID_ARGUMENT", format!("column={col} > cols={cols}"))); } // 列越界
    call(table, "Cell", &[Variant::I4(row), Variant::I4(col)])?.as_dispatch() // 返回
}
fn write_cell(table: &ComObj, row: i32, col: i32, text: &str) -> ComResult<()> { // 写单元格
    let c = cell(table, row, col)?; // 单元格
    let range = get(&c, "Range")?.as_dispatch()?; // Range
    set(&range, "Text", Variant::Str(text.to_string())) // 写
}
fn insert(ctx: &crate::com::document::DocCtx, args: &Map<String, Value>) -> ComResult<Value> { // 插入表格
    let rows = i32_arg(args, "rows", 1); // 行
    let cols = i32_arg(args, "columns", 1); // 列
    if rows < 1 || rows > 500 || cols < 1 || cols > 100 { return Err(ToolError::new("INVALID_ARGUMENT", format!("rows={rows}/columns={cols} 超出 1~500/1~100"))); } // 上限校验
    let app = get(&ctx.doc, "Application")?.as_dispatch()?; // Application
    let sel = get(&app, "Selection")?.as_dispatch()?; // Selection
    let anchor = get(&sel, "Range")?.as_dispatch()?; // 选区 Range
    let tables = get(&ctx.doc, "Tables")?.as_dispatch()?; // 集合
    let table = call(&tables, "Add", &[Variant::Dispatch(anchor), Variant::I4(rows), Variant::I4(cols)])?.as_dispatch()?; // 添加
    if let Some(autofit) = arg(args, "autoFitBehavior") { // 自适应
        let value = match autofit.as_str().unwrap_or("") { "contents" => WD_AUTO_FIT_CONTENT, "window" => WD_AUTO_FIT_WINDOW, _ => WD_AUTO_FIT_FIXED }; // 映射
        call(&table, "AutoFitBehavior", &[Variant::I4(value)])?; // 设置
    }
    if let Some(data) = arg(args, "data").and_then(|v| v.as_array()) { // 数据
        for (r, row) in data.iter().enumerate() { // 逐行
            if let Some(cells) = row.as_array() { // 行数组
                for (c, val) in cells.iter().enumerate() { // 逐列
                    write_cell(&table, r as i32 + 1, c as i32 + 1, &json_str(val))?; // 写
                }
            }
        }
    }
    Ok(json!({"rows": rows, "columns": cols})) // 返回
}
fn edit_cell(ctx: &crate::com::document::DocCtx, args: &Map<String, Value>, batch: bool) -> ComResult<Value> { // 编辑单元格
    let table = table_at(ctx, args)?; // 表格
    if batch { // 批量
        let rows = get(&table, "Rows")?.as_dispatch().and_then(|v| get(&v, "Count")).map(|v| v.as_i32()).unwrap_or(0); // 行数
        let cols = get(&table, "Columns")?.as_dispatch().and_then(|v| get(&v, "Count")).map(|v| v.as_i32()).unwrap_or(0); // 列数
        let start_row = i32_arg(args, "row", 1); // 起始行
        let start_col = i32_arg(args, "column", 1); // 起始列
        if start_row < 1 || start_row > rows || start_col < 1 || start_col > cols { return Err(ToolError::new("INVALID_ARGUMENT", format!("row={start_row}/column={start_col} 超出表格 {rows}x{cols}"))); } // 越界
        let data = arg(args, "data").and_then(|v| v.as_array()).ok_or_else(|| ToolError::new("INVALID_ARGUMENT", "data 必须是二维数组"))?; // 数据
        let mut count = 0; // 计数
        for (r, row) in data.iter().enumerate() { // 逐行
            if let Some(cells) = row.as_array() { // 行
                for (c, val) in cells.iter().enumerate() { // 列
                    let row_index = start_row + r as i32; // 实际行（相对起始行）
                    let col_index = start_col + c as i32; // 实际列（相对起始列）
                    if row_index > rows || col_index > cols { return Err(ToolError::new("INVALID_ARGUMENT", format!("数据超出表格范围：写入 ({row_index},{col_index})，表格 {rows}x{cols}"))); } // 越界
                    write_cell(&table, row_index, col_index, &json_str(val))?; // 写
                    count += 1; // 计数
                }
            }
        }
        Ok(json!({"cells": count, "startRow": start_row, "startColumn": start_col})) // 返回
    } else { // 单格
        let row = i32_arg(args, "row", 1); // 行
        let col = i32_arg(args, "column", 1); // 列
        let text = str_arg(args, "text"); // 文本
        write_cell(&table, row, col, &text)?; // 写
        Ok(json!({"row": row, "column": col})) // 返回
    }
}
fn add_row(ctx: &crate::com::document::DocCtx, args: &Map<String, Value>) -> ComResult<Value> { // 加行
    let table = table_at(ctx, args)?; // 表格
    let rows = get(&table, "Rows")?.as_dispatch()?; // 行集合
    call(&rows, "Add", &[])?; // 添加（忽略返回值：复杂表格可能不返回 Row 对象）
    let index = get(&rows, "Count").map(|v| v.as_i32()).unwrap_or(1).max(1); // 新行位于末尾
    if let Some(data) = arg(args, "data").and_then(|v| v.as_array()) { // 数据
        let cols = get(&table, "Columns").and_then(|v| v.as_dispatch()).and_then(|c| get(&c, "Count")).map(|v| v.as_i32()).unwrap_or(0); // 实际列数
        for (c, val) in data.iter().enumerate() { // 逐列
            if c as i32 >= cols { break; } // 不越界
            write_cell(&table, index, c as i32 + 1, &json_str(val))?; // 写
        }
    }
    Ok(json!({"added": true, "index": index})) // 返回
}
fn delete_row(ctx: &crate::com::document::DocCtx, args: &Map<String, Value>) -> ComResult<Value> { // 删行
    let table = table_at(ctx, args)?; // 表格
    let index = i32_arg(args, "rowIndex", 1); // 行号
    let rows = get(&table, "Rows")?.as_dispatch()?; // 集合
    let count = get(&rows, "Count")?.as_i32(); // 数量
    if index < 1 || index > count { return Err(ToolError::new("INVALID_ARGUMENT", format!("rowIndex={index} > rows={count}"))); } // 越界
    let row = call(&rows, "Item", &[Variant::I4(index)])?.as_dispatch()?; // 行
    call(&row, "Delete", &[])?; // 删除
    Ok(json!({"deleted": true})) // 返回
}
fn add_column(ctx: &crate::com::document::DocCtx, args: &Map<String, Value>) -> ComResult<Value> { // 加列
    let table = table_at(ctx, args)?; // 表格
    let cols = get(&table, "Columns")?.as_dispatch()?; // 集合
    let before = i32_arg(args, "column", 0); // 前插列
    let count = get(&cols, "Count").map(|v| v.as_i32()).unwrap_or(0); // 列数
    if before < 0 || before > count + 1 { return Err(ToolError::new("INVALID_ARGUMENT", format!("column={before} 超出 0..={}", count + 1))); } // 越界
    if before > 0 { // 指定位置
        let col = call(&cols, "Item", &[Variant::I4(before)])?.as_dispatch()?; // 列
        call(&col, "Select", &[])?; // 选中
        call(&cols, "Add", &[])?; // 添加
    } else { call(&cols, "Add", &[])?; } // 末尾加
    Ok(json!({"added": true})) // 返回
}
fn delete_column(ctx: &crate::com::document::DocCtx, args: &Map<String, Value>) -> ComResult<Value> { // 删列
    let table = table_at(ctx, args)?; // 表格
    let index = i32_arg(args, "column", 1); // 列
    let cols = get(&table, "Columns")?.as_dispatch()?; // 集合
    let count = get(&cols, "Count")?.as_i32(); // 数量
    if index < 1 || index > count { return Err(ToolError::new("INVALID_ARGUMENT", format!("column={index} > cols={count}"))); } // 越界
    let col = call(&cols, "Item", &[Variant::I4(index)])?.as_dispatch()?; // 列
    call(&col, "Delete", &[])?; // 删除
    Ok(json!({"deleted": true})) // 返回
}
fn set_borders(ctx: &crate::com::document::DocCtx, args: &Map<String, Value>) -> ComResult<Value> { // 边框
    let table = table_at(ctx, args)?; // 表格
    let borders = get(&table, "Borders")?.as_dispatch()?; // 边框集合
    let inside = arg(args, "inside").and_then(|v| v.as_object()); // 内框
    let outside = arg(args, "outside").and_then(|v| v.as_object()); // 外框
    if inside.is_none() && outside.is_none() { // 兼容顶层 style
        let style = arg(args, "style").map(line_style).unwrap_or(1); // 线型
        for index in [WD_BORDER_TOP, WD_BORDER_LEFT, WD_BORDER_BOTTOM, WD_BORDER_RIGHT, WD_BORDER_HORIZONTAL, WD_BORDER_VERTICAL] { // 六边
            let border = call(&borders, "Item", &[Variant::I4(index)])?.as_dispatch()?; // 边框
            set(&border, "LineStyle", Variant::I4(style))?; // 线型
        }
        return Ok(json!({"applied": true})); // 返回
    }
    for (index, spec) in [(WD_BORDER_HORIZONTAL, inside), (WD_BORDER_VERTICAL, inside), (WD_BORDER_TOP, outside), (WD_BORDER_LEFT, outside), (WD_BORDER_BOTTOM, outside), (WD_BORDER_RIGHT, outside)] { // 六个方向
        let Some(spec) = spec else { continue; }; // 未指定
        let border = call(&borders, "Item", &[Variant::I4(index)])?.as_dispatch()?; // 边框
        set(&border, "LineStyle", Variant::I4(spec.get("style").map(line_style).unwrap_or(1)))?; // 线型
        if let Some(size) = spec.get("size").and_then(|v| v.as_i64()) { let _ = set(&border, "LineWidth", Variant::I4(size.clamp(2, 48) as i32)); } // 宽度（部分版本不支持，忽略）
        if let Some(color) = spec.get("color").and_then(|v| v.as_str()).and_then(parse_bgr) { set(&border, "Color", Variant::I4(color))?; } // 颜色
    }
    Ok(json!({"applied": true})) // 返回
}
fn line_style(value: &Value) -> i32 { // 边框线型映射
    if let Some(number) = value.as_i64() { return number as i32; } // 数字直传
    match value.as_str().unwrap_or("single") { "none" => 0, "dot" => 2, "dashSmallGap" => 3, "dash" => 4, "dashLargeGap" => 5, "dashDot" => 6, "double" => 8, _ => 1 } // 枚举
}
fn set_shading(ctx: &crate::com::document::DocCtx, args: &Map<String, Value>) -> ComResult<Value> { // 底纹
    let table = table_at(ctx, args)?; // 表格
    let color = parse_bgr(&str_arg(args, "color")).unwrap_or(BGR_WHITE); // 颜色
    let target = str_arg(args, "target"); // 目标
    let shading = if target == "row" { // 行
        let row = call(&get(&table, "Rows")?.as_dispatch()?, "Item", &[Variant::I4(i32_arg(args, "rowIndex", 1))])?.as_dispatch()?; // 行
        get(&row, "Shading")?.as_dispatch()? // 底纹
    } else { get(&table, "Shading")?.as_dispatch()? }; // 表
    set(&shading, "BackgroundPatternColor", Variant::I4(color))?; // 设置
    Ok(json!({"applied": true})) // 返回
}
fn merge(ctx: &crate::com::document::DocCtx, args: &Map<String, Value>) -> ComResult<Value> { // 合并
    let table = table_at(ctx, args)?; // 表格
    let start = cell(&table, i32_arg(args, "rowStart", 1), i32_arg(args, "colStart", 1))?; // 起
    let end = cell(&table, i32_arg(args, "rowEnd", 1), i32_arg(args, "colEnd", 1))?; // 止
    call(&start, "Merge", &[Variant::Dispatch(end)])?; // 合并
    Ok(json!({"merged": true})) // 返回
}
fn set_column_width(ctx: &crate::com::document::DocCtx, args: &Map<String, Value>) -> ComResult<Value> { // 列宽
    let table = table_at(ctx, args)?; // 表格
    let index = i32_arg(args, "column", 1); // 列
    let width = f64_arg(args, "width", 50.0); // 宽
    if !(1.0..=500.0).contains(&width) { return Err(ToolError::new("INVALID_ARGUMENT", format!("width={width} 超出 1~500"))); } // 范围
    let col = call(&get(&table, "Columns")?.as_dispatch()?, "Item", &[Variant::I4(index)])?.as_dispatch()?; // 列
    set(&col, "Width", Variant::R8(width))?; // 设置
    Ok(json!({"applied": true})) // 返回
}
fn set_row_height(ctx: &crate::com::document::DocCtx, args: &Map<String, Value>) -> ComResult<Value> { // 行高
    let table = table_at(ctx, args)?; // 表格
    let index = i32_arg(args, "row", 1); // 行
    let height = f64_arg(args, "height", 20.0); // 高
    if !(1.0..=500.0).contains(&height) { return Err(ToolError::new("INVALID_ARGUMENT", format!("height={height} 超出 1~500"))); } // 范围
    let row = call(&get(&table, "Rows")?.as_dispatch()?, "Item", &[Variant::I4(index)])?.as_dispatch()?; // 行
    set(&row, "HeightRule", Variant::I4(1))?; // AtLeast
    set(&row, "Height", Variant::R8(height))?; // 设置
    Ok(json!({"applied": true})) // 返回
}
fn set_cell_font(ctx: &crate::com::document::DocCtx, args: &Map<String, Value>) -> ComResult<Value> { // 单元格字体
    let table = table_at(ctx, args)?; // 表格
    let c = cell(&table, i32_arg(args, "row", 1), i32_arg(args, "column", 1))?; // 单元格
    let font = get(&get(&c, "Range")?.as_dispatch()?, "Font")?.as_dispatch()?; // Font
    if let Some(name) = arg(args, "name").and_then(|v| v.as_str()) { set(&font, "Name", Variant::Str(name.to_string()))?; } // 字体名
    if let Some(size) = arg(args, "size").and_then(|v| v.as_f64()) { set(&font, "Size", Variant::R8(size))?; } // 字号
    if let Some(bold) = arg(args, "bold").and_then(|v| v.as_bool()) { set(&font, "Bold", Variant::I4(if bold { -1 } else { 0 }))?; } // 粗体
    if let Some(italic) = arg(args, "italic").and_then(|v| v.as_bool()) { set(&font, "Italic", Variant::I4(if italic { -1 } else { 0 }))?; } // 斜体
    if let Some(strike) = arg(args, "strikethrough").and_then(|v| v.as_bool()) { set(&font, "StrikeThrough", Variant::I4(if strike { -1 } else { 0 }))?; } // 删除线
    if let Some(underline) = arg(args, "underline").and_then(|v| v.as_str()) { let value = match underline { "double" => WD_UNDERLINE_DOUBLE, "wavy" => WD_UNDERLINE_WAVY, "single" => WD_UNDERLINE_SINGLE, _ => WD_UNDERLINE_NONE }; set(&font, "Underline", Variant::I4(value))?; } // 下划线
    if let Some(color) = arg(args, "color").and_then(|v| v.as_str()).and_then(parse_bgr) { set(&font, "Color", Variant::I4(color))?; } // 颜色
    Ok(json!({"applied": true})) // 返回
}
fn set_cell_alignment(ctx: &crate::com::document::DocCtx, args: &Map<String, Value>) -> ComResult<Value> { // 垂直对齐
    let table = table_at(ctx, args)?; // 表格
    let c = cell(&table, i32_arg(args, "row", 1), i32_arg(args, "column", 1))?; // 单元格
    let align = match str_arg(args, "alignment").as_str() { "center" => WD_CELL_ALIGN_VERTICAL_CENTER, "bottom" => WD_CELL_ALIGN_VERTICAL_BOTTOM, _ => WD_CELL_ALIGN_VERTICAL_TOP }; // 映射
    set(&c, "VerticalAlignment", Variant::I4(align))?; // 设置
    Ok(json!({"applied": true})) // 返回
}
fn apply_style(ctx: &crate::com::document::DocCtx, args: &Map<String, Value>) -> ComResult<Value> { // 表格样式
    let table = table_at(ctx, args)?; // 表格
    let name = str_arg(args, "styleName"); // 样式名
    if set(&table, "Style", Variant::Str(name.clone())).is_err() { // 失败回退按本名枚举
        let styles = get(&ctx.doc, "Styles")?.as_dispatch()?; // 样式集
        let count = get(&styles, "Count")?.as_i32(); // 数量
        let mut index = 1; // 遍历
        while index <= count { // 逐样式
            if let Ok(style) = call(&styles, "Item", &[Variant::I4(index)]).and_then(|v| v.as_dispatch()) { // 样式
                if get(&style, "NameLocal").map(|v| v.as_string()).unwrap_or_default() == name { set(&table, "Style", Variant::Dispatch(style))?; break; } // 命中设置
            }
            index += 1; // 下一项
        }
    }
    Ok(json!({"applied": true})) // 返回
}
fn to_text(ctx: &crate::com::document::DocCtx, args: &Map<String, Value>) -> ComResult<Value> { // 转文本
    let table = table_at(ctx, args)?; // 表格
    let sep = str_arg(args, "separator"); // 分隔符
    let separator = if sep.is_empty() { "\t".to_string() } else { sep }; // 默认制表符
    call(&table, "ConvertToText", &[Variant::Str(separator), Variant::Bool(true)])?; // 转换
    Ok(json!({"converted": true})) // 返回
}
fn from_text(ctx: &crate::com::document::DocCtx, args: &Map<String, Value>) -> ComResult<Value> { // 文本转表格
    let app = get(&ctx.doc, "Application")?.as_dispatch()?; // Application
    let sel = get(&app, "Selection")?.as_dispatch()?; // Selection
    let sep = str_arg(args, "separator"); // 分隔符
    let args = if sep.is_empty() { vec![] } else { vec![Variant::I4(9)] }; // 参数（wdSeparateByTabs=9）
    call(&sel, "ConvertToTable", &args)?; // 转换
    Ok(json!({"converted": true})) // 返回
}
fn json_str(value: &Value) -> String { // JSON 值转字符串
    match value { Value::String(s) => s.clone(), Value::Number(n) => n.to_string(), Value::Bool(b) => b.to_string(), _ => String::new() } // 分派
}
fn parse_bgr(color: &str) -> Option<i32> { // #RRGGBB → BGR
    let hex = color.trim_start_matches('#'); // 去 #
    if hex.len() != 6 { return None; } // 长度
    let r = u8::from_str_radix(&hex[0..2], 16).ok()? as i32; // 红
    let g = u8::from_str_radix(&hex[2..4], 16).ok()? as i32; // 绿
    let b = u8::from_str_radix(&hex[4..6], 16).ok()? as i32; // 蓝
    Some(r + g * 256 + b * 65536) // BGR
}
