use serde_json::{json, Map, Value}; // JSON 构造
use crate::com::app::AppHandle; // 应用句柄
use crate::com::constants::*; // 常量
use crate::com::dispatch::{get_args, call, get, set, ComObj, ComResult, Variant}; // COM 工具
use crate::com::document::{active_document, create_document, open_document, paragraph_index_at, DocCtx}; // 文档上下文
use crate::word::backup::backup_before_save; // 备份
use crate::word::paths::Paths; // 路径
use crate::word::report::{err_json, ok_json, Report, ToolError, Warning}; // 信封
use crate::word::{instances, session}; // 会话与实例
fn arg<'a>(args: &'a Map<String, Value>, key: &str) -> Option<&'a Value> { args.get(key) } // 取参数
fn str_arg(args: &Map<String, Value>, key: &str) -> String { arg(args, key).and_then(|v| v.as_str()).unwrap_or("").to_string() } // 字符串参数
fn i32_arg(args: &Map<String, Value>, key: &str, default: i32) -> i32 { arg(args, key).and_then(|v| v.as_i64()).unwrap_or(default as i64) as i32 } // 整数参数
fn bool_arg(args: &Map<String, Value>, key: &str, default: bool) -> bool { arg(args, key).and_then(|v| v.as_bool()).unwrap_or(default) } // 布尔参数
pub fn run(args: &Map<String, Value>) -> Value { // word_document 10 action
    let action = str_arg(args, "action"); // 动作
    match action.as_str() { // 分派
        "status" => status(args), // 状态
        "create" => create(args), // 新建
        "open" => open(args), // 打开
        "save" => save(args), // 保存
        "save_as" => save_as(args), // 另存
        "export_pdf" => export_pdf(args), // 导出 PDF
        "close" => close(args), // 关闭
        "quit" => quit(args), // 退出
        "info" => info(args), // 信息
        "structure" => structure(args), // 结构
        other => err_json(&ToolError::new("INVALID_ARGUMENT", format!("未知 action：{other}"))), // 未知
    }
}
fn status(args: &Map<String, Value>) -> Value { // 状态查询（不创建 Word）
    let path = str_arg(args, "path"); // 可选路径
    let docs = instances::all_documents().unwrap_or_default(); // ROT 文档
    let controlled = crate::com::app::controlled_hwnds(); // 受控实例窗口
    let alive = !docs.is_empty() || !controlled.is_empty(); // ROT 文档 + 受控实例为准
    let process_detected = word_running(); // tasklist 仅作补充提示
    let running = alive; // wordRunning 以 ROT/受控实例为准
    let mut groups: Vec<(i32, Vec<Value>)> = Vec::new(); // 实例分组
    let mut active_json = Value::Null; // 活动文档
    for hit in &docs { // 逐个文档
        let hwnd = instances::application_hwnd(&hit.app); // 所属实例
        let name = file_name(&hit.path); // 文件名
        let is_active = instances::same_path(&active_path(&hit.app), &hit.path); // 是否活动
        if is_active { active_json = json!({"path": hit.path, "name": name, "saved": hit.saved}); } // 记录活动
        let item = json!({"path": hit.path, "name": name, "saved": hit.saved, "active": is_active}); // 文档项
        match groups.iter_mut().find(|(value, _)| *value == hwnd) { Some((_, list)) => list.push(item), None => groups.push((hwnd, vec![item])) } // 分组
    }
    let instances_json: Vec<Value> = groups.iter().map(|(hwnd, list)| json!({"pid": window_pid(*hwnd), "hwnd": hwnd, "visible": window_visible(*hwnd), "foreground": window_foreground(*hwnd), "docs": list})).collect(); // 实例列表
    let dialog = groups.iter().any(|(hwnd, _)| crate::com::document::instance_has_dialog(*hwnd)); // 任一实例存在对话框
    let state = if dialog { "DIALOG" } else if !path.is_empty() { // 指定文件
        if docs.iter().any(|hit| instances::same_path(&hit.path, &path)) { "DOC_ACTIVE" } else if running { "NO_DOC" } else { "NO_WORD" } // 命中判断
    } else if !docs.is_empty() { "DOC_ACTIVE" } else if running { "NO_DOC" } else { "NO_WORD" }; // 全局
    let mut data = json!({"state": state, "wordRunning": running, "processDetected": process_detected, "instances": instances_json}); // 返回体
    if !active_json.is_null() { data["active"] = active_json; } // 附活动文档
    ok_json(data) // 返回
}
fn word_running() -> bool { // 检测 WINWORD 进程（有 ROT 文档或进程名）
    use std::process::Command; // 进程
    let out = Command::new("tasklist").args(["/FI", "IMAGENAME eq WINWORD.EXE"]).output(); // 查询
    out.map(|o| String::from_utf8_lossy(&o.stdout).to_lowercase().contains("winword.exe")).unwrap_or(false) // 判定
}
fn create(args: &Map<String, Value>) -> Value { // 新建文档
    let path = str_arg(args, "path"); // 路径
    if path.is_empty() { return err_json(&ToolError::new("INVALID_ARGUMENT", "create 必须传 path")); } // 校验
    let paths = Paths::load(); // 策略
    let template = arg(args, "templatePath").and_then(|v| v.as_str()); // 模板
    match create_document(&path, &paths, template) { // 创建
        Ok(ctx) => { // 应用可选设置
            if let Err(err) = apply_create_options(&ctx, args) { let _ = ctx.close(false); return err_json(&err); } // 页面/属性/样式
            if let Err(err) = call(&ctx.doc, "Save", &[]) { let _ = ctx.close(false); return err_json(&err); } // 选项落盘（Saved=true）
            let hwnd = ctx.app.active_window_hwnd().unwrap_or(0); // 实例窗口
            let pid = window_pid(hwnd); // 进程号
            let title = get(&ctx.doc, "ActiveWindow").and_then(|v| v.as_dispatch()).and_then(|window| get(&window, "Caption")).map(|v| v.as_string()).unwrap_or_default(); // 窗口标题
            ok_json(json!({"path": ctx.path.to_string_lossy(), "pid": pid, "windowTitle": title, "created": true})) // 返回
        }
        Err(err) => err_json(&err), // 失败
    }
}
fn apply_create_options(ctx: &DocCtx, args: &Map<String, Value>) -> crate::com::dispatch::ComResult<()> { // 应用 title/author/margins/orientation/baseStyleProfile
    for (key, name) in [("title", "Title"), ("author", "Author")] { // 标题/作者
        if let Some(value) = arg(args, key).and_then(|v| v.as_str()) { // 有值
            let prop = crate::com::document::builtin_property(&ctx.doc, name)?; // 按序号枚举并匹配英文 Name
            set(&prop, "Value", Variant::Str(value.to_string()))?; // 写入（失败即报错）
        }
    }
    let page = get(&ctx.doc, "PageSetup")?.as_dispatch()?; // 页面设置
    let margins = [("topMargin", "TopMargin"), ("bottomMargin", "BottomMargin"), ("leftMargin", "LeftMargin"), ("rightMargin", "RightMargin")]; // 边距映射
    for (key, prop) in margins { // 逐项
        if let Some(cm) = arg(args, key).and_then(|v| v.as_f64()) { set(&page, prop, Variant::R8(cm * CM_TO_POINTS))?; } // 厘米转磅
    }
    if let Some(orient) = arg(args, "orientation").and_then(|v| v.as_str()) { // 方向
        set(&page, "Orientation", Variant::I4(if orient == "landscape" { 1 } else { 0 }))?; // 设置
    }
    if let Some(profile) = arg(args, "baseStyleProfile") { // 样式档案
        apply_style_profile_value(&ctx.doc, profile)?; // 对象或逗号字符串
    }
    Ok(()) // 完成
}
fn apply_style_profile(doc: &ComObj, profile: &str) -> crate::com::dispatch::ComResult<()> { // 应用 Normal 样式档案（字体,字号,行距）
    let styles = get(doc, "Styles")?.as_dispatch()?; // 样式集
    let count = get(&styles, "Count")?.as_i32(); // 数量
    let mut target = None; // 正文样式
    for index in 1..=count { // 遍历（避免 Item(字符串) 异常）
        if let Ok(style) = call(&styles, "Item", &[Variant::I4(index)]).and_then(|v| v.as_dispatch()) { // 样式
            let name = get(&style, "Name").map(|v| v.as_string()).unwrap_or_default(); // 英文名
            let local = get(&style, "NameLocal").map(|v| v.as_string()).unwrap_or_default(); // 本地名
            if name.eq_ignore_ascii_case("Normal") || local == "Normal" || local == "正文" || local == "普通" { target = Some(style); break; } // 命中
        }
    }
    let style = match target { Some(style) => style, None => call(&styles, "Item", &[Variant::I4(1)])?.as_dispatch()? }; // 兜底首个样式
    let parts: Vec<String> = profile.split(',').map(|value| value.trim().to_string()).collect(); // 字段
    let font = get(&style, "Font")?.as_dispatch()?; // 字体
    if let Some(name) = parts.get(0).filter(|value| !value.is_empty()) { set(&font, "Name", Variant::Str(name.clone()))?; } // 字体名
    if let Some(size) = parts.get(1).filter(|value| !value.is_empty()) { let value: f64 = size.parse().map_err(|_| ToolError::new("INVALID_ARGUMENT", "baseStyleProfile 字号无效"))?; set(&font, "Size", Variant::R8(value))?; } // 字号
    if let Some(rule) = parts.get(2).filter(|value| !value.is_empty()) { // 行距
        let value = match rule.as_str() { "1.5" | "one_point_five" => WD_LINE_SPACE_ONE_POINT_FIVE, "double" => WD_LINE_SPACE_DOUBLE, _ => WD_LINE_SPACE_SINGLE }; // 映射
        let format = get(&style, "ParagraphFormat")?.as_dispatch()?; // 段落格式
        set(&format, "LineSpacingRule", Variant::I4(value))?; // 设置
    }
    Ok(()) // 完成
}
fn color_value(value: &Value) -> Option<i32> { // 颜色：hex/名称/整数 → WdColor(BGR)
    if let Some(text) = value.as_str() { // 字符串
        let text = text.trim(); // 去空白
        if let Some(hex) = text.strip_prefix('#') { // #RRGGBB
            if hex.len() == 6 { // 六位
                let r = i32::from_str_radix(&hex[0..2], 16).ok()?; // 红
                let g = i32::from_str_radix(&hex[2..4], 16).ok()?; // 绿
                let b = i32::from_str_radix(&hex[4..6], 16).ok()?; // 蓝
                return Some(r + g * 256 + b * 65536); // BGR
            }
        }
        return match text.to_lowercase().as_str() { // 名称
            "black" => Some(0), "red" => Some(255), "green" => Some(65280), // 常用色
            "blue" => Some(16711680), "yellow" => Some(65535), "white" => Some(16777215), // 常用色
            _ => None, // 未知
        };
    }
    value.as_i64().map(|v| v as i32) // 直接整数
}
fn find_style(doc: &ComObj, name: &str) -> ComResult<ComObj> { // 按英文/本地名/内置 id 查找样式
    let styles = get(doc, "Styles")?.as_dispatch()?; // 样式集
    let lower = name.trim().to_lowercase(); // 归一化
    let builtin = if matches!(lower.as_str(), "normal" | "正文" | "普通") { Some(-1) } else { // Normal = wdStyleNormal
        let level = lower.trim_start_matches("heading").trim_start_matches("标题").trim().parse::<i32>().ok(); // 标题级别
        level.filter(|v| (1..=9).contains(v)).map(|v| -1 - v) // Heading 1..9 = -2..-10
    };
    if let Some(id) = builtin { // 内置样式优先
        if let Ok(style) = call(&styles, "Item", &[Variant::I4(id)]).and_then(|v| v.as_dispatch()) { return Ok(style); } // 命中
    }
    let count = get(&styles, "Count")?.as_i32(); // 数量
    for index in 1..=count { // 逐项
        if let Ok(style) = call(&styles, "Item", &[Variant::I4(index)]).and_then(|v| v.as_dispatch()) { // 样式
            let en = get(&style, "Name").map(|v| v.as_string()).unwrap_or_default(); // 英文名
            let local = get(&style, "NameLocal").map(|v| v.as_string()).unwrap_or_default(); // 本地名
            if en.eq_ignore_ascii_case(name) || local.eq_ignore_ascii_case(name) { return Ok(style); } // 命中
        }
    }
    call(&styles, "Item", &[Variant::Str(name.to_string())]).and_then(|v| v.as_dispatch()).map_err(|_| ToolError::new("WORD_ERROR", format!("样式不存在：{name}"))) // 字符串回退
}
fn apply_style_profile_value(doc: &ComObj, profile: &Value) -> ComResult<()> { // 样式档案：字符串或对象
    if let Some(text) = profile.as_str() { // 兼容逗号字符串
        if !text.trim().is_empty() { apply_style_profile(doc, text)?; } // 应用
        return Ok(()); // 完成
    }
    let Some(map) = profile.as_object() else { return Err(ToolError::new("INVALID_ARGUMENT", "baseStyleProfile 必须是对象或字符串")); }; // 类型
    for (name, spec) in map { // 逐样式
        let style = find_style(doc, name)?; // 样式对象
        if let Some(font) = spec.get("font").and_then(|v| v.as_object()) { // 字体
            let target = get(&style, "Font")?.as_dispatch()?; // Font
            if let Some(v) = font.get("name").filter(|v| !v.is_null()) { set(&target, "Name", Variant::Str(v.as_str().unwrap_or_default().to_string()))?; } // 字体名
            if let Some(v) = font.get("size").and_then(|v| v.as_f64()) { set(&target, "Size", Variant::R8(v))?; } // 字号
            if let Some(v) = font.get("bold").and_then(|v| v.as_bool()) { set(&target, "Bold", Variant::Bool(v))?; } // 粗体
            if let Some(v) = font.get("italic").and_then(|v| v.as_bool()) { set(&target, "Italic", Variant::Bool(v))?; } // 斜体
            if let Some(v) = font.get("strikethrough").and_then(|v| v.as_bool()) { set(&target, "StrikeThrough", Variant::Bool(v))?; } // 删除线
            if let Some(v) = font.get("superscript").and_then(|v| v.as_bool()) { set(&target, "Superscript", Variant::Bool(v))?; } // 上标
            if let Some(v) = font.get("subscript").and_then(|v| v.as_bool()) { set(&target, "Subscript", Variant::Bool(v))?; } // 下标
            if let Some(v) = font.get("color").and_then(color_value) { set(&target, "Color", Variant::I4(v))?; } // 颜色
            if let Some(v) = font.get("underline").and_then(|v| v.as_str()) { // 下划线
                let value = match v { "single" => WD_UNDERLINE_SINGLE, "double" => WD_UNDERLINE_DOUBLE, "wavy" => WD_UNDERLINE_WAVY, _ => WD_UNDERLINE_NONE }; // 映射
                set(&target, "Underline", Variant::I4(value))?; // 写入
            }
        }
        if let Some(paragraph) = spec.get("paragraph").and_then(|v| v.as_object()) { // 段落
            let target = get(&style, "ParagraphFormat")?.as_dispatch()?; // ParagraphFormat
            if let Some(v) = paragraph.get("alignment").and_then(|v| v.as_str()) { // 对齐
                let value = match v { "center" => WD_ALIGN_PARAGRAPH_CENTER, "right" => WD_ALIGN_PARAGRAPH_RIGHT, "justify" => WD_ALIGN_PARAGRAPH_JUSTIFY, _ => WD_ALIGN_PARAGRAPH_LEFT }; // 映射
                set(&target, "Alignment", Variant::I4(value))?; // 写入
            }
            if let Some(v) = paragraph.get("firstLineIndent").and_then(|v| v.as_f64()) { set(&target, "FirstLineIndent", Variant::R8(v * CM_TO_POINTS))?; } // 首行缩进 cm
            if let Some(v) = paragraph.get("spaceBefore").and_then(|v| v.as_f64()) { set(&target, "SpaceBefore", Variant::R8(v))?; } // 段前 pt
            if let Some(v) = paragraph.get("spaceAfter").and_then(|v| v.as_f64()) { set(&target, "SpaceAfter", Variant::R8(v))?; } // 段后 pt
            if let Some(v) = paragraph.get("lineSpacing").and_then(|v| v.as_f64()) { set(&target, "LineSpacing", Variant::R8(v))?; } // 行距
            if let Some(v) = paragraph.get("lineSpacingRule").and_then(|v| v.as_str()) { // 行距规则
                let value = match v { "one_point_five" => WD_LINE_SPACE_ONE_POINT_FIVE, "double" => WD_LINE_SPACE_DOUBLE, "at_least" => WD_LINE_SPACE_AT_LEAST, "exactly" => WD_LINE_SPACE_EXACTLY, "multiple" => WD_LINE_SPACE_MULTIPLE, _ => WD_LINE_SPACE_SINGLE }; // 映射
                set(&target, "LineSpacingRule", Variant::I4(value))?; // 写入
            }
            if let Some(v) = paragraph.get("keepWithNext").and_then(|v| v.as_bool()) { set(&target, "KeepWithNext", Variant::Bool(v))?; } // 与下段同页
            if let Some(v) = paragraph.get("pageBreakBefore").and_then(|v| v.as_bool()) { set(&target, "PageBreakBefore", Variant::Bool(v))?; } // 段前分页
        }
    }
    Ok(()) // 完成
}
fn open(args: &Map<String, Value>) -> Value { // 打开文档
    let path = str_arg(args, "path"); // 路径
    if path.is_empty() { return err_json(&ToolError::new("INVALID_ARGUMENT", "open 必须传 path")); } // 校验
    let paths = Paths::load(); // 策略
    match open_document(&path, &paths, bool_arg(args, "forceNewWindow", false)) { // 打开
        Ok((ctx, already)) => ok_json(json!({"path": ctx.path.to_string_lossy(), "pid": window_pid(instances::application_hwnd(&ctx.app.com)), "wasAlreadyOpen": already})), // 返回
        Err(err) => err_json(&err), // 失败
    }
}
fn save(args: &Map<String, Value>) -> Value { // 保存
    let path = str_arg(args, "path"); // 路径
    let paths = Paths::load(); // 策略
    match active_document(&path, &paths) { // 取文档
        Ok(ctx) => { // 备份
            if let Err(err) = crate::com::document::ensure_writable(&ctx.app) { return err_json(&err); } // 对话框拦截
            let backup = match backup_before_save(&ctx.path) { Ok(value) => value, Err(error) => return err_json(&error) }; // 备份失败中止
            match ctx.save(backup.as_ref()) { // 保存
                Ok(bytes) => ok_json(json!({"path": ctx.path.to_string_lossy(), "backupPath": backup.map(|p| p.to_string_lossy().to_string()), "bytes": bytes, "savedAt": chrono::Local::now().to_rfc3339()})), // 成功
                Err(err) => err_json(&err), // 失败
            }
        }
        Err(err) => err_json(&err), // 失败
    }
}
fn save_as(args: &Map<String, Value>) -> Value { // 另存为
    let path = str_arg(args, "path"); // 源
    let target = str_arg(args, "targetPath"); // 目标
    if target.is_empty() { return err_json(&ToolError::new("INVALID_ARGUMENT", "save_as 必须传 targetPath")); } // 校验
    let paths = Paths::load(); // 策略
    let full = match paths.resolve(&target) { Ok(p) => p, Err(e) => return err_json(&e) }; // 校验目标
    let format = format_of(&str_arg(args, "format"), &full); // 格式
    match active_document(&path, &paths) { // 取文档
        Ok(ctx) => { // 备份目标
            if let Err(err) = crate::com::document::ensure_writable(&ctx.app) { return err_json(&err); } // 对话框拦截
            let backup = match backup_before_save(&full) { Ok(value) => value, Err(error) => return err_json(&error) }; // 备份失败中止
            match call(&ctx.doc, "SaveAs2", &[Variant::Str(full.to_string_lossy().to_string()), Variant::I4(format)]) { // 另存
                Ok(_) => { call(&ctx.doc, "Activate", &[]).ok(); crate::common::registry::forget(&path); crate::common::registry::put(&full.to_string_lossy(), WORD_PROGID, &ctx.app.com, &ctx.doc); ok_json(json!({"path": full.to_string_lossy(), "format": str_arg(args, "format"), "backupPath": backup.map(|p| p.to_string_lossy().to_string())})) } // 成功（同步路径登记）
                Err(err) => err_json(&err), // 失败
            }
        }
        Err(err) => err_json(&err), // 失败
    }
}
fn format_of(format: &str, path: &std::path::Path) -> i32 { // 扩展名→格式
    let ext = format.to_lowercase(); // 格式字符串
    let by_ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase(); // 扩展名
    let key = if ext.is_empty() { by_ext } else { ext }; // 优先显式格式
    match key.as_str() { // 映射
        "doc" => WD_FORMAT_DOCUMENT, // doc
        "pdf" => WD_FORMAT_PDF, // pdf
        "rtf" => WD_FORMAT_RTF, // rtf
        "txt" => WD_FORMAT_TEXT, // txt
        "html" | "htm" => WD_FORMAT_HTML, // html
        "mht" => WD_FORMAT_WEB_ARCHIVE, // mht
        "xml" => WD_FORMAT_XML, // xml
        "odt" => WD_FORMAT_OPEN_DOCUMENT_TEXT, // odt
        "docm" => WD_FORMAT_XML_DOCUMENT_MACRO_ENABLED, // docm
        "dotx" => WD_FORMAT_XML_TEMPLATE, // dotx
        "dotm" => WD_FORMAT_XML_TEMPLATE_MACRO_ENABLED, // dotm
        _ => WD_FORMAT_DOCUMENT_DEFAULT, // docx
    }
}
fn export_pdf(args: &Map<String, Value>) -> Value { // 导出 PDF
    let path = str_arg(args, "path"); // 源
    let paths = Paths::load(); // 策略
    let full = if str_arg(args, "targetPath").is_empty() { // 默认同目录同名
        match paths.resolve(&path) { Ok(p) => p.with_extension("pdf"), Err(e) => return err_json(&e) } // 目标
    } else { // 显式
        match paths.resolve(&str_arg(args, "targetPath")) { Ok(p) => p, Err(e) => return err_json(&e) } // 目标
    };
    match active_document(&path, &paths) { // 取文档
        Ok(ctx) => { // 导出
            if let Err(err) = crate::com::document::ensure_writable(&ctx.app) { return err_json(&err); } // 对话框拦截
            let args = [Variant::Str(full.to_string_lossy().to_string()), Variant::I4(17), Variant::Bool(false)]; // ExportAsFixedFormat 简参
            match call(&ctx.doc, "ExportAsFixedFormat", &args) { Ok(_) => ok_json(json!({"pdfPath": full.to_string_lossy()})), Err(err) => err_json(&err) } // 结果
        }
        Err(err) => err_json(&err), // 失败
    }
}
fn close(args: &Map<String, Value>) -> Value { // 关闭文档
    let path = str_arg(args, "path"); // 路径
    let save_changes = bool_arg(args, "saveChanges", false); // 保存标志
    let paths = Paths::load(); // 策略
    match active_document(&path, &paths) { // 取文档
        Ok(ctx) => { // 检查未保存
            if let Err(err) = crate::com::document::ensure_writable(&ctx.app) { return err_json(&err); } // 对话框拦截
            let saved = ctx.saved(); // 状态
            match ctx.close(save_changes) { // 关闭
                Ok(()) => { // 成功
                    let mut report = Report::default(); // 报告
                    if !saved && !save_changes { report.warn(Warning::new("UNSAVED_CHANGES_DISCARDED", "未保存修改已丢弃")); } // 告警
                    crate::common::registry::forget(&path); // 清理路径登记
                    report.ok_json(json!({"closed": true})) // 返回（保留警告）
                }
                Err(err) => err_json(&err), // 失败
            }
        }
        Err(err) => err_json(&err), // 失败
    }
}
fn quit(args: &Map<String, Value>) -> Value { // 退出受控实例
    let _force = bool_arg(args, "force", false); // 强制（受控实例统一不保存）
    let apps = crate::com::app::take_controlled_apps(); // 受控 Application
    let mut quit_count = 0; // 退出实例数
    let mut closed: Vec<String> = Vec::new(); // 关闭文档
    for app in &apps { // 逐个受控实例
        if let Ok(docs) = get(app, "Documents").and_then(|v| v.as_dispatch()) { // 文档集合
            let count = get(&docs, "Count").map(|v| v.as_i32()).unwrap_or(0); // 数量
            for index in 1..=count { // 逐文档
                if let Ok(doc) = get_args(&docs, "Item", &[Variant::I4(index)]).and_then(|v| v.as_dispatch()) { // 文档
                    if let Ok(full) = get(&doc, "FullName").map(|v| v.as_string()) { if !full.is_empty() { closed.push(full); } } // 记录路径
                }
            }
        }
        if call(app, "Quit", &[Variant::I4(WD_DO_NOT_SAVE_CHANGES)]).is_ok() { quit_count += 1; } // 退出实例
    }
    crate::com::app::clear_controlled(); // 清空窗口登记
    crate::common::registry::forget_label(WORD_PROGID); // 清理 Word 路径登记
    ok_json(json!({"quitCount": quit_count, "closedDocs": closed})) // 返回
}
fn info(args: &Map<String, Value>) -> Value { // 文档信息
    let path = str_arg(args, "path"); // 路径
    let paths = Paths::load(); // 策略
    match active_document(&path, &paths) { // 取文档
        Ok(ctx) => { // 统计
            let words = call(&ctx.doc, "ComputeStatistics", &[Variant::I4(WD_STATISTIC_WORDS)]).map(|v| v.as_i32()).unwrap_or(0); // 字数
            let pages = call(&ctx.doc, "ComputeStatistics", &[Variant::I4(WD_STATISTIC_PAGES)]).map(|v| v.as_i32()).unwrap_or(0); // 页数
            let chars = call(&ctx.doc, "ComputeStatistics", &[Variant::I4(WD_STATISTIC_CHARACTERS)]).map(|v| v.as_i32()).unwrap_or(0); // 字符
            let paragraphs = get(&ctx.doc, "Paragraphs").and_then(|v| v.as_dispatch()).and_then(|p| get(&p, "Count")).map(|v| v.as_i32()).unwrap_or(0); // 段落
            let sections = get(&ctx.doc, "Sections").and_then(|v| v.as_dispatch()).and_then(|s| get(&s, "Count")).map(|v| v.as_i32()).unwrap_or(0); // 节
            ok_json(json!({"path": ctx.path.to_string_lossy(), "wordCount": words, "pageCount": pages, "paragraphCount": paragraphs, "characterCount": chars, "sectionCount": sections, "saved": ctx.saved()})) // 返回
        }
        Err(err) => err_json(&err), // 失败
    }
}
fn structure(args: &Map<String, Value>) -> Value { // 文档结构
    let path = str_arg(args, "path"); // 路径
    let max_level = i32_arg(args, "maxLevel", 9); // 最大级别
    let paths = Paths::load(); // 策略
    match active_document(&path, &paths) { // 取文档
        Ok(ctx) => { // 标题
            let mut headings = Vec::new(); // 标题集
            let paras = get(&ctx.doc, "Paragraphs").and_then(|v| v.as_dispatch()); // 段落集
            if let Ok(paras) = paras { // 成功
                let count = get(&paras, "Count").map(|v| v.as_i32()).unwrap_or(0); // 数量
                let mut idx = 1; // 遍历
                while idx <= count { // 逐段
                    let para = call(&paras, "Item", &[Variant::I4(idx)]).and_then(|v| v.as_dispatch()); // 段落（带参 Item 用 METHOD）
                    if let Ok(para) = para { // 成功
                        let outline = get(&para, "OutlineLevel").map(|v| v.as_i32()).unwrap_or(10); // 级别
                        if outline >= 1 && outline <= max_level { // 命中
                            let text = get(&para, "Range").and_then(|v| v.as_dispatch()).and_then(|r| get(&r, "Text")).map(|v| v.as_string()).unwrap_or_default(); // 文本
                            headings.push(json!({"level": outline, "text": text.trim(), "paragraphIndex": idx})); // 收集
                        }
                    }
                    idx += 1; // 下一段
                }
            }
            let mut tables = Vec::new(); // 表格集
            if let Ok(tables_obj) = get(&ctx.doc, "Tables").and_then(|v| v.as_dispatch()) { // 集合
                let count = get(&tables_obj, "Count").map(|v| v.as_i32()).unwrap_or(0); // 数量
                let mut index = 1; // 遍历
                while index <= count { // 逐表
                    if let Ok(table) = call(&tables_obj, "Item", &[Variant::I4(index)]).and_then(|v| v.as_dispatch()) { // 表格（带参 Item 用 METHOD）
                        let rows = get(&table, "Rows").and_then(|v| v.as_dispatch()).and_then(|r| get(&r, "Count")).map(|v| v.as_i32()).unwrap_or(0); // 行
                        let cols = get(&table, "Columns").and_then(|v| v.as_dispatch()).and_then(|c| get(&c, "Count")).map(|v| v.as_i32()).unwrap_or(0); // 列
                        let start = get(&table, "Range").and_then(|v| v.as_dispatch()).and_then(|r| get(&r, "Start")).map(|v| v.as_i32()).unwrap_or(0); // 起点
                        tables.push(json!({"index": index, "rows": rows, "cols": cols, "paragraphIndex": paragraph_index_at(&ctx.doc, start), "start": start})); // 收集
                    }
                    index += 1; // 下一表
                }
            }
            ok_json(json!({"headings": headings, "tables": tables})) // 返回
        }
        Err(err) => err_json(&err), // 失败
    }
}
pub fn where_am_i(args: &Map<String, Value>) -> Value { // word_where_am_i
    let path = str_arg(args, "path"); // 路径
    if path.is_empty() { return err_json(&ToolError::new("INVALID_ARGUMENT", "where_am_i 必须传 path")); } // 校验
    let paths = Paths::load(); // 策略
    match active_document(&path, &paths) { // 取文档
        Ok(ctx) => { // 光标
            let app = get(&ctx.doc, "Application").and_then(|v| v.as_dispatch()); // Application
            let mut start = 0; // 起点
            let mut end = 0; // 终点
            let mut page = 1; // 页码
            let mut in_table = false; // 是否在表格
            let mut in_header = false; // 是否在页眉页脚
            let mut preview = String::new(); // 选区预览
            if let Ok(app) = app { // 成功
                if let Ok(sel) = get(&app, "Selection").and_then(|v| v.as_dispatch()) { // Selection
                    start = get(&sel, "Start").map(|v| v.as_i32()).unwrap_or(0); // 起点
                    end = get(&sel, "End").map(|v| v.as_i32()).unwrap_or(0); // 终点
                    page = call(&sel, "Information", &[Variant::I4(WD_ACTIVE_END_PAGE_NUMBER)]).map(|v| v.as_i32()).unwrap_or(1); // 页码
                    in_table = get(&sel, "Tables").and_then(|v| v.as_dispatch()).and_then(|t| get(&t, "Count")).map(|v| v.as_i32() > 0).unwrap_or(false); // 表格内
                    in_header = get(&sel, "StoryType").map(|v| v.as_i32() != 1).unwrap_or(false); // 非主正文
                    preview = get(&sel, "Text").map(|v| v.as_string().chars().take(80).collect::<String>()).unwrap_or_default(); // 预览
                }
            }
            let docs = instances::all_documents().unwrap_or_default(); // ROT 文档
            let instances_json: Vec<Value> = docs.iter().map(|hit| json!({"path": hit.path, "saved": hit.saved})).collect(); // 实例摘要
            let paragraph_index = paragraph_index_at(&ctx.doc, start); // 光标段落（1 起）
            ok_json(json!({"state": "DOC_ACTIVE", "doc": {"path": ctx.path.to_string_lossy(), "name": file_name(&ctx.path.to_string_lossy()), "saved": ctx.saved()}, "cursor": {"start": start, "end": end, "paragraphIndex": paragraph_index, "inTable": in_table, "inHeaderFooter": in_header, "page": page}, "selection": {"start": start, "end": end, "textPreview": preview}, "wordRunning": true, "instances": instances_json})) // 返回
        }
        Err(err) => err_json(&err), // 失败
    }
}
fn active_path(app: &ComObj) -> String { // 活动文档完整路径
    get(app, "ActiveDocument").ok().and_then(|v| v.as_dispatch().ok()).and_then(|doc| get(&doc, "FullName").ok()).map(|v| v.as_string()).unwrap_or_default() // 读 FullName
}
pub fn file_name(path: &str) -> String { // 文件名
    std::path::Path::new(path).file_name().map(|name| name.to_string_lossy().to_string()).unwrap_or_else(|| path.to_string()) // 提取
}
fn window_pid(hwnd: i32) -> u32 { // 窗口进程号
    use windows::Win32::Foundation::HWND; // 句柄类型
    use windows::Win32::UI::WindowsAndMessaging::GetWindowThreadProcessId; // API
    if hwnd == 0 { return 0; } // 无窗口
    let mut pid = 0u32; // 接收
    unsafe { GetWindowThreadProcessId(HWND(hwnd as isize as *mut std::ffi::c_void), Some(&mut pid)); } // 查询
    pid // 返回
}
fn window_visible(hwnd: i32) -> bool { // 窗口可见性
    use windows::Win32::Foundation::HWND; // 句柄类型
    use windows::Win32::UI::WindowsAndMessaging::IsWindowVisible; // API
    hwnd != 0 && unsafe { IsWindowVisible(HWND(hwnd as isize as *mut std::ffi::c_void)) }.as_bool() // 查询
}
fn window_foreground(hwnd: i32) -> bool { // 是否前台
    use windows::Win32::Foundation::HWND; // 句柄类型
    use windows::Win32::UI::WindowsAndMessaging::GetForegroundWindow; // API
    hwnd != 0 && unsafe { GetForegroundWindow() } == HWND(hwnd as isize as *mut std::ffi::c_void) // 比较
}
