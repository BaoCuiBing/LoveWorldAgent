pub mod adb; // adb 解析与同步执行
pub mod apps; // 应用列表/前台/启动/停止/安装/卸载
pub mod coords; // PNG 与设备分辨率换算
pub mod devices; // 设备列表
pub mod input; // 点击/滑动/输入/按键/剪贴板
pub mod logs; // 设备日志与崩溃
pub mod recording; // 屏幕录制
pub mod screen; // 截图/屏幕信息/方向
pub mod ui; // uiautomator 元素树
use serde::Deserialize; // 配置反序列化
use std::path::PathBuf; // 路径
use crate::config; // 静态目录
use crate::mobile::adb::ToolResult; // adb 结果别名
#[derive(Debug, Clone, Default, Deserialize)] // sidecar 设置
#[serde(default)] // 缺省字段
pub struct Settings { // .loveword/agent_mobile.json
    pub enabled: bool, // 总开关
    #[serde(rename = "adbPath")] // 后端字段名
    pub adb_path: String, // adb 路径
    #[serde(rename = "defaultDevice")] // 后端字段名
    pub default_device: String, // 默认设备
    #[serde(rename = "screenshotMaxSize")] // 后端字段名
    pub screenshot_max_size: u32, // 截图最大边
}
pub fn settings() -> Settings { // 读取设置
    let Some(paths) = config::detect_paths() else { return Settings::default(); }; // 未命中目录
    let dir = paths.static_dir.join(".loveword");
    let path = dir.join("agent_mobile.json"); // 新配置文件
    let target = if path.exists() { path } else { dir.join("mobile_option.json") }; // 兼容旧文件名
    std::fs::read_to_string(target).ok().and_then(|text| serde_json::from_str::<Settings>(&text).ok()).unwrap_or_default() // 容错解析
}
pub fn adb_and_device() -> ToolResult<(PathBuf, String)> { // 解析 adb 与目标设备
    let settings = settings(); // 读取配置
    let adb = adb::resolve_adb(Some(settings.adb_path.as_str()))?; // adb 路径
    let preferred = if settings.default_device.trim().is_empty() { None } else { Some(settings.default_device.as_str()) }; // 默认设备
    let serial = adb::select(&adb, preferred)?; // 选择设备
    Ok((adb, serial)) // 返回
}