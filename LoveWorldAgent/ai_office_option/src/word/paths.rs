use std::path::{Path, PathBuf}; // 路径类型
use crate::word::report::ToolError; // 错误类型
#[derive(Clone)] // 路径策略快照
pub struct Paths { // 路径校验器
    pub project_dir: PathBuf, // 项目目录（相对路径基准）
}
impl Paths { // 校验操作
    pub fn load() -> Self { // 构造路径策略
        let project_dir = crate::config::runtime().paths.as_ref().map(|p| p.root.clone()).unwrap_or_else(|| std::env::current_dir().unwrap_or_default()); // 项目目录
        Self { project_dir } // 返回
    }
    pub fn resolve(&self, path: &str) -> Result<PathBuf, ToolError> { // 校验并规范化路径
        let trimmed = path.trim(); // 去空白
        if trimmed.is_empty() { return Err(ToolError::new("INVALID_ARGUMENT", "path 不能为空")); } // 空路径
        let raw = PathBuf::from(trimmed); // 转路径
        if raw.components().any(|component| matches!(component, std::path::Component::ParentDir)) { return Err(ToolError::new("PATH_DENIED", "路径不允许包含 ..")); } // 拒绝穿越
        let absolute = if raw.is_absolute() { raw } else { self.project_dir.join(raw) }; // 相对转绝对
        let normalized = normalize(&absolute); // 规范化（消解 ..）
        check_path(&normalized)?; // 基础安全检查
        Ok(normalized) // 返回
    }
}
fn check_path(path: &Path) -> Result<(), ToolError> { // 基础安全检查（支持本机所有合法路径，仅拦截 UNC 与设备路径）
    let text = path.to_string_lossy(); // 字符串
    if text.starts_with(r"\\.\") || text.starts_with(r"\\?\GLOBALROOT") { return Err(ToolError::new("PATH_DENIED", "设备路径被拒绝")); } // 设备路径（优先于 UNC 判定）
    if text.starts_with(r"\\") || text.starts_with("//") { return Err(ToolError::new("PATH_DENIED", "网络路径不支持")); } // UNC 路径
    Ok(()) // 放行其它本地路径
}
pub fn normalize(path: &Path) -> PathBuf { // 消解 . 与 .. 的规范化
    let mut out = PathBuf::new(); // 结果
    for component in path.components() { // 逐段处理
        use std::path::Component::*; // 组件类型
        match component { // 分派
            ParentDir => { out.pop(); } // 回退
            CurDir => {} // 忽略
            Prefix(prefix) => out.push(prefix.as_os_str()), // 盘符
            RootDir => out.push(std::path::MAIN_SEPARATOR.to_string()), // 根
            Normal(part) => out.push(part), // 普通段
        }
    }
    out // 返回
}
#[cfg(test)] // 单元测试
mod tests { // 路径校验
    use super::*; // 引入
    fn policy() -> Paths { Paths { project_dir: PathBuf::from("E:/BiSaiCodeProjects/LoveWorldAgent") } } // 策略
    #[test] // 拒绝穿越
    fn rejects_parent_dir() { assert_eq!(policy().resolve("../evil.docx").unwrap_err().code, "PATH_DENIED"); } // ..
    #[test] // 拒绝 UNC
    fn rejects_unc() { let err = policy().resolve("//server/share/x.docx").unwrap_err(); assert_eq!(err.code, "PATH_DENIED"); assert!(err.message.contains("网络路径不支持")); } // 网络
    #[test] // 相对路径
    fn allows_project_relative() { assert!(policy().resolve("docx/a.docx").is_ok()); } // 相对
    #[test] // 绝对路径不再受目录限制
    fn allows_outside() { assert!(policy().resolve("C:/Windows/a.docx").is_ok()); } // 任意本地路径
    #[test] // 拒绝设备路径
    fn rejects_device_path() {
        assert_eq!(policy().resolve(r"\\.\C:\a.docx").unwrap_err().code, "PATH_DENIED");
        assert_eq!(policy().resolve(r"\\?\GLOBALROOT\Device\HarddiskVolume1\a.docx").unwrap_err().code, "PATH_DENIED");
    }
    #[test] // 放行任意本机磁盘路径
    fn allows_arbitrary_local_drive_paths() {
        assert!(policy().resolve(r"D:\docs\test.docx").is_ok());
        assert!(policy().resolve(r"C:\Users\Public\a.xlsx").is_ok());
    }
    #[test] // 规范化
    fn normalizes_cur_dir() { assert_eq!(normalize(Path::new("E:/a/./b")), PathBuf::from("E:/a/b")); } // .
}
