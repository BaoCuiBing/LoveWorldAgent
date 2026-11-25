use serde_json::{json, Map, Value}; // JSON 构造
use crate::com::constants::MSO_TRUE; // COM 布尔真
use crate::com::dispatch::{call, get, ComResult, Variant}; // COM 工具
use crate::common::paths::Paths; // 路径策略
use crate::ppt::locator::{self, f64_arg, i32_arg, str_arg}; // 上下文与辅助
use crate::word::report::{err_json, ok_json, ToolError}; // 信封

pub fn run(args: &Map<String, Value>) -> Value { // ppt_media 3 action
    let action = str_arg(args, "action"); // 动作
    let result: ComResult<Value> = match action.as_str() { // 分派
        "image" => image(args), // 图片
        "audio" => media(args, "audio"), // 音频
        "video" => media(args, "video"), // 视频
        other => Err(ToolError::new("INVALID_ARGUMENT", format!("未知 ppt_media action：{other}"))), // 未知
    };
    match result { Ok(value) => ok_json(value), Err(err) => err_json(&err) } // 返回
}
fn slide_of(args: &Map<String, Value>) -> ComResult<crate::com::dispatch::ComObj> { // 取幻灯片
    let ctx = locator::open(&str_arg(args, "path"))?; // 上下文
    locator::slide(&ctx, i32_arg(args, "slideIndex", 1)) // 幻灯片
}
fn image(args: &Map<String, Value>) -> ComResult<Value> { // 插入图片
    let slide = slide_of(args)?; // 幻灯片
    let image_path = Paths::load().resolve(&str_arg(args, "imagePath"))?; // 图片
    if !image_path.is_file() { return Err(ToolError::new("FILE_NOT_FOUND", format!("图片不存在：{}", image_path.display()))); } // 不存在
    let shapes = get(&slide, "Shapes")?.as_dispatch()?; // 形状集
    let shape = call(&shapes, "AddPicture", &[Variant::Str(image_path.to_string_lossy().to_string()), Variant::I4(MSO_TRUE), Variant::I4(0), Variant::R4(f64_arg(args, "left", 80.0) as f32), Variant::R4(f64_arg(args, "top", 80.0) as f32), Variant::R4(f64_arg(args, "width", 320.0) as f32), Variant::R4(f64_arg(args, "height", 240.0) as f32)])?.as_dispatch()?; // 插入
    Ok(json!({"inserted": true, "shapeName": get(&shape, "Name").map(|v| v.as_string()).unwrap_or_default()})) // 返回
}
fn media(args: &Map<String, Value>, kind: &str) -> ComResult<Value> { // 插入音视频
    let slide = slide_of(args)?; // 幻灯片
    let source = str_arg(args, "mediaPath"); // 媒体路径
    let full = Paths::load().resolve(&source)?; // 校验
    if !full.is_file() { return Err(ToolError::new("FILE_NOT_FOUND", format!("{kind} 文件不存在：{}", full.display()))); } // 不存在
    let shapes = get(&slide, "Shapes")?.as_dispatch()?; // 形状集
    let shape = call(&shapes, "AddMediaObject2", &[Variant::Str(full.to_string_lossy().to_string()), Variant::I4(MSO_TRUE), Variant::I4(MSO_TRUE), Variant::R4(f64_arg(args, "left", 80.0) as f32), Variant::R4(f64_arg(args, "top", 80.0) as f32), Variant::R4(f64_arg(args, "width", 320.0) as f32), Variant::R4(f64_arg(args, "height", 240.0) as f32)])?.as_dispatch()?; // 插入
    Ok(json!({"inserted": true, "kind": kind, "shapeName": get(&shape, "Name").map(|v| v.as_string()).unwrap_or_default()})) // 返回
}