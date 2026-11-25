use serde_json::Value; // JSON 值
use crate::com::dispatch::{get_args, call, get, ComObj, ComResult, Variant}; // COM 工具

#[derive(Clone, Debug)] // 语义定位器
pub struct Locator { // 定位参数
    pub by: String, // heading/paragraph/table/bookmark/cursor/end
    pub matched: String, // 匹配文本
    pub match_mode: String, // exact/contains/startsWith/regex
    pub occurrence: usize, // 第几个匹配（1 起）
    pub offset_direction: String, // before/after
    pub offset_count: usize, // 偏移段落数
    pub name: String, // bookmark 名称
    pub level: i32, // 标题级别
}
impl Default for Locator { // 默认光标定位
    fn default() -> Self { // 构造
        Self { by: "cursor".into(), matched: String::new(), match_mode: "exact".into(), occurrence: 1, offset_direction: "after".into(), offset_count: 1, name: String::new(), level: 0 } // 默认
    }
}
impl Locator { // 解析与匹配
    pub fn from_json(value: Option<&Value>) -> Self { // 从 JSON 解析
        let Some(obj) = value.and_then(|v| v.as_object()) else { return Self::default(); }; // 无对象用默认
        let mut loc = Self::default(); // 起点
        if let Some(v) = obj.get("by").and_then(|v| v.as_str()) { loc.by = v.to_string(); } // 类型
        if let Some(v) = obj.get("match").and_then(|v| v.as_str()) { loc.matched = v.to_string(); } // 匹配
        if let Some(v) = obj.get("matchMode").and_then(|v| v.as_str()) { loc.match_mode = v.to_string(); } // 匹配方式
        if let Some(v) = obj.get("occurrence").and_then(|v| v.as_u64()) { loc.occurrence = v.max(1) as usize; } // 序号
        if let Some(v) = obj.get("offsetDirection").and_then(|v| v.as_str()) { loc.offset_direction = v.to_string(); } // 方向
        if let Some(v) = obj.get("offsetCount").and_then(|v| v.as_u64()) { loc.offset_count = v.max(1) as usize; } // 偏移
        if let Some(v) = obj.get("name").and_then(|v| v.as_str()) { loc.name = v.to_string(); } // 书签
        if let Some(v) = obj.get("level").and_then(|v| v.as_i64()) { loc.level = v as i32; } // 级别
        loc // 返回
    }
    pub fn matches(&self, text: &str) -> bool { // 文本匹配判定
        let needle = &self.matched; // 目标
        match self.match_mode.as_str() { // 按模式
            "contains" => text.contains(needle), // 包含
            "startsWith" => text.starts_with(needle), // 前缀
            "regex" => regex_match(needle, text), // 正则（简易）
            _ => text == needle, // 精确
        }
    }
}
fn regex_match(pattern: &str, text: &str) -> bool { // 极简正则：支持 .* 通配与字面量
    if pattern.is_empty() { return true; } // 空匹配
    if let Some((left, right)) = pattern.split_once(".*") { // 通配
        if let Some(pos) = text.find(left) { return text[pos + left.len()..].contains(right); } // 分段匹配
        return false; // 未匹配
    }
    text.contains(pattern) // 退化包含
}
pub struct LocateResult { // 定位结果
    pub found: bool, // 是否命中
    pub start: i32, // Range 起点
    pub end: i32, // Range 终点
    pub paragraph_index: i32, // 段落序号
    pub heading_context: String, // 所属标题
}
impl LocateResult { // 默认未命中
    fn none() -> Self { Self { found: false, start: 0, end: 0, paragraph_index: 0, heading_context: String::new() } } // 构造
}
pub fn locate_range(doc: &ComObj, loc: &Locator) -> ComResult<LocateResult> { // 在文档中定位 Range（8s 预算，按需扫描）
    match loc.by.as_str() { // 按类型
        "cursor" | "end" => locate_cursor(doc, loc), // 光标/文末
        "bookmark" => locate_bookmark(doc, loc), // 书签
        "heading" => scan_paragraphs(doc, loc, true), // 标题
        "paragraph" => scan_paragraphs(doc, loc, false), // 段落
        "table" => locate_table(doc, loc), // 表格
        _ => Ok(LocateResult::none()), // 未知类型
    }
}
fn locate_cursor(doc: &ComObj, loc: &Locator) -> ComResult<LocateResult> { // 光标或文末定位
    let app = get(doc, "Application")?.as_dispatch()?; // Application
    let sel = get(&app, "Selection")?.as_dispatch()?; // Selection
    let (start, end) = if loc.by == "end" { // 文末
        let content = get(doc, "Content")?.as_dispatch()?; // 正文
        let e = get(&content, "End")?.as_i32(); // 末尾
        (e, e) // 折叠到末尾
    } else { // 光标
        (get(&sel, "Start")?.as_i32(), get(&sel, "End")?.as_i32()) // 选区范围
    };
    Ok(LocateResult { found: true, start, end, paragraph_index: 0, heading_context: String::new() }) // 返回
}
fn locate_bookmark(doc: &ComObj, loc: &Locator) -> ComResult<LocateResult> { // 书签定位
    let bookmarks = get(doc, "Bookmarks")?.as_dispatch()?; // 集合
    let exists: bool = call(&bookmarks, "Exists", &[Variant::Str(loc.name.clone())])?.as_bool(); // 存在
    if !exists { return Ok(LocateResult::none()); } // 未找到
    let bm = call(&bookmarks, "Item", &[Variant::Str(loc.name.clone())])?.as_dispatch()?; // 取书签
    let start = get(&bm, "Start")?.as_i32(); // 起点
    let end = get(&bm, "End")?.as_i32(); // 终点
    Ok(LocateResult { found: true, start, end, paragraph_index: 0, heading_context: String::new() }) // 返回
}
fn scan_paragraphs(doc: &ComObj, loc: &Locator, heading: bool) -> ComResult<LocateResult> { // 扫描段落/标题
    let paragraphs = get(doc, "Paragraphs")?.as_dispatch()?; // 集合
    let count = get(&paragraphs, "Count")?.as_i32(); // 段落数
    let mut hits = 0usize; // 命中计数
    let mut context = String::new(); // 最近标题
    let mut i = 1; // 1 起
    while i <= count { // 遍历
        let para = call(&paragraphs, "Item", &[Variant::I4(i)])?.as_dispatch()?; // 段落
        let range = get(&para, "Range")?.as_dispatch()?; // Range
        let text = get(&range, "Text")?.as_string().trim().to_string(); // 文本
        let outline = get(&para, "OutlineLevel").map(|v| v.as_i32()).unwrap_or(10); // 大纲级别
        if heading { // 标题模式（正文段落若文本匹配也允许定位）
            if outline <= 9 { context = text.clone(); } // 更新上下文
            if outline >= 10 && !loc.matches(&text) { i += 1; continue; } // 正文且不匹配跳过
            if loc.level > 0 && outline < 10 && outline != loc.level { i += 1; continue; } // 非目标级别
        }
        if loc.matches(&text) { // 匹配
            hits += 1; // 计数
            if hits == loc.occurrence { // 命中目标
                let start = get(&range, "Start")?.as_i32(); // 起点
                let end = get(&range, "End")?.as_i32(); // 终点
                let (s, e) = apply_offset(doc, start, end, loc)?; // 应用偏移
                return Ok(LocateResult { found: true, start: s, end: e, paragraph_index: i, heading_context: context }); // 返回
            }
        }
        i += 1; // 下一段
    }
    Ok(LocateResult::none()) // 未命中
}
fn apply_offset(doc: &ComObj, start: i32, end: i32, loc: &Locator) -> ComResult<(i32, i32)> { // 相对偏移
    if loc.offset_count == 0 { return Ok((start, end)); } // 无偏移
    let paragraphs = get(doc, "Paragraphs")?.as_dispatch()?; // 集合
    let count = get(&paragraphs, "Count")?.as_i32(); // 数量
    let mut index = 1i32; // 当前段落
    let mut i = 1; // 遍历起点
    while i <= count { // 找起点所在段
        let para = call(&paragraphs, "Item", &[Variant::I4(i)])?.as_dispatch()?; // 段落
        let range = get(&para, "Range")?.as_dispatch()?; // Range
        if get(&range, "End")?.as_i32() >= start { index = i; break; } // 命中
        i += 1; // 下一段
    }
    let target = if loc.offset_direction == "before" { index - loc.offset_count as i32 } else { index + loc.offset_count as i32 }; // 目标段
    if target < 1 || target > count { return Ok((start, end)); } // 越界保持
    let para = call(&paragraphs, "Item", &[Variant::I4(target)])?.as_dispatch()?; // 目标段
    let range = get(&para, "Range")?.as_dispatch()?; // Range
    Ok((get(&range, "Start")?.as_i32(), get(&range, "End")?.as_i32())) // 返回
}
fn locate_table(doc: &ComObj, loc: &Locator) -> ComResult<LocateResult> { // 表格定位（按序号）
    let tables = get(doc, "Tables")?.as_dispatch()?; // 集合
    let count = get(&tables, "Count")?.as_i32(); // 数量
    if count < 1 { return Ok(LocateResult::none()); } // 无表格
    let index = loc.occurrence.min(count as usize) as i32; // 序号
    let table = call(&tables, "Item", &[Variant::I4(index)])?.as_dispatch()?; // 表格
    let range = get(&table, "Range")?.as_dispatch()?; // Range
    let start = get(&range, "Start")?.as_i32(); // 起点
    let end = get(&range, "End")?.as_i32(); // 终点
    Ok(LocateResult { found: true, start, end, paragraph_index: 0, heading_context: String::new() }) // 返回
}
