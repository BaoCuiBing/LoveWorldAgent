use serde_json::{json, Map, Value}; // JSON 构造
use crate::com::constants::*; // 常量
use crate::com::dispatch::{attach_app, call, get, ComObj, ComResult, Variant}; // COM 工具
use crate::common::paths::Paths; // 路径策略
use crate::common::progid::{installed, take_controlled}; // 安装探测与受控实例
use crate::ppt::locator::{self, presentations, str_arg, PresCtx}; // PPT 上下文
use crate::word::report::{err_json, ok_json, ToolError}; // 信封

pub fn run(args: &Map<String, Value>) -> Value { // ppt_presentation 9 action
    let action = str_arg(args, "action"); // 动作
    let result: ComResult<Value> = match action.as_str() { // 分派
        "status" => status(), // 状态
        "create" => create(args), // 新建
        "open" => open(args), // 打开
        "save" => save(args), // 保存
        "save_as" => save_as(args), // 另存
        "export_pdf" => export_pdf(args), // 导出 PDF
        "close" => close(args), // 关闭
        "quit" => quit(), // 退出受控实例
        "info" => info(args), // 信息
        other => Err(ToolError::new("INVALID_ARGUMENT", format!("未知 ppt_presentation action：{other}"))), // 未知
    };
    match result { Ok(value) => ok_json(value), Err(err) => err_json(&err) } // 返回
}
fn status() -> ComResult<Value> { // 状态查询（不创建实例）
    let office = installed(POWERPOINT_PROGID); // 是否安装
    let mut running = false; // 是否运行
    let mut count = 0; // 演示文稿数
    let mut names: Vec<String> = Vec::new(); // 名称列表
    if let Ok(app) = attach_app(POWERPOINT_PROGID) { // 附着已有实例
        running = true; // 运行中
        if let Ok(collection) = presentations(&app) { // 集合
            count = get(&collection, "Count").map(|v| v.as_i32()).unwrap_or(0); // 数量
            for index in 1..=count { // 逐个
                if let Ok(pres) = call(&collection, "Item", &[Variant::I4(index)]).and_then(|v| v.as_dispatch()) { // 演示文稿
                    if let Ok(name) = get(&pres, "Name") { names.push(name.as_string()); } // 名称
                }
            }
        }
    }
    Ok(json!({"family": "ppt", "officeInstalled": office, "running": running, "presentationCount": count, "presentations": names})) // 返回
}
fn create(args: &Map<String, Value>) -> ComResult<Value> { // 新建
    let path = str_arg(args, "path"); // 路径
    if path.is_empty() { return Err(ToolError::new("INVALID_ARGUMENT", "create 必须传 path")); } // 校验
    let ctx = locator::create(&path)?; // 创建
    let mut data = locator::info_json(&ctx)?; // 信息
    data["created"] = json!(true); // 创建标记
    Ok(data) // 返回
}
fn open(args: &Map<String, Value>) -> ComResult<Value> { // 打开
    let path = str_arg(args, "path"); // 路径
    if path.is_empty() { return Err(ToolError::new("INVALID_ARGUMENT", "open 必须传 path")); } // 校验
    let ctx = locator::open(&path)?; // 打开
    let mut data = locator::info_json(&ctx)?; // 信息
    data["opened"] = json!(true); // 打开标记
    Ok(data) // 返回
}
fn save(args: &Map<String, Value>) -> ComResult<Value> { // 保存
    let ctx = locator::open(&str_arg(args, "path"))?; // 取演示文稿
    locator::save(&ctx) // 保存
}
fn save_as(args: &Map<String, Value>) -> ComResult<Value> { // 另存为
    let source = str_arg(args, "path"); // 源路径
    let ctx = locator::open(&source)?; // 源
    let target = str_arg(args, "targetPath"); // 目标
    if target.is_empty() { return Err(ToolError::new("INVALID_ARGUMENT", "save_as 必须传 targetPath")); } // 校验
    let full = Paths::load().resolve(&target)?; // 校验目标
    call(&ctx.pres, "SaveAs", &[Variant::Str(full.to_string_lossy().to_string()), Variant::I4(PP_SAVE_AS_OPEN_XML_PRESENTATION)])?; // 另存
    let _ = call(&ctx.pres, "Activate", &[]); // 激活
    crate::common::registry::forget(&source); crate::common::registry::put(&full.to_string_lossy(), POWERPOINT_PROGID, &ctx.app, &ctx.pres); // 同步路径登记
    Ok(json!({"path": full.to_string_lossy(), "format": "pptx"})) // 返回
}
fn export_pdf(args: &Map<String, Value>) -> ComResult<Value> { // 导出 PDF
    let ctx = locator::open(&str_arg(args, "path"))?; // 源
    let target = str_arg(args, "targetPath"); // 目标
    let full = if target.is_empty() { ctx.path.with_extension("pdf") } else { Paths::load().resolve(&target)? }; // 目标路径
    locator::export_pdf(&ctx, &full.to_string_lossy()) // 导出
}
fn close(args: &Map<String, Value>) -> ComResult<Value> { // 关闭
    let ctx = locator::open(&str_arg(args, "path"))?; // 取演示文稿
    let saved = get(&ctx.pres, "Saved").map(|v| v.as_bool()).unwrap_or(true); // 保存状态
    let save_changes = args.get("saveChanges").and_then(|v| v.as_bool()).unwrap_or(false); // 保存标志
    locator::close(&ctx, save_changes)?; // 关闭
    crate::common::registry::forget(&str_arg(args, "path")); // 清理路径登记
    Ok(json!({"closed": true, "hadUnsavedChanges": !saved})) // 返回
}
fn quit() -> ComResult<Value> { // 退出受控实例
    let apps = take_controlled(); // 受控 Application
    let mut quit_count = 0; // 退出数
    let mut closed: Vec<String> = Vec::new(); // 关闭的演示文稿
    for app in &apps { // 逐个
        if let Ok(collection) = presentations(app) { // 集合
            let count = get(&collection, "Count").map(|v| v.as_i32()).unwrap_or(0); // 数量
            for index in 1..=count { // 逐个关闭
                if let Ok(pres) = call(&collection, "Item", &[Variant::I4(index)]).and_then(|v| v.as_dispatch()) { // 演示文稿
                    if let Ok(full) = get(&pres, "FullName") { if !full.as_string().is_empty() { closed.push(full.as_string()); } } // 路径
                    let _ = call(&pres, "Close", &[]); // 关闭
                }
            }
        }
        if call(app, "Quit", &[]).is_ok() { quit_count += 1; } // 退出
    }
    crate::common::registry::forget_label(POWERPOINT_PROGID); // 清理 PPT 路径登记
    Ok(json!({"quitCount": quit_count, "closedDocs": closed})) // 返回
}
fn info(args: &Map<String, Value>) -> ComResult<Value> { // 信息
    let ctx = locator::open(&str_arg(args, "path"))?; // 取演示文稿
    locator::info_json(&ctx) // 返回
}
pub fn where_am_i(args: &Map<String, Value>) -> Value { // ppt_where_am_i
    let result: ComResult<Value> = (|| { // 错误传播闭包
        let path = str_arg(args, "path"); // 路径
        if !path.is_empty() { // 指定文件
            let ctx: PresCtx = locator::open(&path)?; // 取演示文稿
            let active = active_index(&ctx.app); // 当前幻灯片
            let count = get(&get(&ctx.pres, "Slides")?.as_dispatch()?, "Count").map(|v| v.as_i32()).unwrap_or(0); // 数量
            let full = ctx.path.to_string_lossy().to_string(); // 完整路径
            return Ok(json!({"path": full.clone(), "activePresentation": full, "slideCount": count, "slideIndex": active, "family": "ppt"})); // 返回
        }
        let app = attach_app(POWERPOINT_PROGID)?; // 附着
        let pres = get(&app, "ActivePresentation")?.as_dispatch()?; // 活动演示
        let full = get(&pres, "FullName").map(|v| v.as_string()).unwrap_or_default(); // 路径
        let count = get(&get(&pres, "Slides")?.as_dispatch()?, "Count").map(|v| v.as_i32()).unwrap_or(0); // 数量
        Ok(json!({"path": full.clone(), "activePresentation": full, "slideCount": count, "slideIndex": active_index(&app), "family": "ppt"})) // 返回
    })();
    match result { Ok(value) => ok_json(value), Err(err) => err_json(&err) } // 返回
}
fn active_index(app: &ComObj) -> i32 { // 当前幻灯片序号
    get(app, "ActiveWindow").ok().and_then(|v| v.as_dispatch().ok()).and_then(|window| get(&window, "View").ok()).and_then(|v| v.as_dispatch().ok()).and_then(|view| get(&view, "Slide").ok()).and_then(|v| v.as_dispatch().ok()).and_then(|slide| get(&slide, "SlideIndex").ok()).map(|v| v.as_i32()).unwrap_or(0) // 链式读取
}