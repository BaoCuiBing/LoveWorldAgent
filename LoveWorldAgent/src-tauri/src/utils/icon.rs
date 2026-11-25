use std::path::{Path, PathBuf}; // 图标工具：确保运行时窗口图标资源存在，必要时从随包资源复制到可写目录
use crate::config; // 图标路径配置
use crate::utils::logger; // 日志输出
pub const ICON_FILE_NAME: &str = "dragon.ico"; // 窗口图标文件名
pub fn is_valid_ico(path: &Path) -> bool { // 校验文件是否为有效 ICO（头部魔数）
    match std::fs::read(path) { // 读取文件
        Ok(bytes) => bytes.len() > 6 && bytes[0] == 0 && bytes[1] == 0 && bytes[2] == 1 && bytes[3] == 0, // ICO 头：保留 0、类型 1
        Err(_) => false, // 读取失败视为无效
    }
}
pub fn ensure_icon() -> Option<PathBuf> { // 确保可写目录有有效图标，否则从随包复制
    let cache = config::ICON_CACHE_DIR.join(ICON_FILE_NAME); // 缓存路径
    if is_valid_ico(&cache) { return Some(cache); } // 缓存有效直接用
    let bundled = config::STATIC_DIR.join("imgs").join(ICON_FILE_NAME); // 随包图标路径
    if is_valid_ico(&bundled) { // 随包图标有效
        if let Some(parent) = cache.parent() { let _ = std::fs::create_dir_all(parent); } // 确保缓存目录存在
        match std::fs::copy(&bundled, &cache) { // 复制到缓存
            Ok(_) => { // 复制成功
                logger::info("icon", &format!("已复制窗口图标: {}", cache.display())); // 记录
                return Some(cache); // 返回缓存路径
            }
            Err(err) => { // 复制失败回退随包
                logger::warn("icon", &format!("复制窗口图标失败: {err}")); // 记录告警
                return Some(bundled); // 返回随包路径
            }
        }
    }
    logger::warn("icon", &format!("窗口图标资源缺失，使用系统默认图标: {}", (*config::WINDOW_ICON).display())); // 资源缺失
    None // 用系统默认
}
