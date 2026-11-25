use std::collections::HashMap; // 附件上传路由：multipart 落盘 uploads 目录，另提供图片预览与越界防护删除
use std::path::PathBuf; // 路径类型
use axum::body::{Body, Bytes}; // 响应体与请求体
use axum::extract::{DefaultBodyLimit, Multipart, Query}; // multipart 与查询参数
use axum::http::{header, StatusCode}; // 响应头与状态码
use axum::response::{IntoResponse, Response}; // 响应 trait
use axum::routing::{get, post}; // GET/POST 方法
use axum::Router; // 路由器类型
use serde_json::json; // JSON 构造
use crate::config; // 可写目录配置
use crate::state::{body_object, fail, json_str, ok, ApiResult, AppState}; // 请求解析与响应工具
pub const MAX_SIZE: usize = 20 * 1024 * 1024; // 附件大小上限 20MB
pub fn router() -> Router<AppState> { // 注册上传路由
    Router::new() // 创建路由器
        .route("/api/upload", post(upload_file).delete(delete_file)) // 上传与删除
        .route("/api/upload/preview", get(preview_file)) // 图片预览
        .layer(DefaultBodyLimit::max(MAX_SIZE + 1024 * 1024)) // 放宽请求体上限
}
fn upload_dir() -> PathBuf { // 上传目录路径
    config::writable_static_dir().join("uploads") // 位于可写 static 下
}
fn safe_name(name: &str) -> String { // 净化文件名，防路径穿越
    let raw = name.replace('\\', "/"); // 统一斜杠
    let last = raw.rsplit('/').next().unwrap_or_default().trim(); // 取末段文件名
    let base = if last.is_empty() { "file" } else { last }; // 空名用默认值
    let cleaned: String = base.chars().map(|ch| if ch.is_alphanumeric() || matches!(ch, '.' | '-' | '_') { ch } else { '_' }).collect(); // 非法字符替换为下划线
    let trimmed = cleaned.trim_start_matches('.').to_string(); // 去前导点
    let result = if trimmed.is_empty() { "file".to_string() } else { trimmed }; // 空名用默认值
    result.chars().take(80).collect() // 截断到 80 字符
}
fn resolve_upload(value: &str) -> Option<PathBuf> { // 解析并校验附件路径在上传目录内
    let trimmed = value.trim(); // 去首尾空白
    if trimmed.is_empty() { return None; } // 空值返回 None
    let last = trimmed.replace('\\', "/"); // 统一斜杠
    let name = safe_name(last.rsplit('/').next().unwrap_or_default()); // 净化文件名
    let root = upload_dir(); // 上传目录
    let target = root.join(&name); // 目标路径
    if target.parent()? != root.as_path() || !target.is_file() { return None; } // 越界或非文件返回 None
    Some(target) // 返回合法路径
}
fn image_mime(path: &std::path::Path) -> &'static str { // 按扩展名推断图片 MIME
    match path.extension().map(|ext| ext.to_string_lossy().to_lowercase()).unwrap_or_default().as_str() { // 取小写扩展名
        "png" => "image/png", // PNG
        "jpg" | "jpeg" => "image/jpeg", // JPEG
        "webp" => "image/webp", // WebP
        "gif" => "image/gif", // GIF
        "bmp" => "image/bmp", // BMP
        _ => "", // 非图片返回空
    }
}
async fn upload_file(mut multipart: Multipart) -> Response { // 处理附件上传
    let mut found: Option<(String, Bytes)> = None; // 保存文件名与内容
    while let Ok(Some(field)) = multipart.next_field().await { // 遍历 multipart 字段
        if field.name() != Some("file") { continue; } // 只处理 file 字段
        let name = field.file_name().unwrap_or_default().to_string(); // 原始文件名
        match field.bytes().await { // 读取字段内容
            Ok(bytes) => { found = Some((name, bytes)); break; } // 命中即退出
            Err(err) => return fail(400, &format!("读取上传内容失败: {err}")).into_response(), // 读取失败
        }
    }
    let Some((origin, body)) = found else { return fail(400, "缺少 file 字段").into_response(); }; // 缺少文件字段
    if body.is_empty() { return fail(400, "文件内容为空").into_response(); } // 空文件
    if body.len() > MAX_SIZE { return fail(400, "文件超过 20MB 上限").into_response(); } // 超限
    let dir = upload_dir(); // 上传目录
    if let Err(err) = std::fs::create_dir_all(&dir) { crate::utils::logger::error("upload_router", &format!("创建上传目录失败: {err}")); return fail(500, "保存附件失败").into_response(); } // 确保目录存在
    let stamp = chrono::Local::now().format("%Y%m%d_%H%M%S").to_string(); // 时间戳前缀
    let target = dir.join(format!("{stamp}_{}", safe_name(&origin))); // 生成目标文件名
    if let Err(err) = std::fs::write(&target, &body) { crate::utils::logger::error("upload_router", &format!("保存附件失败: {err}")); return fail(500, "保存附件失败").into_response(); } // 落盘
    crate::utils::logger::info("upload_router", &format!("上传附件: {} ({} 字节)", target.file_name().map(|name| name.to_string_lossy().to_string()).unwrap_or_default(), body.len())); // 记录上传日志
    ok(json!({"name": target.file_name().map(|name| name.to_string_lossy().to_string()).unwrap_or_default(), "origin": origin, "size": body.len(), "path": target.to_string_lossy()})).into_response() // 返回附件信息
}
async fn preview_file(Query(query): Query<HashMap<String, String>>) -> Response { // 图片预览
    let name = query.get("name").cloned().unwrap_or_default(); // 文件名
    let Some(target) = resolve_upload(&name) else { return fail(404, "附件不存在").into_response(); }; // 校验路径
    let mime = image_mime(&target); // 推断 MIME
    if mime.is_empty() { return fail(400, "该文件不是可预览图片").into_response(); } // 非图片
    match std::fs::read(&target) { // 读取文件
        Ok(bytes) => Response::builder().status(StatusCode::OK).header(header::CONTENT_TYPE, mime).body(Body::from(bytes)).unwrap_or_else(|_| fail(500, "读取附件失败").into_response()), // 返回图片响应
        Err(_) => fail(500, "读取附件失败").into_response(), // 读取失败
    }
}
async fn delete_file(Query(query): Query<HashMap<String, String>>, body: Bytes) -> ApiResult { // 删除附件
    let payload = body_object(&body); // 解析请求体
    let value = payload.get("path").or_else(|| payload.get("name")).map(|item| json_str(Some(item))).filter(|text| !text.is_empty()).or_else(|| query.get("path").cloned()).unwrap_or_default(); // 取路径或名称
    let Some(target) = resolve_upload(&value) else { return fail(404, "附件不存在或路径越界"); }; // 校验路径
    if let Err(err) = std::fs::remove_file(&target) { crate::utils::logger::error("upload_router", &format!("删除附件失败: {err}")); return fail(500, "删除附件失败"); } // 删除文件
    crate::utils::logger::info("upload_router", &format!("删除附件: {}", target.file_name().map(|name| name.to_string_lossy().to_string()).unwrap_or_default())); // 记录删除日志
    ok(json!({"name": target.file_name().map(|name| name.to_string_lossy().to_string()).unwrap_or_default()})) // 返回结果
}
