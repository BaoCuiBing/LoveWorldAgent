use serde::Serialize; // 元素序列化
use std::path::Path; // adb 路径
use std::sync::{Mutex, OnceLock}; // 元素缓存
use std::time::Duration; // 超时
use crate::mobile::adb::{self, ToolError, ToolResult}; // adb 与错误
#[derive(Debug, Clone, Serialize)] // UI 元素
pub struct UiNode { // uiautomator 节点
    pub index: usize, // 节点序号
    #[serde(rename = "ref")] // 对外字段名
    pub reference: String, // @eN 引用
    pub text: String, // 文本
    pub resource_id: String, // 资源 id
    pub class: String, // 类名
    pub content_desc: String, // 内容描述
    pub clickable: bool, // 可点击
    pub enabled: bool, // 可用
    pub bounds: [i64; 4], // [x1,y1,x2,y2]
    pub center: [i64; 2], // 中心点
}
static CACHE: OnceLock<Mutex<Vec<UiNode>>> = OnceLock::new(); // 最近一次元素树
fn cache() -> &'static Mutex<Vec<UiNode>> { CACHE.get_or_init(|| Mutex::new(Vec::new())) } // 取缓存
fn decode(raw: &str) -> String { // 解码 XML 实体
    raw.replace("&amp;", "&").replace("&lt;", "<").replace("&gt;", ">").replace("&quot;", "\"").replace("&apos;", "'").replace("&#10;", "\n").replace("&#13;", "\r").replace("&#39;", "'") // 常见实体
}
fn attr(tag: &str, name: &str) -> Option<String> { // 读取标签属性
    let needle = format!(" {}=\"", name); // 属性前缀
    let start = tag.find(&needle)? + needle.len(); // 值起点
    let rest = &tag[start..]; // 余下文本
    let end = rest.find('"')?; // 值终点
    Some(decode(&rest[..end])) // 返回解码值
}
fn parse_bounds(raw: &str) -> Option<[i64; 4]> { // 解析 [x1,y1][x2,y2]
    let text = raw.trim(); // 去空白
    let inner = text.strip_prefix('[')?.strip_suffix(']')?; // 去掉外层
    let (first, second) = inner.split_once("][")?; // 拆两段
    let (x1, y1) = first.split_once(',')?; // 左上
    let (x2, y2) = second.split_once(',')?; // 右下
    Some([x1.trim().parse().ok()?, y1.trim().parse().ok()?, x2.trim().parse().ok()?, y2.trim().parse().ok()?]) // 返回
}
pub fn parse(xml: &str) -> Vec<UiNode> { // 解析 uiautomator XML
    let mut nodes = Vec::new(); // 结果
    let mut cursor = 0usize; // 扫描位置
    while let Some(found) = xml[cursor..].find("<node ") { // 查找节点开始
        let start = cursor + found; // 绝对起点
        let Some(end_found) = xml[start..].find('>') else { break; }; // 标签结束
        let end = start + end_found + 1; // 绝对终点
        let tag = &xml[start..end]; // 标签文本
        cursor = end; // 前进
        let Some(bounds) = attr(tag, "bounds").and_then(|raw| parse_bounds(&raw)) else { continue; }; // 无边界跳过
        let text = attr(tag, "text").unwrap_or_default(); // 文本
        let resource_id = attr(tag, "resource-id").unwrap_or_default(); // 资源 id
        let content_desc = attr(tag, "content-desc").unwrap_or_default(); // 内容描述
        let class = attr(tag, "class").unwrap_or_default(); // 类名
        let clickable = attr(tag, "clickable").map(|value| value == "true").unwrap_or(false); // 可点击
        let enabled = attr(tag, "enabled").map(|value| value != "false").unwrap_or(true); // 可用
        if text.is_empty() && resource_id.is_empty() && content_desc.is_empty() && !clickable { continue; } // 忽略纯容器
        let index = nodes.len(); // 序号
        let center = [(bounds[0] + bounds[2]) / 2, (bounds[1] + bounds[3]) / 2]; // 中心点
        nodes.push(UiNode { index, reference: format!("@e{}", index + 1), text, resource_id, class, content_desc, clickable, enabled, bounds, center }); // 收集节点
        if nodes.len() >= 4000 { break; } // 数量上限
    }
    nodes // 返回
}
pub fn dump(adb: &Path, serial: &str) -> ToolResult<Vec<UiNode>> { // 重新抓取元素树
    adb::shell(adb, serial, &["uiautomator", "dump", "/sdcard/window_dump.xml"], Duration::from_secs(30))?; // 生成 XML
    let xml = adb::shell(adb, serial, &["cat", "/sdcard/window_dump.xml"], Duration::from_secs(20))?; // 读取 XML
    if !xml.contains("<hierarchy") { return Err(ToolError::new("INVALID_ARGUMENT", "uiautomator dump 输出无效")); } // 校验内容
    let nodes = parse(&xml); // 解析
    if let Ok(mut guard) = cache().lock() { *guard = nodes.clone(); } // 更新缓存
    Ok(nodes) // 返回
}
pub fn cached() -> Vec<UiNode> { cache().lock().map(|guard| guard.clone()).unwrap_or_default() } // 读取缓存
pub fn node_by_ref(reference: &str) -> Option<UiNode> { // 按 @eN 查找
    let raw = reference.trim().trim_start_matches('@').trim_start_matches('e'); // 归一化
    let index: usize = raw.parse().ok()?; // 解析序号（1 起）
    cached().into_iter().find(|node| node.index + 1 == index) // 命中
}