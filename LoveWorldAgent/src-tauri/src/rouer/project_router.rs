use std::collections::HashMap; // 项目目录路由：项目选择、文件浏览/文本读写、二进制预览、条目增删复制与越界防护
use std::path::{Component, Path, PathBuf}; // 路径类型与组件
use axum::body::{Body, Bytes}; // 响应体与请求体
use axum::extract::Query; // 查询参数
use axum::http::{header, StatusCode}; // 响应头与状态码
use axum::response::{IntoResponse, Response}; // 响应 trait
use axum::routing::{get, post}; // GET/POST 方法
use axum::Router; // 路由器类型
use serde_json::{json, Value}; // JSON 构造
use walkdir::WalkDir; // 目录遍历
use crate::config; // 项目路径配置
use crate::state::{body_object, fail, obj_str, ok, ApiResult, AppState}; // 请求解析与响应工具
pub const READ_LIMIT: u64 = 200 * 1024; // 文本读取上限 200KB
pub const WRITE_LIMIT: usize = 1024 * 1024; // 写入上限 1MB
pub const RAW_LIMIT: u64 = 50 * 1024 * 1024; // 预览上限 50MB
pub const MAX_SEARCH: usize = 500; // 搜索/列举条数上限
pub const MAX_DEPTH: usize = 6; // 搜索最大深度
pub const NO_PROJECT: &str = "请先选择项目文件夹"; // 未选项目提示
pub const SKIP_DIRS: [&str; 11] = [".git", ".venv", "node_modules", "__pycache__", "dist", "webdist", "build", "out", "third_party", ".trae", ".vs"]; // 搜索跳过的目录
pub const FORBIDDEN_DIRS: [&str; 6] = [".venv", "node_modules", "dist", "webdist", ".git", "__pycache__"]; // 禁止访问的目录
pub const FORBIDDEN_FILES: [&str; 6] = ["src-tauri/static/config.json", "src-tauri/static/.loveword/agent_config.json", "src-tauri/static/.loveword/agent_permission.json", "src-tauri/static/.loveword/agent_mcp.json", "src-tauri/static/.loveword/agent_browser.json", "src-tauri/static/.loveword/agent_computer_use.json"]; // 禁止访问的文件
pub fn router() -> Router<AppState> { // 注册项目路由
    Router::new() // 创建路由器
        .route("/api/project", get(get_project).put(put_project)) // 项目路径读写
        .route("/api/project/files", get(list_files)) // 文件列举与搜索
        .route("/api/project/file", get(read_file).put(write_file)) // 文本读写
        .route("/api/project/file/raw", get(read_file_raw)) // 二进制预览
        .route("/api/project/entry", get(stat_entry).post(create_entry).delete(remove_entry)) // 条目信息与增删
        .route("/api/project/copy", post(copy_entry)) // 复制条目
}
fn rel_of(target: &Path, root: &Path) -> String { // 取目标相对项目根的路径字符串
    target.strip_prefix(root).map(|rel| rel.components().map(|part| part.as_os_str().to_string_lossy().to_string()).collect::<Vec<String>>().join("/")).unwrap_or_default() // 统一用正斜杠
}
fn normalize_path(path: &Path) -> PathBuf { // 手工规范化路径，消解 . 与 ..
    let mut result = PathBuf::new(); // 结果路径
    for component in path.components() { // 逐组件处理
        match component { // 按组件类型分派
            Component::ParentDir => { result.pop(); } // 上级目录则回退
            Component::CurDir => {} // 当前目录忽略
            other => result.push(other.as_os_str()), // 其他组件追加
        }
    }
    result // 返回规范化路径
}
fn forbidden(root: &Path, target: &Path) -> bool { // 判断目标是否位于禁访目录
    let rel = rel_of(target, root).to_lowercase(); // 相对路径小写
    rel.split('/').any(|part| FORBIDDEN_DIRS.contains(&part)) // 任一段命中禁访目录
}
fn resolve_target(root: &Path, raw: &str) -> Result<PathBuf, (u16, String)> { // 校验并解析项目内路径
    let text = raw.trim().replace('\\', "/"); // 统一斜杠
    let text = text.trim_start_matches('/'); // 去前导斜杠
    let target = if text.is_empty() || text == "." { root.to_path_buf() } else { normalize_path(&root.join(text)) }; // 空路径返回根
    if target != root && !target.starts_with(root) { return Err((400, format!("路径越界，仅允许项目内相对路径: {text}"))); } // 越界拒绝
    let rel = rel_of(&target, root).to_lowercase(); // 相对路径
    if rel.split('/').any(|part| FORBIDDEN_DIRS.contains(&part)) { return Err((403, format!("禁止访问该目录: {text}"))); } // 禁访目录拒绝
    if FORBIDDEN_FILES.contains(&rel.as_str()) { return Err((403, "禁止读写智能体配置文件".to_string())); } // 禁访文件拒绝
    if rel.contains(".loveword/agents/") { return Err((403, "禁止读写智能体提示词".to_string())); } // 子智能体提示词拒绝
    Ok(target) // 返回合法路径
}
fn has_children(path: &Path, root: &Path) -> bool { // 判断目录是否含子项
    if forbidden(root, path) { return false; } // 禁访目录视为无子项
    std::fs::read_dir(path).map(|mut entries| entries.next().is_some()).unwrap_or(false) // 有任一条目即为 true
}
fn item_of(path: &Path, root: &Path) -> Value { // 路径转条目 JSON
    let is_dir = path.is_dir(); // 是否目录
    let size = if is_dir { 0 } else { std::fs::metadata(path).map(|info| info.len()).unwrap_or(0) }; // 文件大小
    json!({"name": path.file_name().map(|name| name.to_string_lossy().to_string()).unwrap_or_default(), "type": if is_dir { "dir" } else { "file" }, "size": size, "rel": rel_of(path, root), "abs": path.to_string_lossy(), "has_children": if is_dir { has_children(path, root) } else { false }}) // 投影字段
}
fn children_of(target: &Path, root: &Path) -> std::io::Result<Vec<Value>> { // 列举目录直接子项
    let mut entries: Vec<(bool, String, PathBuf)> = std::fs::read_dir(target)?.flatten().map(|entry| { // 读取条目
        let path = entry.path(); // 条目路径
        (path.is_file(), path.file_name().map(|name| name.to_string_lossy().to_lowercase()).unwrap_or_default(), path) // 三元组用于排序
    }).collect();
    entries.sort_by(|left, right| left.0.cmp(&right.0).then_with(|| left.1.cmp(&right.1))); // 目录在前按名排序
    Ok(entries.iter().map(|(_, _, path)| item_of(path, root)).collect()) // 转为条目列表
}
fn search(root: &Path, keyword: &str) -> Vec<Value> { // 按文件名搜索，限制深度与数量
    let mut items: Vec<Value> = Vec::new(); // 结果
    let walker = WalkDir::new(root).max_depth(MAX_DEPTH).into_iter().filter_entry(|entry| { // 限制深度
        if entry.depth() == 0 || !entry.file_type().is_dir() { return true; } // 根与非目录保留
        let name = entry.file_name().to_string_lossy().to_string(); // 目录名
        !SKIP_DIRS.contains(&name.as_str()) // 跳过隐藏与依赖目录
    });
    for entry in walker.flatten() { // 遍历
        if entry.depth() == 0 { continue; } // 跳过根目录
        if !entry.file_name().to_string_lossy().to_lowercase().contains(keyword) { continue; } // 名称不匹配
        items.push(item_of(entry.path(), root)); // 写入结果
        if items.len() >= MAX_SEARCH { break; } // 达上限停止
    }
    items // 返回结果
}
fn unique_target(target: &Path) -> PathBuf { // 为目标路径生成不冲突的副本名
    if !target.exists() { return target.to_path_buf(); } // 不存在直接用
    let stem = target.file_stem().map(|name| name.to_string_lossy().to_string()).unwrap_or_default(); // 主名
    let suffix = target.extension().map(|name| format!(".{}", name.to_string_lossy())).unwrap_or_default(); // 扩展名
    let parent = target.parent().map(|path| path.to_path_buf()).unwrap_or_default(); // 父目录
    let mut index = 1usize; // 副本序号
    loop { // 递增直到不冲突
        let name = if index == 1 { format!("{stem}-copy{suffix}") } else { format!("{stem}-copy{index}{suffix}") }; // 副本名
        let candidate = parent.join(name); // 候选路径
        if !candidate.exists() { return candidate; } // 不冲突则返回
        index += 1; // 递增
    }
}
fn count_tree(target: &Path) -> usize { // 统计目录树条目数（上限 MAX_SEARCH）
    let mut total = 0usize; // 计数
    for _entry in WalkDir::new(target).min_depth(1).into_iter().flatten() { // 遍历子树
        total += 1; // 累加
        if total >= MAX_SEARCH { return total; } // 达上限提前返回
    }
    total // 返回总数
}
fn raw_mime(path: &str) -> &'static str { // 按扩展名推断预览 MIME
    match Path::new(path).extension().map(|ext| ext.to_string_lossy().to_lowercase()).unwrap_or_default().as_str() { // 取小写扩展名
        "png" => "image/png", // PNG
        "jpg" | "jpeg" => "image/jpeg", // JPEG
        "gif" => "image/gif", // GIF
        "webp" => "image/webp", // WebP
        "bmp" => "image/bmp", // BMP
        "svg" => "image/svg+xml", // SVG
        "ico" => "image/x-icon", // ICO
        "pdf" => "application/pdf", // PDF
        "doc" => "application/msword", // DOC
        "docx" => "application/vnd.openxmlformats-officedocument.wordprocessingml.document", // DOCX
        "xls" => "application/vnd.ms-excel", // XLS
        "xlsx" => "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet", // XLSX
        "ppt" => "application/vnd.ms-powerpoint", // PPT
        "pptx" => "application/vnd.openxmlformats-officedocument.presentationml.presentation", // PPTX
        _ => "", // 不支持的类型
    }
}
fn project_root() -> Result<PathBuf, ApiResult> { // 取项目根，未选返回错误
    config::project_path().ok_or_else(|| fail(400, NO_PROJECT)) // 转为 Result
}
async fn get_project() -> ApiResult { // 读取当前项目路径
    let path = config::project_path().map(|path| path.to_string_lossy().to_string()).unwrap_or_default(); // 路径字符串
    ok(json!({"path": path})) // 返回
}
async fn put_project(body: Bytes) -> ApiResult { // 设置项目路径
    let payload = body_object(&body); // 解析请求体
    match config::save_project_path(&obj_str(payload.as_object().unwrap(), "path")) { // 保存路径
        Ok(path) => ok(json!({"path": path.to_string_lossy()})), // 成功返回
        Err(err) => { // 失败分类
            let message = err.to_string(); // 错误信息
            if message.contains("不能为空") || message.contains("不存在") { fail(400, &message) } else { fail(500, "写入配置失败") } // 参数错误返回 400
        }
    }
}
async fn list_files(Query(query): Query<HashMap<String, String>>) -> ApiResult { // 列举文件，支持关键字搜索
    let root = match project_root() { Ok(root) => root, Err(result) => return result }; // 取项目根
    let raw = query.get("path").cloned().unwrap_or_default(); // 目标路径
    let target = match resolve_target(&root, &raw) { // 校验路径
        Ok(path) => path, // 合法路径
        Err((code, message)) => return fail(code, &message), // 校验失败
    };
    let keyword = query.get("q").cloned().unwrap_or_default().trim().to_lowercase(); // 搜索关键字
    if !keyword.is_empty() { // 有关键字走搜索
        let items = search(&root, &keyword); // 执行搜索
        return ok(json!({"total": items.len(), "list": items})); // 返回搜索结果
    }
    if !target.is_dir() { return fail(400, &format!("不是目录: {raw}")); } // 非目录
    match children_of(&target, &root) { // 列举子项
        Ok(items) => ok(json!({"total": items.len(), "list": items})), // 成功返回
        Err(err) => fail(400, &format!("读取目录失败: {err}")), // 失败
    }
}
async fn read_file(Query(query): Query<HashMap<String, String>>) -> ApiResult { // 读取文本文件内容
    let root = match project_root() { Ok(root) => root, Err(result) => return result }; // 取项目根
    let raw = query.get("path").cloned().unwrap_or_default(); // 目标路径
    let target = match resolve_target(&root, &raw) { // 校验路径
        Ok(path) => path, // 合法路径
        Err((code, message)) => return fail(code, &message), // 校验失败
    };
    if !target.is_file() { return fail(400, &format!("文件不存在: {raw}")); } // 非文件
    let size = match std::fs::metadata(&target) { Ok(info) => info.len(), Err(err) => return fail(400, &format!("读取失败: {err}")) }; // 取大小
    if size > READ_LIMIT { return fail(400, "文件超过 200KB，禁止读取"); } // 超限
    let data = match std::fs::read(&target) { Ok(bytes) => bytes, Err(err) => return fail(400, &format!("读取失败: {err}")) }; // 读取
    if data.contains(&0) { return fail(400, "二进制文件不支持读取"); } // 二进制拒绝
    ok(json!({"path": rel_of(&target, &root), "content": String::from_utf8_lossy(&data), "size": data.len()})) // 返回内容
}
async fn read_file_raw(Query(query): Query<HashMap<String, String>>) -> Response { // 二进制预览
    let root = match project_root() { Ok(root) => root, Err(result) => return result.into_response() }; // 取项目根
    let raw = query.get("path").cloned().unwrap_or_default(); // 目标路径
    let mime = raw_mime(&raw); // 推断 MIME
    if mime.is_empty() { return fail(400, "该文件类型不支持预览").into_response(); } // 类型不支持
    let target = match resolve_target(&root, &raw) { // 校验路径
        Ok(path) => path, // 合法路径
        Err((code, message)) => return fail(code, &message).into_response(), // 校验失败
    };
    if !target.is_file() { return fail(404, &format!("文件不存在: {raw}")).into_response(); } // 非文件
    let size = match std::fs::metadata(&target) { Ok(info) => info.len(), Err(err) => return fail(500, &format!("读取失败: {err}")).into_response() }; // 取大小
    if size > RAW_LIMIT { return fail(400, "文件超过 50MB，禁止预览").into_response(); } // 超限
    match std::fs::read(&target) { // 读取
        Ok(data) => Response::builder().status(StatusCode::OK).header(header::CONTENT_TYPE, mime).header(header::CACHE_CONTROL, "no-store").header("X-Content-Type-Options", "nosniff").body(Body::from(data)).unwrap_or_else(|_| fail(500, "读取失败").into_response()), // 返回二进制响应
        Err(err) => fail(500, &format!("读取失败: {err}")).into_response(), // 失败
    }
}
async fn write_file(body: Bytes) -> ApiResult { // 原子写入文本文件
    let root = match project_root() { Ok(root) => root, Err(result) => return result }; // 取项目根
    let payload = body_object(&body); // 解析请求体
    let raw = obj_str(payload.as_object().unwrap(), "path"); // 目标路径
    let target = match resolve_target(&root, &raw) { // 校验路径
        Ok(path) => path, // 合法路径
        Err((code, message)) => return fail(code, &message), // 校验失败
    };
    if target == root || target.is_dir() { return fail(400, "目标必须是文件"); } // 必须是文件
    let data = obj_str(payload.as_object().unwrap(), "content").into_bytes(); // 写入内容
    if data.len() > WRITE_LIMIT { return fail(400, "内容超过 1MB，禁止写入"); } // 超限
    if !target.parent().map(|parent| parent.is_dir()).unwrap_or(false) { return fail(400, &format!("父目录不存在: {}", rel_of(target.parent().unwrap_or(root.as_path()), &root))); } // 父目录校验
    let temp = target.with_file_name(format!("{}.tmp", target.file_name().map(|name| name.to_string_lossy().to_string()).unwrap_or_default())); // 临时文件路径
    let result = std::fs::write(&temp, &data).and_then(|_| { if target.exists() { std::fs::remove_file(&target)?; } std::fs::rename(&temp, &target) }); // 写临时文件后重命名
    if let Err(err) = result { crate::utils::logger::error("project_router", &format!("写入文件失败: {err}")); return fail(500, &format!("写入失败: {err}")); } // 失败回滚提示
    crate::utils::logger::info("project_router", &format!("已写入项目文件: {} ({} 字节)", rel_of(&target, &root), data.len())); // 记录成功
    ok(json!({"path": rel_of(&target, &root), "size": data.len()})) // 返回结果
}
async fn stat_entry(Query(query): Query<HashMap<String, String>>) -> ApiResult { // 查询条目信息，目录附带子项
    let root = match project_root() { Ok(root) => root, Err(result) => return result }; // 取项目根
    let raw = query.get("path").cloned().unwrap_or_default(); // 目标路径
    let target = match resolve_target(&root, &raw) { // 校验路径
        Ok(path) => path, // 合法路径
        Err((code, message)) => return fail(code, &message), // 校验失败
    };
    if !target.exists() { return fail(404, &format!("条目不存在: {raw}")); } // 不存在
    let info = match std::fs::metadata(&target) { Ok(info) => info, Err(err) => return fail(400, &format!("读取失败: {err}")) }; // 取元信息
    let is_dir = target.is_dir(); // 是否目录
    let mut data = json!({"path": rel_of(&target, &root), "type": if is_dir { "dir" } else { "file" }, "size": info.len(), "mtime": info.modified().map(crate::agent::store_sqls::db::stamp_of).unwrap_or_default()}); // 基础信息
    if is_dir { // 目录附加子项列表
        let mut children: Vec<(bool, String, PathBuf)> = match std::fs::read_dir(&target) { Ok(entries) => entries.flatten().map(|entry| { let path = entry.path(); (path.is_file(), path.file_name().map(|name| name.to_string_lossy().to_lowercase()).unwrap_or_default(), path) }).collect(), Err(err) => return fail(400, &format!("读取目录失败: {err}")) }; // 读取子项
        children.sort_by(|left, right| left.0.cmp(&right.0).then_with(|| left.1.cmp(&right.1))); // 排序
        data["children"] = Value::Array(children.iter().take(MAX_SEARCH).map(|(_, _, path)| item_of(path, &root)).collect()); // 子项列表限 500
        data["total"] = json!(children.len()); // 子项总数
    }
    ok(data) // 返回信息
}
async fn create_entry(body: Bytes) -> ApiResult { // 新建文件或目录
    let root = match project_root() { Ok(root) => root, Err(result) => return result }; // 取项目根
    let payload = body_object(&body); // 解析请求体
    let raw = obj_str(payload.as_object().unwrap(), "path"); // 目标路径
    let kind = payload.get("type").and_then(Value::as_str).unwrap_or("file").trim().to_lowercase(); // 条目类型
    if kind != "file" && kind != "dir" { return fail(400, "type 必须是 file/dir"); } // 类型校验
    let target = match resolve_target(&root, &raw) { // 校验路径
        Ok(path) => path, // 合法路径
        Err((code, message)) => return fail(code, &message), // 校验失败
    };
    if target == root { return fail(400, "不能创建项目根目录本身"); } // 不能创建根
    if target.exists() { return fail(409, "同名已存在"); } // 已存在
    if !target.parent().map(|parent| parent.is_dir()).unwrap_or(false) { return fail(400, &format!("父目录不存在: {}", rel_of(target.parent().unwrap_or(root.as_path()), &root))); } // 父目录校验
    let result = if kind == "dir" { std::fs::create_dir(&target) } else { std::fs::write(&target, b"") }; // 创建目录或空文件
    if let Err(err) = result { crate::utils::logger::error("project_router", &format!("创建条目失败: {err}")); return fail(500, &format!("创建失败: {err}")); } // 失败
    crate::utils::logger::info("project_router", &format!("已创建{}: {}", if kind == "dir" { "目录" } else { "文件" }, rel_of(&target, &root))); // 记录成功
    ok(json!({"path": rel_of(&target, &root), "type": kind})) // 返回结果
}
async fn remove_entry(Query(query): Query<HashMap<String, String>>) -> ApiResult { // 删除文件或目录
    let root = match project_root() { Ok(root) => root, Err(result) => return result }; // 取项目根
    let raw = query.get("path").cloned().unwrap_or_default(); // 目标路径
    let target = match resolve_target(&root, &raw) { // 校验路径
        Ok(path) => path, // 合法路径
        Err((code, message)) => return fail(code, &message), // 校验失败
    };
    if target == root { return fail(400, "不能删除项目根目录"); } // 不能删根
    if !target.exists() { return fail(404, &format!("条目不存在: {raw}")); } // 不存在
    let is_dir = target.is_dir(); // 是否目录
    let removed = if is_dir { count_tree(&target) } else { 1 }; // 统计删除项数
    let result = if is_dir { std::fs::remove_dir_all(&target) } else { std::fs::remove_file(&target) }; // 执行删除
    if let Err(err) = result { crate::utils::logger::error("project_router", &format!("删除失败: {err}")); return fail(500, &format!("删除失败: {err}")); } // 失败
    crate::utils::logger::info("project_router", &format!("已删除{}: {} ({} 项)", if is_dir { "目录" } else { "文件" }, rel_of(&target, &root), removed)); // 记录成功
    ok(json!({"path": rel_of(&target, &root), "type": if is_dir { "dir" } else { "file" }, "removed": removed})) // 返回结果
}
async fn copy_entry(body: Bytes) -> ApiResult { // 复制文件或目录，冲突自动改名
    let root = match project_root() { Ok(root) => root, Err(result) => return result }; // 取项目根
    let payload = body_object(&body); // 解析请求体
    let src_raw = obj_str(payload.as_object().unwrap(), "src"); // 源路径
    let dest_raw = obj_str(payload.as_object().unwrap(), "dest"); // 目标路径
    let src = match resolve_target(&root, &src_raw) { Ok(path) => path, Err((code, message)) => return fail(code, &message) }; // 校验源
    let dest = match resolve_target(&root, &dest_raw) { Ok(path) => path, Err((code, message)) => return fail(code, &message) }; // 校验目标
    if !src.exists() { return fail(404, &format!("源不存在: {src_raw}")); } // 源不存在
    if src == root { return fail(400, "不能复制项目根目录"); } // 不能复制根
    if dest == root { return fail(400, "目标不能是项目根"); } // 目标不能是根
    if src.is_dir() && (dest == src || dest.starts_with(&src)) { return fail(400, "不能把目录复制到自身内部"); } // 防递归复制
    if !dest.parent().map(|parent| parent.is_dir()).unwrap_or(false) { return fail(400, &format!("目标目录不存在: {}", rel_of(dest.parent().unwrap_or(root.as_path()), &root))); } // 目标父目录校验
    let final_path = unique_target(&dest); // 生成不冲突路径
    let is_dir = src.is_dir(); // 是否目录
    let result = if is_dir { crate::agent::copy_tree(&src, &final_path) } else { std::fs::copy(&src, &final_path).map(|_| ()) }; // 执行复制
    if let Err(err) = result { crate::utils::logger::error("project_router", &format!("复制失败: {err}")); return fail(500, &format!("复制失败: {err}")); } // 失败
    crate::utils::logger::info("project_router", &format!("已复制{}: {} -> {}", if is_dir { "目录" } else { "文件" }, rel_of(&src, &root), rel_of(&final_path, &root))); // 记录成功
    ok(json!({"path": rel_of(&final_path, &root), "type": if is_dir { "dir" } else { "file" }, "renamed": final_path != dest})) // 返回结果
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn forbidden_files_blocks_agent_computer_use_config() {
        assert!(FORBIDDEN_FILES.contains(&"src-tauri/static/.loveword/agent_computer_use.json"));
        let root = std::env::temp_dir().join(format!("lwa_proj_router_{}", std::process::id()));
        let _ = std::fs::create_dir_all(&root);
        let err = resolve_target(&root, "src-tauri/static/.loveword/agent_computer_use.json").unwrap_err();
        assert_eq!(err.0, 403);
        let _ = std::fs::remove_dir_all(&root);
    }
}
