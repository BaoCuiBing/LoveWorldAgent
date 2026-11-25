use std::path::PathBuf; // 帮助文档打开：仅允许 .loveword/doc_mds 内的白名单文件名，经 explorer 拉起默认程序
use std::process::Command; // 外部命令
use crate::config; // 文档目录配置
use crate::utils::logger; // 日志输出
pub fn doc_dir() -> PathBuf { // 帮助文档目录
    config::LOVEWORD_DIR.join("doc_mds") // 位于 loveword 目录下
}
pub struct OpenError(pub String); // 打开文档错误
impl std::fmt::Display for OpenError { // 错误展示
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { // 格式化
        f.write_str(&self.0) // 输出错误文本
    }
}
fn valid_name(text: &str) -> bool { // 校验文档名只含允许字符
    !text.is_empty() && text.chars().all(|ch| ch.is_alphanumeric() || matches!(ch, '.' | '-' | '_')) // 非空且字符合法
}
pub fn open_doc(name: &str) -> Result<String, OpenError> { // 校验并打开帮助文档
    let raw = name.trim(); // 去首尾空白
    if raw.is_empty() { return Err(OpenError("文档名不能为空".to_string())); } // 空名
    if raw.contains('/') || raw.contains('\\') { return Err(OpenError("文档名不能包含路径".to_string())); } // 禁止路径
    if !valid_name(raw) { return Err(OpenError("文档名只允许中文、英文字母、数字、点、横线与下划线".to_string())); } // 非法字符
    let file = if raw.to_lowercase().ends_with(".md") { raw.to_string() } else { format!("{raw}.md") }; // 补 .md 后缀
    let dir = doc_dir(); // 文档目录
    let path = dir.join(&file); // 文件路径
    if !path.starts_with(&dir) || !path.is_file() { return Err(OpenError(format!("文档不存在: {file}"))); } // 越界或不存在
    if let Err(err) = Command::new("explorer.exe").arg(&path).spawn() { logger::warn("open_file", &format!("拉起文档失败: {err}")); } // 调用资源管理器打开
    logger::info("open_file", &format!("打开帮助文档: {}", path.display())); // 记录
    Ok(path.to_string_lossy().to_string()) // 返回路径
}
