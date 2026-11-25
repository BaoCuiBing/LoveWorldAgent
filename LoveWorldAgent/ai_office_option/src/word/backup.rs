use chrono::Local; // 本地时间
use std::path::{Path, PathBuf}; // 路径类型
use crate::word::report::ToolError; // 错误类型
pub fn backup_before_save(path: &Path) -> Result<Option<PathBuf>, ToolError> { // 保存前备份，返回备份路径
    if !path.is_file() { return Ok(None); } // 文件不存在无需备份
    let stamp = Local::now().format("%Y%m%d-%H%M%S").to_string(); // 时间戳
    let backup = backup_name(path, &stamp); // 备份文件名
    std::fs::copy(path, &backup).map_err(|e| ToolError::new("BACKUP_FAILED", format!("备份失败：{e}")))?; // 复制
    rotate(path)?; // 按 agent_office.json backup.keep 轮转
    Ok(Some(backup)) // 返回
}
fn backup_name(path: &Path, stamp: &str) -> PathBuf { // {name}.{ext}.bak.yyyyMMdd-HHmmss
    let parent = path.parent().unwrap_or_else(|| Path::new(".")); // 父目录
    let file_name = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default(); // 文件名
    parent.join(format!("{file_name}.bak.{stamp}")) // 拼接
}
fn rotate(path: &Path) -> Result<(), ToolError> { // 按配置份数轮转
    rotate_in(path, crate::config::backup_keep().max(1))
}
pub fn rotate_in(path: &Path, keep: usize) -> Result<(), ToolError> { // 仅保留最近 keep 份
    let keep = keep.max(1);
    let parent = path.parent().unwrap_or_else(|| Path::new(".")); // 父目录
    let file_name = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default(); // 文件名
    let prefix = format!("{file_name}.bak."); // 备份前缀
    let mut backups: Vec<PathBuf> = std::fs::read_dir(parent).ok().into_iter().flatten().flatten() // 遍历目录
        .filter_map(|entry| { // 过滤备份
            let name = entry.file_name().to_string_lossy().to_string(); // 文件名
            if name.starts_with(&prefix) { Some(entry.path()) } else { None } // 命中
        }).collect(); // 收集
    backups.sort_by_key(|path| path.file_name().and_then(|n| n.to_str()).map(|s| s.to_string()).unwrap_or_default()); // 按名排序（时间戳后缀可字典序）
    if backups.len() > keep { // 超限
        for old in backups.iter().take(backups.len() - keep) { // 删除最旧
            let _ = std::fs::remove_file(old); // 忽略删除错误
        }
    }
    Ok(()) // 完成
}

#[cfg(test)] // 单元测试
mod tests { // 备份命名与轮转
    use super::*; // 引入
    #[test] // 命名格式
    fn names_with_stamp() { assert_eq!(backup_name(Path::new("E:/x/a.docx"), "20260101-010101"), PathBuf::from("E:/x/a.docx.bak.20260101-010101")); } // 断言
    #[test] // 只保留指定份数
    fn rotate_keeps_five() {
        let dir = std::env::temp_dir().join(format!("word_option_backup_{}", std::process::id())); // 临时目录
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap(); // 创建
        let main_path = dir.join("a.docx"); // 主文件
        std::fs::write(&main_path, b"x").unwrap(); // 写入
        for index in 0..6 { std::fs::write(dir.join(format!("a.docx.bak.20260101-01010{index}")), b"x").unwrap(); } // 6 份
        rotate_in(&main_path, 5).unwrap(); // 轮转
        let count = std::fs::read_dir(&dir).unwrap().flatten().filter(|entry| entry.file_name().to_string_lossy().starts_with("a.docx.bak.")).count(); // 统计
        assert_eq!(count, 5); // 保留 5
        rotate_in(&main_path, 2).unwrap(); // 自定义保留 2 份
        let count_two = std::fs::read_dir(&dir).unwrap().flatten().filter(|entry| entry.file_name().to_string_lossy().starts_with("a.docx.bak.")).count();
        assert_eq!(count_two, 2);
        let _ = std::fs::remove_dir_all(&dir); // 清理
    }
}
