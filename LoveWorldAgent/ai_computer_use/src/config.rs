use serde::Deserialize; // 反序列化派生
use std::path::{Path, PathBuf}; // 路径类型
use std::sync::OnceLock; // 一次性初始化
pub const CONFIG_NAME: &str = "config.json"; // 主应用配置文件名
pub const COMPUTER_USE_CONFIG_NAME: &str = "agent_computer_use.json"; // 电脑控制配置文件名
pub const STATIC_DIR_NAME: &str = "static"; // 静态资源目录名
pub const COMPUTER_SCREEN_DIR_NAME: &str = "computer_screen"; // 电脑控制自动截图子目录名
pub const LOG_DIR_NAME: &str = "logs"; // 日志目录默认子目录
pub const DEFAULT_MAX_LINES: usize = 30000; // 归档阈值默认值（与主应用一致）
pub const MIN_MAX_LINES: usize = 100; // 归档阈值下限
const MAX_UP_LEVELS: usize = 8; // 开发态向上探测层数
#[derive(Debug, Clone, Default, Deserialize)] // 主应用配置（只取所需字段）
#[serde(default)] // 缺省字段取默认值
struct FileConfig { // 主应用 config.json
    app: FileApp, // app 段
    logging: FileLogging, // logging 段
}
#[derive(Debug, Clone, Default, Deserialize)] // app 段
#[serde(default)] // 缺省字段取默认值
struct FileApp { // app 段字段
    log_level: String, // 日志级别（空则 INFO）
}
#[derive(Debug, Clone, Deserialize)] // logging 段
#[serde(default)] // 缺省字段取默认值
struct FileLogging { // logging 段字段
    dir: String, // 日志目录（空→static/logs，相对→相对根，绝对→原样）
    max_lines: usize, // 归档阈值
}
impl Default for FileLogging { // 默认 logging 段
    fn default() -> Self { Self { dir: String::new(), max_lines: DEFAULT_MAX_LINES } } // 与主应用默认值对齐
}
#[derive(Debug, Clone)] // 根目录探测结果
pub struct Paths { // 探测到的目录
    pub root: PathBuf, // 根目录（开发态项目根 / 打包态 app_dir 或资源目录）
    pub static_dir: PathBuf, // 静态资源目录
}
#[derive(Debug, Clone)] // 运行期配置
pub struct Runtime { // 日志与路径配置快照
    pub paths: Option<Paths>, // 未命中时为 None
    pub config_file: Option<PathBuf>, // 命中的主应用配置
    pub log_dir: PathBuf, // 日志目录（已合并环境变量）
    pub log_level: String, // 日志级别（已合并环境变量）
    pub log_max_lines: usize, // 归档阈值（不低于 MIN_MAX_LINES）
}
static RUNTIME: OnceLock<Runtime> = OnceLock::new(); // 全局配置快照
pub fn runtime() -> &'static Runtime { // 获取运行期配置
    RUNTIME.get_or_init(load) // 首次调用时解析
}
pub fn exe_dir() -> PathBuf { // 可执行文件目录
    std::env::current_exe().ok().and_then(|path| path.parent().map(|parent| parent.to_path_buf())).unwrap_or_else(|| PathBuf::from(".")) // 失败回退当前目录
}
fn dev_paths(from: &Path) -> Option<Paths> { // 开发态：向上命中 src-tauri/static/config.json
    for dir in from.ancestors().take(MAX_UP_LEVELS) { // 逐级向上探测
        let static_dir = dir.join("src-tauri").join(STATIC_DIR_NAME); // 开发态静态目录
        if static_dir.join(CONFIG_NAME).is_file() { return Some(Paths { root: dir.to_path_buf(), static_dir }); } // 命中即返回
    }
    None // 未命中
}
fn packaged_paths(from: &Path) -> Option<Paths> { // 打包态：可写 app_dir/static 优先，资源目录回退
    let app = from.parent().map(|parent| parent.to_path_buf()).unwrap_or_else(|| from.to_path_buf()); // sidecar 位于 <app_dir>/computer_use
    let candidates = [app.join(STATIC_DIR_NAME), from.join(STATIC_DIR_NAME), app.join("resources").join(STATIC_DIR_NAME), app.join("resource").join(STATIC_DIR_NAME)]; // 候选静态目录
    for static_dir in candidates { // 逐个尝试
        if static_dir.join(CONFIG_NAME).is_file() { // 命中配置文件
            let root = static_dir.parent().map(|parent| parent.to_path_buf()).unwrap_or_else(|| app.clone()); // 根目录取配置文件上一级
            return Some(Paths { root, static_dir }); // 返回结果
        }
    }
    None // 未命中
}
pub fn detect_paths() -> Option<Paths> { // 统一根目录探测
    dev_paths(&exe_dir()).or_else(|| packaged_paths(&exe_dir())) // 先开发态后打包态
}
fn env_value(key: &str) -> Option<String> { // 读取非空环境变量
    std::env::var(key).ok().map(|value| value.trim().to_string()).filter(|value| !value.is_empty()) // 空值视为未设置
}
fn resolve_log_dir(paths: Option<&Paths>, logging: &FileLogging) -> PathBuf { // 解析日志目录
    if let Some(raw) = env_value("LOG_DIR") { return PathBuf::from(raw); } // 环境变量优先
    let Some(paths) = paths else { return exe_dir().join(LOG_DIR_NAME); }; // 未探测到根目录时回退 exe/logs
    let dir = logging.dir.trim(); // 配置目录
    if dir.is_empty() { return paths.static_dir.join(LOG_DIR_NAME); } // 空→static/logs
    let path = PathBuf::from(dir); // 配置值转路径
    if path.is_absolute() { path } else { paths.root.join(path) } // 绝对原样，相对相对根
}
fn resolve_log_level(app: &FileApp) -> String { // 解析日志级别
    env_value("LOG_LEVEL").or_else(|| Some(app.log_level.trim().to_string()).filter(|value| !value.is_empty())).unwrap_or_else(|| "INFO".to_string()) // 环境变量 > app.log_level > INFO
}
fn load() -> Runtime { // 解析配置并合并环境变量
    let paths = detect_paths(); // 探测根目录
    let config_file = paths.as_ref().map(|paths| paths.static_dir.join(CONFIG_NAME)); // 命中的配置文件
    let file_config = config_file.as_ref().and_then(|path| std::fs::read_to_string(path).ok()).and_then(|text| serde_json::from_str::<FileConfig>(&text).ok()).unwrap_or_default(); // 读取失败用默认值
    Runtime { // 组装运行期配置
        log_dir: resolve_log_dir(paths.as_ref(), &file_config.logging), // 日志目录
        log_level: resolve_log_level(&file_config.app), // 日志级别
        log_max_lines: file_config.logging.max_lines.max(MIN_MAX_LINES), // 阈值下限保护
        config_file,
        paths,
    }
}
pub fn resolve_static_dir_with(env_dir: Option<&str>, paths: Option<&Paths>, exe: &Path) -> PathBuf { // 解析静态资源目录：LOVEWORD_STATIC_DIR 优先
    if let Some(raw) = env_dir.map(str::trim).filter(|value| !value.is_empty()) {
        return PathBuf::from(raw);
    }
    if let Some(paths) = paths {
        return paths.static_dir.clone();
    }
    exe.join(STATIC_DIR_NAME)
}
pub fn static_dir() -> PathBuf { // 获取当前可写 static 目录
    resolve_static_dir_with(env_value("LOVEWORD_STATIC_DIR").as_deref(), runtime().paths.as_ref(), &exe_dir())
}
pub fn computer_use_config_path() -> PathBuf { // 电脑控制配置文件路径（static/.loveword/agent_computer_use.json）
    static_dir().join(".loveword").join(COMPUTER_USE_CONFIG_NAME)
}
pub fn screenshot_dir() -> PathBuf { // 电脑控制自动截图落盘目录（static/data/computer_screen）
    static_dir().join("data").join(COMPUTER_SCREEN_DIR_NAME)
}
pub fn parse_auto_screenshot(raw: &str) -> bool { // 解析 auto_screenshot 开关（缺省或无效返回 true）
    let Ok(value) = serde_json::from_str::<serde_json::Value>(raw) else { return true; };
    match value.get("auto_screenshot") {
        Some(serde_json::Value::Bool(flag)) => *flag,
        Some(serde_json::Value::Number(num)) => num.as_f64().map(|v| v != 0.0).unwrap_or(true),
        Some(serde_json::Value::String(text)) => !matches!(text.trim().to_ascii_lowercase().as_str(), "" | "false" | "0" | "no" | "off"),
        _ => true,
    }
}
pub fn auto_screenshot_enabled() -> bool { // 动态读取当前 auto_screenshot 配置（缺失/异常默认 true）
    match std::fs::read_to_string(computer_use_config_path()) {
        Ok(raw) => parse_auto_screenshot(&raw),
        Err(_) => true,
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn static_dir_prefers_env_over_detected_paths() {
        let detected = Paths { root: PathBuf::from("C:\\proj"), static_dir: PathBuf::from("C:\\proj\\src-tauri\\static") };
        let exe = PathBuf::from("C:\\exe");
        assert_eq!(resolve_static_dir_with(Some("D:\\custom_static"), Some(&detected), &exe), PathBuf::from("D:\\custom_static"));
        assert_eq!(resolve_static_dir_with(Some("  "), Some(&detected), &exe), PathBuf::from("C:\\proj\\src-tauri\\static"));
        assert_eq!(resolve_static_dir_with(None, None, &exe), PathBuf::from("C:\\exe\\static"));
    }
    #[test]
    fn auto_screenshot_defaults_true_when_missing_or_invalid() {
        assert!(parse_auto_screenshot("{}"));
        assert!(parse_auto_screenshot("invalid json"));
        assert!(parse_auto_screenshot(r#"{"auto_screenshot": true}"#));
        assert!(!parse_auto_screenshot(r#"{"auto_screenshot": false}"#));
        assert!(!parse_auto_screenshot(r#"{"auto_screenshot": "false"}"#));
    }
    #[test]
    fn screenshot_dir_resolves_under_static_data_computer_screen() {
        let base = resolve_static_dir_with(Some("E:\\Test\\static"), None, Path::new("E:\\exe"));
        assert_eq!(base.join("data").join(COMPUTER_SCREEN_DIR_NAME), PathBuf::from("E:\\Test\\static\\data\\computer_screen"));
    }
}
