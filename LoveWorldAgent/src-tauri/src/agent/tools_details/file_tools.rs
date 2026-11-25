use std::path::{Path, PathBuf}; // 通用文件工具：限项目内白名单的 file_list/read/write/delete，并写旁路元数据供文件卡与 diff 卡渲染
use serde_json::{json, Value}; // JSON 构造与动态值
use crate::agent::tools::{params, blocking, ToolDef}; // 工具定义与同步包装
use crate::agent::tools_details::common::{diff_hunks, remember_tool_meta, resolve_in_root, truncate_chars, NO_PROJECT, READ_LIMIT, WRITE_LIMIT}; // 路径门禁、diff 与元数据
fn text_of(path: &Path) -> String { // 读取文件文本，非文件或失败按空串
    if !path.is_file() { return String::new(); } // 非文件返回空串
    std::fs::read(path).map(|bytes| String::from_utf8_lossy(&bytes).to_string()).unwrap_or_default() // 读取并损失性解码
}
fn arg(args: &str, key: &str) -> String { // 从参数 JSON 读取字符串字段
    serde_json::from_str::<Value>(args).ok().and_then(|value| value.get(key).and_then(Value::as_str).map(str::to_string)).unwrap_or_default() // 解析失败返回空串
}
pub fn build_file_tools(root: Option<PathBuf>) -> Vec<ToolDef> { // 构造四个文件工具
    let base = root.clone(); // 克隆项目根供 list 使用
    let list = ToolDef::new("file_list", "列出项目内目录的一层条目（名称、类型、大小）；path 为项目相对路径，空串表示项目根。禁访目录与敏感配置会被拒绝", params(&[("path", "string", "项目相对路径，空串表示项目根")]), blocking(move |args: String| { // 列出目录条目
        let Some(root) = base.clone() else { return NO_PROJECT.to_string(); }; // 未选项目目录返回提示
        let path = arg(&args, "path"); // 目标相对路径
        let target = match resolve_in_root(&root, &path) { Ok(value) => value, Err(err) => { remember_tool_meta("file_list", json!({"path": path, "ok": false, "error": err})); return format!("列出失败：{err}"); } }; // 校验路径
        if !target.exists() { return format!("目录不存在: {path}"); } // 路径不存在
        if !target.is_dir() { return format!("不是目录: {path}"); } // 路径不是目录
        let mut entries: Vec<(bool, String, PathBuf)> = std::fs::read_dir(&target).map(|items| items.flatten().map(|item| { // 读取目录条目
            let name = item.file_name().to_string_lossy().to_string(); // 条目名
            let is_file = item.path().is_file(); // 是否文件
            (is_file, name.to_lowercase(), item.path()) // 三元组用于排序
        }).collect()).unwrap_or_default(); // 读取失败按空目录
        entries.sort(); // 按类型与小写名排序
        let items: Vec<Value> = entries.iter().map(|(is_file, _, path)| json!({"name": path.file_name().map(|name| name.to_string_lossy().to_string()).unwrap_or_default(), "type": if *is_file { "file" } else { "dir" }, "size": if *is_file { std::fs::metadata(path).map(|meta| meta.len()).unwrap_or(0) } else { 0 }})).collect(); // 投影为名称/类型/大小
        remember_tool_meta("file_list", json!({"path": path, "entries": items.len(), "items": items.iter().take(200).cloned().collect::<Vec<Value>>()})); // 写元数据，列表最多 200 项
        if items.is_empty() { "（空目录）".to_string() } else { serde_json::to_string(&items).unwrap_or_else(|_| "（空目录）".to_string()) } // 空目录给占位
    }));
    let base = root.clone(); // 克隆项目根供 read 使用
    let read = ToolDef::new("file_read", "读取项目内文本文件（≤200KB），path 为项目相对路径。二进制文件与敏感配置不可读", params(&[("path", "string", "项目相对路径")]), blocking(move |args: String| { // 读取文本文件
        let Some(root) = base.clone() else { return NO_PROJECT.to_string(); }; // 未选项目目录返回提示
        let path = arg(&args, "path"); // 目标相对路径
        let target = match resolve_in_root(&root, &path) { Ok(value) => value, Err(err) => { remember_tool_meta("file_read", json!({"path": path, "ok": false, "error": err})); return format!("读取失败：{err}"); } }; // 校验路径
        if !target.is_file() { remember_tool_meta("file_read", json!({"path": path, "ok": false, "error": "文件不存在"})); return format!("文件不存在: {path}"); } // 非文件
        let size = std::fs::metadata(&target).map(|meta| meta.len() as usize).unwrap_or(0); // 取文件大小
        if size > READ_LIMIT { remember_tool_meta("file_read", json!({"path": path, "ok": false, "error": "文件超过 200KB"})); return format!("文件超过 200KB，禁止读取: {path}"); } // 超限拒绝
        let text = text_of(&target); // 读取正文
        let lines = text.matches('\n').count() + if text.is_empty() { 0 } else { 1 }; // 统计行数
        remember_tool_meta("file_read", json!({"path": path, "ok": true, "bytes": text.len(), "startLine": 1, "endLine": lines.max(1), "lines": lines.max(1)})); // 写成功元数据
        text // 返回正文
    }));
    let base = root.clone(); // 克隆项目根供 write 使用
    let write = ToolDef::new("file_write", "新建或覆盖项目内文本文件（≤1MB）；写前先用 file_read 了解现状，不要写入二进制或敏感配置", params(&[("path", "string", "项目相对路径"), ("content", "string", "文件全文")]), blocking(move |args: String| { // 写入文本文件
        let Some(root) = base.clone() else { return NO_PROJECT.to_string(); }; // 未选项目目录返回提示
        let value: Value = serde_json::from_str(&args).unwrap_or_else(|_| json!({})); // 解析参数
        let path = value.get("path").and_then(Value::as_str).unwrap_or_default().to_string(); // 目标相对路径
        let content = value.get("content").and_then(Value::as_str).unwrap_or_default().to_string(); // 文件全文
        if content.len() > WRITE_LIMIT { remember_tool_meta("file_write", json!({"path": path, "ok": false, "error": "内容超过 1MB"})); return format!("内容超过 1MB，禁止写入: {path}"); } // 超限拒绝
        let target = match resolve_in_root(&root, &path) { Ok(item) => item, Err(err) => { remember_tool_meta("file_write", json!({"path": path, "ok": false, "error": err})); return format!("写入失败：{err}"); } }; // 校验路径
        let before = text_of(&target); // 读取旧内容用于生成 diff
        if let Some(parent) = target.parent() { if let Err(err) = std::fs::create_dir_all(parent) { return format!("写入失败：{err}"); } } // 确保父目录存在
        if let Err(err) = std::fs::write(&target, &content) { remember_tool_meta("file_write", json!({"path": path, "ok": false, "error": err.to_string()})); return format!("写入失败：{err}"); } // 写入文件，失败写元数据
        let (additions, deletions, hunks) = diff_hunks(&before, &content); // 计算逐行 diff
        let bytes = std::fs::metadata(&target).map(|meta| meta.len()).unwrap_or(0); // 取写入后大小
        remember_tool_meta("file_write", json!({"path": path, "ok": true, "created": before.is_empty(), "before": before, "after": content, "additions": additions, "deletions": deletions, "hunks": hunks, "truncated": hunks.len() >= 400, "bytes": bytes})); // 写 diff 元数据
        format!("已写入 {path}（{bytes} 字节）") // 返回成功提示
    }));
    let base = root; // 移动项目根供 delete 使用
    let delete = ToolDef::new("file_delete", "删除项目内单个文件（不允许删除目录）；仅在用户明确要求时调用，删除前先确认路径", params(&[("path", "string", "项目相对路径")]), blocking(move |args: String| { // 删除文件
        let Some(root) = base.clone() else { return NO_PROJECT.to_string(); }; // 未选项目目录返回提示
        let path = arg(&args, "path"); // 目标相对路径
        let target = match resolve_in_root(&root, &path) { Ok(value) => value, Err(err) => { remember_tool_meta("file_delete", json!({"path": path, "ok": false, "error": err})); return format!("删除失败：{err}"); } }; // 校验路径
        if !target.is_file() { remember_tool_meta("file_delete", json!({"path": path, "ok": false, "error": "文件不存在或不是文件"})); return format!("文件不存在或不是文件: {path}"); } // 非文件
        let before = text_of(&target); // 删除前读取原文，用于生成删除 diff
        match std::fs::remove_file(&target) { // 执行删除
            Ok(()) => { let (additions, deletions, hunks) = diff_hunks(&before, ""); remember_tool_meta("file_delete", json!({"path": path, "ok": true, "before": before, "after": "", "additions": additions, "deletions": deletions, "hunks": hunks, "truncated": hunks.len() >= 400})); format!("已删除 {}", truncate_chars(&path, 300)) } // 删除视为全部删行，写入 diff 元数据
            Err(err) => { remember_tool_meta("file_delete", json!({"path": path, "ok": false, "error": err.to_string()})); format!("删除失败：{err}") } // 删除失败写元数据
        }
    }));
    vec![list, read, write, delete] // 返回四个工具
}
