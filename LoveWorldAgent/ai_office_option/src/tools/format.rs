use serde_json::{json, Map, Value}; // JSON 构造
use crate::com::dispatch::{call, get, set, ComObj, ComResult, Variant}; // COM 工具
use crate::com::document::active_document; // 文档上下文
use crate::com::constants::*; // 常量
use crate::word::paths::Paths; // 路径
use crate::word::report::{err_json, ok_json, Report, ToolError, Warning}; // 信封
fn arg<'a>(args: &'a Map<String, Value>, key: &str) -> Option<&'a Value> { args.get(key) } // 取参数
fn str_arg(args: &Map<String, Value>, key: &str) -> String { arg(args, key).and_then(|v| v.as_str()).unwrap_or("").to_string() } // 字符串
fn i32_arg(args: &Map<String, Value>, key: &str, d: i32) -> i32 { arg(args, key).and_then(|v| v.as_i64()).unwrap_or(d as i64) as i32 } // 整数
pub fn run(args: &Map<String, Value>) -> Value { // word_format 8 target
    let target = str_arg(args, "target"); // 目标
    let path = str_arg(args, "path"); // 路径
    let paths = Paths::load(); // 策略
    let ctx = match active_document(&path, &paths) { Ok(c) => c, Err(e) => return err_json(&e) }; // 取文档
    if let Err(err) = crate::com::document::ensure_writable(&ctx.app) { return err_json(&err); } // 对话框拦截
    if ctx.read_only() { return err_json(&ToolError::new("READ_ONLY", "文档为只读，无法应用格式").recoverable("取消只读或另存为可写文档后重试")); } // 只读拦截
    let mut report = Report::default(); // 报告
    let result: ComResult<Value> = match target.as_str() { // 分派
        "font" => font(&ctx, args, &mut report), // 字体
        "paragraph" => paragraph(&ctx, args, &mut report), // 段落
        "style" => style(&ctx, args), // 样式
        "page_setup" => page_setup(&ctx, args), // 页面
        "properties" => properties(&ctx, args), // 属性
        "body_indent" => body_indent(&ctx, args), // 正文缩进
        "track_changes" => track_changes(&ctx, args), // 修订
        "track_apply" => track_apply(&ctx, args), // 接受拒绝
        other => Err(ToolError::new("INVALID_ARGUMENT", format!("未知 format target：{other}"))), // 未知
    };
    match result { Ok(v) => report.ok_json(v), Err(e) => err_json(&e) } // 返回
}
fn selection(ctx: &crate::com::document::DocCtx) -> ComResult<ComObj> { // 取 Selection
    let app = get(&ctx.doc, "Application")?.as_dispatch()?; // Application
    get(&app, "Selection")?.as_dispatch() // Selection
}
fn font(ctx: &crate::com::document::DocCtx, args: &Map<String, Value>, report: &mut Report) -> ComResult<Value> { // 字体格式
    let sel = selection(ctx)?; // Selection
    let range = get(&sel, "Range")?.as_dispatch()?; // Range
    let f = get(&range, "Font")?.as_dispatch()?; // Font
    let mut applied = false; // 标记
    macro_rules! try_set { ($prop:expr, $val:expr) => { if set(&f, $prop, $val).is_err() { report.warn(Warning::new("PARTIAL_APPLY", format!("{} 应用失败", $prop))); } else { applied = true; } }; } // 辅助
    if let Some(v) = arg(args, "name").and_then(|v| v.as_str()) { try_set!("Name", Variant::Str(v.to_string())); } // 字体名
    if let Some(v) = arg(args, "size").and_then(|v| v.as_f64()) { try_set!("Size", Variant::R8(v)); } // 字号
    if let Some(v) = arg(args, "bold").and_then(|v| v.as_bool()) { try_set!("Bold", Variant::I4(if v { -1 } else { 0 })); } // 粗体
    if let Some(v) = arg(args, "italic").and_then(|v| v.as_bool()) { try_set!("Italic", Variant::I4(if v { -1 } else { 0 })); } // 斜体
    if let Some(v) = arg(args, "strikethrough").and_then(|v| v.as_bool()) { try_set!("StrikeThrough", Variant::I4(if v { -1 } else { 0 })); } // 删除线
    if let Some(v) = arg(args, "underline").and_then(|v| v.as_str()) { // 下划线
        let value = match v { "double" => WD_UNDERLINE_DOUBLE, "wavy" => WD_UNDERLINE_WAVY, "single" => WD_UNDERLINE_SINGLE, _ => WD_UNDERLINE_NONE }; // 映射
        try_set!("Underline", Variant::I4(value)); // 设置
    }
    if let Some(v) = arg(args, "color").and_then(|v| v.as_str()) { if let Some(bgr) = parse_bgr(v) { try_set!("Color", Variant::I4(bgr)); } } // 颜色
    if !applied { return Err(ToolError::new("FORMAT_APPLY_FAILED", "没有任何格式字段成功应用")); } // 失败
    report.partial = !report.warnings.is_empty(); // 部分成功
    Ok(json!({"applied": true})) // 返回
}
fn paragraph(ctx: &crate::com::document::DocCtx, args: &Map<String, Value>, report: &mut Report) -> ComResult<Value> { // 段落格式
    let sel = selection(ctx)?; // Selection
    let pf = get(&sel, "ParagraphFormat")?.as_dispatch()?; // ParagraphFormat
    let keys: [(&str, &str, f64); 7] = [ // 厘米字段
        ("leftIndent", "LeftIndent", CM_TO_POINTS), ("rightIndent", "RightIndent", CM_TO_POINTS), ("firstLineIndent", "FirstLineIndent", CM_TO_POINTS),
        ("spaceBefore", "SpaceBefore", 1.0), ("spaceAfter", "SpaceAfter", 1.0), ("lineSpacing", "LineSpacing", 1.0), ("", "", 1.0),
    ]; // 映射
    for (key, prop, scale) in keys.iter() { // 逐项
        if key.is_empty() { continue; } // 跳过占位
        if let Some(v) = arg(args, key).and_then(|v| v.as_f64()) { if set(&pf, prop, Variant::R8(v * scale)).is_err() { report.warn(Warning::new("PARTIAL_APPLY", format!("{prop} 应用失败"))); } } // 设置
    }
    if let Some(v) = arg(args, "alignment").and_then(|v| v.as_str()) { // 对齐
        let value = match v { "center" => WD_ALIGN_PARAGRAPH_CENTER, "right" => WD_ALIGN_PARAGRAPH_RIGHT, "justify" => WD_ALIGN_PARAGRAPH_JUSTIFY, _ => WD_ALIGN_PARAGRAPH_LEFT }; // 映射
        set(&pf, "Alignment", Variant::I4(value))?; // 关键失败即失败
    }
    if let Some(v) = arg(args, "lineSpacingRule").and_then(|v| v.as_str()) { // 行距规则
        let value = match v { "one_point_five" => WD_LINE_SPACE_ONE_POINT_FIVE, "double" => WD_LINE_SPACE_DOUBLE, "at_least" => WD_LINE_SPACE_AT_LEAST, "exactly" => WD_LINE_SPACE_EXACTLY, "multiple" => WD_LINE_SPACE_MULTIPLE, _ => WD_LINE_SPACE_SINGLE }; // 映射
        set(&pf, "LineSpacingRule", Variant::I4(value))?; // 设置
    }
    report.partial = !report.warnings.is_empty(); // 部分成功
    Ok(json!({"applied": true})) // 返回
}
fn style(ctx: &crate::com::document::DocCtx, args: &Map<String, Value>) -> ComResult<Value> { // 样式
    let sel = selection(ctx)?; // Selection
    let name = str_arg(args, "styleName"); // 样式名
    let candidates = if name.starts_with("Heading ") { vec![name.clone(), name.replacen("Heading", "标题", 1)] } else { vec![name.clone()] }; // 中英文回退
    let mut applied = false; // 标记
    for candidate in &candidates { if set(&sel, "Style", Variant::Str(candidate.clone())).is_ok() { applied = true; break; } } // Selection.Style
    if !applied { let format = get(&sel, "ParagraphFormat")?.as_dispatch()?; for candidate in &candidates { if set(&format, "Style", Variant::Str(candidate.clone())).is_ok() { applied = true; break; } } } // ParagraphFormat.Style
    if !applied { return Err(ToolError::new("FORMAT_APPLY_FAILED", format!("样式 {name} 应用失败"))); } // 失败
    Ok(json!({"applied": true})) // 返回
}
fn page_setup(ctx: &crate::com::document::DocCtx, args: &Map<String, Value>) -> ComResult<Value> { // 页面设置
    let setup = get(&ctx.doc, "PageSetup")?.as_dispatch()?; // PageSetup
    let keys = [("topMargin", "TopMargin"), ("bottomMargin", "BottomMargin"), ("leftMargin", "LeftMargin"), ("rightMargin", "RightMargin"), ("pageWidth", "PageWidth"), ("pageHeight", "PageHeight")]; // 映射
    for (key, prop) in keys.iter() { // 逐项
        if let Some(v) = arg(args, key).and_then(|v| v.as_f64()) { set(&setup, prop, Variant::R8(v * CM_TO_POINTS))?; } // 设置
    }
    if let Some(orient) = arg(args, "orientation").and_then(|v| v.as_str()) { // 方向
        set(&setup, "Orientation", Variant::I4(if orient == "landscape" { 1 } else { 0 }))?; // 设置
    }
    Ok(json!({"applied": true})) // 返回
}
fn properties(ctx: &crate::com::document::DocCtx, args: &Map<String, Value>) -> ComResult<Value> { // 文档属性
    let keys = [("title", "Title"), ("author", "Author"), ("subject", "Subject"), ("keywords", "Keywords"), ("comments", "Comments"), ("category", "Category")]; // 映射
    for (key, prop) in keys.iter() { // 逐项
        if let Some(v) = arg(args, key).and_then(|v| v.as_str()) { // 值
            let item = crate::com::document::builtin_property(&ctx.doc, prop)?; // 按序号枚举并匹配英文 Name
            set(&item, "Value", Variant::Str(v.to_string()))?; // 写
        }
    }
    Ok(json!({"applied": true})) // 返回
}
fn body_indent(ctx: &crate::com::document::DocCtx, args: &Map<String, Value>) -> ComResult<Value> { // 正文缩进
    let indent = arg(args, "indent").and_then(|v| v.as_f64()).unwrap_or(0.74); // 缩进 cm
    if !(0.0..=10.0).contains(&indent) { return Err(ToolError::new("INVALID_ARGUMENT", format!("indent={indent} 超出 0~10cm"))); } // 范围
    let paras = get(&ctx.doc, "Paragraphs")?.as_dispatch()?; // 段落集
    let count = get(&paras, "Count")?.as_i32(); // 数量
    crate::log_info!("fmt", "body_indent paragraphs={}", count); // 诊断
    let mut changed = 0; // 计数
    let mut i = 1; // 遍历
    while i <= count { // 逐段
        let para = call(&paras, "Item", &[Variant::I4(i)]).and_then(|v| v.as_dispatch()); // 段落（Item 用 METHOD）
        if let Ok(para) = para { // 成功
            let style_value = get(&para, "Style"); // 样式（对象或字符串）
            let style_normal = match style_value { // 样式判定
                Ok(value) => match value.as_dispatch() { Ok(style) => is_normal_style(&ctx.doc, &style), Err(_) => is_normal_text(&value.as_string()) }, // 对象/字符串
                Err(_) => false, // 读取失败
            };
            let outline = get(&para, "OutlineLevel").map(|v| v.as_i32()).unwrap_or(10); // 标题 1-9，正文 10
            if i <= 6 { crate::log_info!("fmt", "para i={} outline={} style_normal={}", i, outline, style_normal); } // 诊断
            if outline >= 10 || style_normal { // 正文或正文样式
                let format = get(&para, "Range").and_then(|v| v.as_dispatch()).and_then(|range| get(&range, "ParagraphFormat")).and_then(|v| v.as_dispatch()); // Range.ParagraphFormat 才是 Word 支持成员
                if let Ok(pf) = format {
                    match set(&pf, "FirstLineIndent", Variant::R8(indent * CM_TO_POINTS)) { Ok(_) => changed += 1, Err(err) => crate::log_info!("fmt", "indent set err para {}: {}", i, err.message) } // 缩进
                } else { crate::log_info!("fmt", "ParagraphFormat missing para {}", i); } // 诊断
            }
        }
        i += 1; // 下一段
    }
    Ok(json!({"changed": changed})) // 返回
}
fn is_normal_style(doc: &ComObj, style: &ComObj) -> bool { // 判定正文样式（wdStyleNormal 优先，BuiltIn/Type 兜底）
    if let Ok(styles) = get(doc, "Styles").and_then(|v| v.as_dispatch()) { // 样式集
        if let Ok(normal) = call(&styles, "Item", &[Variant::I4(-1)]).and_then(|v| v.as_dispatch()) { // wdStyleNormal(-1)
            let target = get(style, "Name").map(|v| v.as_string()).unwrap_or_default(); // 目标英文名
            let normal_name = get(&normal, "Name").map(|v| v.as_string()).unwrap_or_default(); // 正文英文名
            if !target.is_empty() && target.eq_ignore_ascii_case(&normal_name) { return true; } // 命中
        }
    }
    let english = get(style, "Name").map(|v| v.as_string()).unwrap_or_default(); // 英文名
    let local = get(style, "NameLocal").map(|v| v.as_string()).unwrap_or_default(); // 本地名
    let built_in = get(style, "BuiltIn").map(|v| v.as_bool()).unwrap_or(false); // 是否内置
    let style_type = get(style, "Type").map(|v| v.as_i32()).unwrap_or(0); // wdStyleTypeParagraph = 1
    built_in && style_type == 1 && (english.eq_ignore_ascii_case("Normal") || local == "Normal" || local == "正文" || local == "普通") // 兜底判定
}
fn is_normal_text(name: &str) -> bool { // 字符串样式名判定正文
    let text = name.trim(); // 去空白
    text.eq_ignore_ascii_case("Normal") || text == "正文" || text == "普通" // 命中
}
fn track_changes(ctx: &crate::com::document::DocCtx, args: &Map<String, Value>) -> ComResult<Value> { // 修订开关
    let enable = arg(args, "enable").and_then(|v| v.as_bool()).unwrap_or(false); // 标志
    set(&ctx.doc, "TrackRevisions", Variant::Bool(enable))?; // 设置
    Ok(json!({"enabled": enable})) // 返回
}
fn track_apply(ctx: &crate::com::document::DocCtx, args: &Map<String, Value>) -> ComResult<Value> { // 接受拒绝修订
    let revisions = get(&ctx.doc, "Revisions")?.as_dispatch()?; // 修订集
    let method = if str_arg(args, "action") == "reject" { "RejectAll" } else { "AcceptAll" }; // 方法
    call(&revisions, method, &[])?; // 执行
    Ok(json!({"applied": method})) // 返回
}
fn parse_bgr(color: &str) -> Option<i32> { // #RRGGBB → BGR
    let hex = color.trim_start_matches('#'); // 去 #
    if hex.len() != 6 { return None; } // 长度
    let r = u8::from_str_radix(&hex[0..2], 16).ok()? as i32; // 红
    let g = u8::from_str_radix(&hex[2..4], 16).ok()? as i32; // 绿
    let b = u8::from_str_radix(&hex[4..6], 16).ok()? as i32; // 蓝
    Some(r + g * 256 + b * 65536) // BGR
}
