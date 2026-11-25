use crate::com::constants::{CM_TO_POINTS, WD_BORDER_BOTTOM, WD_PAGE_BREAK}; // 常量
use crate::com::dispatch::{get_args, call, get, set, ComObj, ComResult, Variant}; // COM 工具
use crate::word::locator::{locate_range, Locator}; // 语义定位
use crate::word::paths::Paths; // 图片路径校验
use crate::word::report::{Report, ToolError, Warning}; // 结果与警告
#[derive(Debug, Default, Clone, Copy)] // 渲染统计
pub struct RenderReport { // 返回给 word_write
    pub blocks: usize, // 渲染块数
    pub chars: usize, // 正文字符数
}
#[derive(Debug, Clone, Default)] // 行内片段
struct Span { // 单个行内片段
    text: String, // 纯文本
    bold: bool, // 粗体
    italic: bool, // 斜体
    strike: bool, // 删除线
    code: bool, // 行内代码
    link: Option<String>, // 链接地址
}
#[derive(Debug, Clone)] // 块类型
enum Block { // Markdown 块
    Heading(usize, Vec<Span>), // 标题（级别/行内）
    Paragraph(Vec<Span>), // 段落
    Bullet(Vec<Span>), // 无序列表项
    Numbered(Vec<Span>), // 有序列表项
    Quote(Vec<Span>), // 引用
    Code(String), // 代码块
    Table(Vec<Vec<String>>), // 表格
    Rule, // 水平分割线
    PageBreak, // 分页符
    Image(String), // 图片
    Blank, // 空行
}
pub fn render(doc: &ComObj, markdown: &str, loc: &Locator, report: &mut Report) -> ComResult<RenderReport> { // 渲染 Markdown 到定位范围
    let located = locate_range(doc, loc)?; // 定位插入点
    if !located.found { return Err(ToolError::new("INVALID_ARGUMENT", "定位失败，未找到插入位置")); } // 未命中
    let blocks = parse(markdown); // 解析块
    let mut pos = located.start; // 当前插入点（cursor 时从选区起点开始）
    let mut stats = RenderReport::default(); // 统计
    for block in &blocks { // 逐块渲染（insert 模式始终插入，不替换目标段落）
        if matches!(block, Block::Blank) { continue; } // 空行跳过
        let next = render_block(doc, block, pos, pos, report)?; // 渲染单块（折叠插入）
        pos = next; // 推进插入点
        stats.blocks += 1; // 计数
        stats.chars += block_chars(block); // 字符
    }
    Ok(stats) // 返回
}
fn render_block(doc: &ComObj, block: &Block, start: i32, end: i32, report: &mut Report) -> ComResult<i32> { // 渲染单块并返回新插入点
    match block { // 分派
        Block::Heading(level, spans) => render_paragraph(doc, spans, start, end, report, Some(*level), false, false), // 标题
        Block::Paragraph(spans) => render_paragraph(doc, spans, start, end, report, None, false, false), // 段落
        Block::Bullet(spans) => render_paragraph(doc, spans, start, end, report, None, true, false), // 无序列表
        Block::Numbered(spans) => render_paragraph(doc, spans, start, end, report, None, false, true), // 有序列表
        Block::Quote(spans) => { // 引用
            let next = render_paragraph(doc, spans, start, end, report, None, false, false)?; // 先写段落
            let range = doc_range(doc, start, next)?; // 范围

            let format = get(&range, "ParagraphFormat")?.as_dispatch()?; // 段落格式
            if set(&format, "LeftIndent", Variant::R8(CM_TO_POINTS)).is_err() { report.warn(Warning::new("PARTIAL_APPLY", "引用缩进设置失败")); } // 缩进
            let font = get(&range, "Font")?.as_dispatch()?; // 字体
            if set(&font, "Color", Variant::I4(0x808080)).is_err() { report.warn(Warning::new("PARTIAL_APPLY", "引用颜色设置失败")); } // 灰色
            Ok(next) // 返回
        }
        Block::Code(text) => render_code(doc, text, start, end, report), // 代码块
        Block::Table(rows) => render_table(doc, rows, start, end), // 表格
        Block::Rule => { // 分割线
            let range = doc_range(doc, start, end)?; // 范围
            set(&range, "Text", Variant::Str("\r".to_string()))?; // 空段落
            let next = get(&range, "End")?.as_i32(); // 新位置
            let borders = get(&range, "Borders")?.as_dispatch()?; // 边框集合
            if let Ok(border) = call(&borders, "Item", &[Variant::I4(WD_BORDER_BOTTOM)]).and_then(|value| value.as_dispatch()) { // 下边框
                if set(&border, "LineStyle", Variant::I4(1)).is_err() { report.warn(Warning::new("PARTIAL_APPLY", "分割线绘制失败")); } // 单实线
            }
            Ok(next) // 返回
        }
        Block::PageBreak => { // 分页符
            let range = doc_range(doc, start, end)?; // 范围
            call(&range, "InsertBreak", &[Variant::I4(WD_PAGE_BREAK)])?; // 插入分页
            Ok(get(&range, "End")?.as_i32()) // 新位置
        }
        Block::Image(path) => { // 图片
            let full = Paths::load().resolve(path)?; // 校验路径
            let range = doc_range(doc, start, end)?; // 锚点
            let shapes = get(doc, "InlineShapes")?.as_dispatch()?; // 图片集合
            let args = [Variant::Str(full.to_string_lossy().to_string()), Variant::Bool(false), Variant::Bool(true), Variant::Dispatch(range)]; // 参数
            call(&shapes, "AddPicture", &args)?; // 插入
            Ok(get(doc, "Content")?.as_dispatch().and_then(|content| get(&content, "End")).map(|value| value.as_i32()).unwrap_or(start)) // 文末
        }
        Block::Blank => Ok(start), // 不渲染
    }
}
fn render_paragraph(doc: &ComObj, spans: &[Span], start: i32, end: i32, report: &mut Report, heading: Option<usize>, bullet: bool, numbered: bool) -> ComResult<i32> { // 段落/标题/列表
    let text = spans_text(spans); // 纯文本
    let mut prefix = String::new(); // 列表回退前缀
    let mut list_ok = None; // 真列表范围
    if bullet || numbered { // 列表
        let range = doc_range(doc, start, end)?; // 范围
        set(&range, "Text", Variant::Str(format!("{text}\r")))?; // 先写内容
        if apply_list(doc, &range, numbered).is_ok() { list_ok = Some(range); } // 应用列表模板
        else { // 回退
            prefix = if bullet { "• ".to_string() } else { "1. ".to_string() }; // 前缀
            set(&range, "Text", Variant::Str(format!("{prefix}{text}\r")))?; // 重写
            report.warn(Warning::new("STYLE_FALLBACK", if bullet { "无序列表回退为符号前缀" } else { "有序列表回退为数字前缀" })); // 警告
        }
    }
    let range = match list_ok { Some(range) => range, None => { let range = doc_range(doc, start, end)?; set(&range, "Text", Variant::Str(format!("{prefix}{text}\r")))?; range } }; // 非列表写入
    let next = get(&range, "End")?.as_i32(); // 新位置
    let base = start + utf16_len(&prefix); // 行内起点
    let mut offset = base; // 当前字符位置
    for span in spans { // 逐片段加样式
        apply_span(doc, span, offset, report)?; // 应用样式
        offset += utf16_len(&span.text); // 推进
    }
    if let Some(level) = heading { // 标题样式
        let style = format!("Heading {level}"); // 英文样式名
        if set(&range, "Style", Variant::Str(style)).is_err() { // 回退中文
            if set(&range, "Style", Variant::Str(format!("标题 {level}"))).is_err() { report.warn(Warning::new("STYLE_FALLBACK", format!("标题 {level} 样式设置失败"))); } // 警告
        }
    }
    Ok(next) // 返回
}
fn apply_list(doc: &ComObj, range: &ComObj, numbered: bool) -> ComResult<()> { // 应用内置列表模板
    let app = get(doc, "Application")?.as_dispatch()?; // Application
    let galleries = get(&app, "ListGalleries")?.as_dispatch()?; // 列表库
    let gallery = call(&galleries, "Item", &[Variant::I4(if numbered { 2 } else { 1 })])?.as_dispatch()?; // 项目符号=1/编号=2
    let templates = get(&gallery, "ListTemplates")?.as_dispatch()?; // 模板集
    let template = call(&templates, "Item", &[Variant::I4(1)])?.as_dispatch()?; // 第一个模板
    let format = get(range, "ListFormat")?.as_dispatch()?; // ListFormat
    let args = [Variant::Dispatch(template), Variant::Bool(false), Variant::I4(0), Variant::I4(0)]; // 模板/继续编号/整表/默认行为
    call(&format, "ApplyListTemplate", &args)?; // 应用
    Ok(()) // 完成
}
fn apply_span(doc: &ComObj, span: &Span, offset: i32, report: &mut Report) -> ComResult<()> { // 行内样式
    let end = offset + utf16_len(&span.text); // 片段终点
    if end <= offset { return Ok(()); } // 空片段
    let range = doc_range(doc, offset, end)?; // 片段范围
    let font = get(&range, "Font")?.as_dispatch()?; // 字体
    if span.bold && set(&font, "Bold", Variant::I4(-1)).is_err() { report.warn(Warning::new("PARTIAL_APPLY", "粗体设置失败")); } // 粗体
    if span.italic && set(&font, "Italic", Variant::I4(-1)).is_err() { report.warn(Warning::new("PARTIAL_APPLY", "斜体设置失败")); } // 斜体
    if span.strike && set(&font, "StrikeThrough", Variant::I4(-1)).is_err() { report.warn(Warning::new("PARTIAL_APPLY", "删除线设置失败")); } // 删除线
    if span.code { // 行内代码
        if set(&font, "Name", Variant::Str("Consolas".to_string())).is_err() { report.warn(Warning::new("PARTIAL_APPLY", "等宽字体设置失败")); } // 等宽
        let shading = get(&range, "Shading")?.as_dispatch()?; // 底纹
        if set(&shading, "BackgroundPatternColor", Variant::I4(0xF2F2F2)).is_err() { report.warn(Warning::new("PARTIAL_APPLY", "行内代码底纹设置失败")); } // 浅灰 BGR
    }
    if let Some(url) = &span.link { // 超链接
        let links = get(doc, "Hyperlinks")?.as_dispatch()?; // 集合
        let args = [Variant::Dispatch(range), Variant::Str(url.clone()), Variant::Str(String::new()), Variant::Str(String::new()), Variant::Str(span.text.clone())]; // Anchor/Address/Sub/ScreenTip/Text
        if call(&links, "Add", &args).is_err() { report.warn(Warning::new("PARTIAL_APPLY", format!("链接 {url} 创建失败"))); } // 警告
    }
    Ok(()) // 完成
}
fn render_code(doc: &ComObj, text: &str, start: i32, end: i32, report: &mut Report) -> ComResult<i32> { // 代码块
    let range = doc_range(doc, start, end)?; // 范围
    let body = if text.ends_with('\n') { text.to_string() } else { format!("{text}\n") }; // 补齐换行
    set(&range, "Text", Variant::Str(body.replace('\n', "\r")))?; // 写入
    let next = get(&range, "End")?.as_i32(); // 新位置
    let font = get(&range, "Font")?.as_dispatch()?; // 字体
    if set(&font, "Name", Variant::Str("Consolas".to_string())).is_err() { report.warn(Warning::new("PARTIAL_APPLY", "代码块等宽字体设置失败")); } // 等宽
    let shading = get(&range, "Shading")?.as_dispatch()?; // 底纹
    if set(&shading, "BackgroundPatternColor", Variant::I4(0xF2F2F2)).is_err() { report.warn(Warning::new("PARTIAL_APPLY", "代码块底纹设置失败")); } // 浅灰
    Ok(next) // 返回
}
fn render_table(doc: &ComObj, rows: &[Vec<String>], start: i32, end: i32) -> ComResult<i32> { // 表格
    if rows.is_empty() { return Ok(start); } // 空表
    let cols = rows.iter().map(|row| row.len()).max().unwrap_or(1).max(1); // 列数
    let anchor = doc_range(doc, start, end)?; // 锚点
    let tables = get(doc, "Tables")?.as_dispatch()?; // 集合
    let args = [Variant::Dispatch(anchor), Variant::I4(rows.len() as i32), Variant::I4(cols as i32)]; // Range/Rows/Columns
    let table = call(&tables, "Add", &args)?.as_dispatch()?; // 添加表格
    for (row_index, row) in rows.iter().enumerate() { // 逐行
        for (col_index, cell) in row.iter().enumerate() { // 逐格
            if let Ok(cell_obj) = call(&table, "Cell", &[Variant::I4(row_index as i32 + 1), Variant::I4(col_index as i32 + 1)]) { // 单元格
                if let Ok(cell_range) = get(&cell_obj.as_dispatch()?, "Range").and_then(|value| value.as_dispatch()) { let _ = set(&cell_range, "Text", Variant::Str(strip_inline(cell))); } // 文本
            }
        }
    }
    if let Ok(first_row) = call(&table, "Rows", &[]).and_then(|value| value.as_dispatch()).and_then(|rows_obj| call(&rows_obj, "Item", &[Variant::I4(1)])).and_then(|value| value.as_dispatch()) { // 表头
        if let Ok(row_range) = get(&first_row, "Range").and_then(|value| value.as_dispatch()) { // 行范围
            if let Ok(font) = get(&row_range, "Font").and_then(|value| value.as_dispatch()) { let _ = set(&font, "Bold", Variant::I4(-1)); } // 加粗
        }
    }
    let table_range = get(&table, "Range")?.as_dispatch()?; // 表格范围
    let table_end = get(&table_range, "End")?.as_i32(); // 表格末尾
    Ok(table_end + 1) // 移到表后
}
fn doc_range(doc: &ComObj, start: i32, end: i32) -> ComResult<ComObj> { // 构造 Range（用 Content 偏移属性，避免 Document.Range 异常）
    let range = get(doc, "Content")?.as_dispatch()?; // 正文 Range 对象
    let max_end = get(&range, "End")?.as_i32().max(0); // 文档末尾位置
    let start = start.clamp(0, max_end); // 允许文档起始 0
    let end = end.clamp(start, max_end); // 保证 end >= start
    set(&range, "Start", Variant::I4(start))?; // 起点
    set(&range, "End", Variant::I4(end))?; // 终点
    Ok(range) // 返回折叠/选区范围
}
fn block_chars(block: &Block) -> usize { // 块字符数
    match block { // 分派
        Block::Heading(_, spans) | Block::Paragraph(spans) | Block::Bullet(spans) | Block::Numbered(spans) | Block::Quote(spans) => spans.iter().map(|span| span.text.chars().count()).sum(), // 行内
        Block::Code(text) => text.chars().count(), // 代码
        Block::Table(rows) => rows.iter().flat_map(|row| row.iter()).map(|cell| cell.chars().count()).sum(), // 表格
        _ => 0, // 其它
    }
}
fn spans_text(spans: &[Span]) -> String { spans.iter().map(|span| span.text.as_str()).collect() } // 拼接纯文本
fn utf16_len(text: &str) -> i32 { text.encode_utf16().count() as i32 } // Word 字符位置按 UTF-16
fn strip_inline(text: &str) -> String { spans_text(&parse_inline(text)) } // 去掉行内标记
fn parse(markdown: &str) -> Vec<Block> { // 解析 Markdown
    let lines: Vec<&str> = markdown.lines().collect(); // 行
    let mut blocks = Vec::new(); // 结果
    let mut index = 0usize; // 游标
    while index < lines.len() { // 扫描
        let line = lines[index]; // 当前行
        let trimmed = line.trim(); // 去空白
        if trimmed.starts_with("```") { // 代码块
            let mut code_lines = Vec::new(); // 缓冲
            index += 1; // 跳过围栏
            while index < lines.len() && !lines[index].trim().starts_with("```") { code_lines.push(lines[index]); index += 1; } // 收集
            blocks.push(Block::Code(code_lines.join("\n"))); // 收集块
            index += 1; // 跳过结束围栏
            continue; // 下一行
        }
        if trimmed.is_empty() { blocks.push(Block::Blank); index += 1; continue; } // 空行
        if trimmed == "\\pagebreak" || trimmed == "<!-- pagebreak -->" { blocks.push(Block::PageBreak); index += 1; continue; } // 分页
        if trimmed == "---" || trimmed == "***" || trimmed == "___" { blocks.push(Block::Rule); index += 1; continue; } // 分割线
        if let Some(path) = image_path(trimmed) { blocks.push(Block::Image(path)); index += 1; continue; } // 图片
        if trimmed.starts_with('#') { // 标题
            let level = trimmed.chars().take_while(|ch| *ch == '#').count().min(6); // 级别
            blocks.push(Block::Heading(level, parse_inline(trimmed[level..].trim()))); // 收集
            index += 1; continue; // 下一行
        }
        if trimmed.starts_with(|ch: char| ch == '|') && trimmed.ends_with('|') { // 表格
            let mut rows = Vec::new(); // 行
            while index < lines.len() { // 连续表格
                let row = lines[index].trim(); // 行
                if !row.starts_with('|') || !row.ends_with('|') { break; } // 结束
                let cells: Vec<String> = row.trim_matches('|').split('|').map(|cell| cell.trim().to_string()).collect(); // 单元格
                if !is_separator_row(&cells) { rows.push(cells); } // 跳过分隔行
                index += 1; // 下一行
            }
            blocks.push(Block::Table(rows)); // 收集
            continue; // 已推进
        }
        if trimmed.starts_with("- ") || trimmed.starts_with("* ") || trimmed.starts_with("+ ") { blocks.push(Block::Bullet(parse_inline(trimmed[2..].trim()))); index += 1; continue; } // 无序列表
        if let Some(text) = numbered_text(trimmed) { blocks.push(Block::Numbered(parse_inline(text))); index += 1; continue; } // 有序列表
        if trimmed.starts_with('>') { blocks.push(Block::Quote(parse_inline(trimmed[1..].trim()))); index += 1; continue; } // 引用
        blocks.push(Block::Paragraph(parse_inline(trimmed))); // 段落
        index += 1; // 下一行
    }
    blocks // 返回
}
fn is_separator_row(cells: &[String]) -> bool { // 表格分隔行
    !cells.is_empty() && cells.iter().all(|cell| !cell.is_empty() && cell.chars().all(|ch| ch == '-' || ch == ':' || ch == ' ')) // 仅分隔符
}
fn image_path(text: &str) -> Option<String> { // 解析整行图片
    let rest = text.strip_prefix("![")?; // 前缀
    let close = rest.find(']')?; // alt 结束
    let tail = rest[close + 1..].trim(); // 余下
    let url = tail.strip_prefix('(')?.strip_suffix(')')?.trim(); // 地址
    if url.is_empty() { None } else { Some(url.to_string()) } // 返回
}
fn numbered_text(text: &str) -> Option<&str> { // 有序列表文本
    let digits = text.chars().take_while(|ch| ch.is_ascii_digit()).count(); // 数字
    if digits == 0 { return None; } // 无数字
    text[digits..].strip_prefix(". ").map(|rest| rest.trim()) // 去序号
}
fn parse_inline(text: &str) -> Vec<Span> { // 解析行内标记
    let mut spans = Vec::new(); // 结果
    let chars: Vec<char> = text.chars().collect(); // 字符
    let mut buffer = String::new(); // 普通文本
    let mut index = 0usize; // 游标
    while index < chars.len() { // 扫描
        let ch = chars[index]; // 当前字符
        if ch == '\\' && index + 1 < chars.len() { buffer.push(chars[index + 1]); index += 2; continue; } // 转义
        if ch == '`' { // 行内代码
            if let Some(end) = find_char(&chars, index + 1, '`') { // 结束
                flush_span(&mut spans, &mut buffer, Span::default()); // 提交普通文本
                spans.push(Span { text: chars[index + 1..end].iter().collect(), code: true, ..Span::default() }); // 代码片段
                index = end + 1; continue; // 推进
            }
        }
        if ch == '*' && index + 1 < chars.len() && chars[index + 1] == '*' { // 粗体
            if let Some(end) = find_seq(&chars, index + 2, "**") { // 结束
                flush_span(&mut spans, &mut buffer, Span::default()); // 提交
                let mut inner = parse_inline(&chars[index + 2..end].iter().collect::<String>()); // 内部
                for span in &mut inner { span.bold = true; } // 加粗
                spans.extend(inner); index = end + 2; continue; // 推进
            }
        }
        if ch == '~' && index + 1 < chars.len() && chars[index + 1] == '~' { // 删除线
            if let Some(end) = find_seq(&chars, index + 2, "~~") { // 结束
                flush_span(&mut spans, &mut buffer, Span::default()); // 提交
                let mut inner = parse_inline(&chars[index + 2..end].iter().collect::<String>()); // 内部
                for span in &mut inner { span.strike = true; } // 删除线
                spans.extend(inner); index = end + 2; continue; // 推进
            }
        }
        if ch == '*' { // 斜体
            if let Some(end) = find_char(&chars, index + 1, '*') { // 结束
                flush_span(&mut spans, &mut buffer, Span::default()); // 提交
                let mut inner = parse_inline(&chars[index + 1..end].iter().collect::<String>()); // 内部
                for span in &mut inner { span.italic = true; } // 斜体
                spans.extend(inner); index = end + 1; continue; // 推进
            }
        }
        if ch == '[' { // 链接
            if let Some((label, url, next)) = parse_link(&chars, index) { // 解析
                flush_span(&mut spans, &mut buffer, Span::default()); // 提交
                spans.push(Span { text: label, link: Some(url), ..Span::default() }); // 链接片段
                index = next; continue; // 推进
            }
        }
        buffer.push(ch); // 普通字符
        index += 1; // 下一字符
    }
    flush_span(&mut spans, &mut buffer, Span::default()); // 收尾
    spans // 返回
}
fn flush_span(spans: &mut Vec<Span>, buffer: &mut String, style: Span) { // 提交普通文本
    if buffer.is_empty() { return; } // 空
    spans.push(Span { text: std::mem::take(buffer), ..style }); // 收集
}
fn find_char(chars: &[char], start: usize, target: char) -> Option<usize> { // 找单字符
    (start..chars.len()).find(|&i| chars[i] == target) // 查找
}
fn find_seq(chars: &[char], start: usize, target: &str) -> Option<usize> { // 找双字符标记
    let target: Vec<char> = target.chars().collect(); // 目标
    if target.is_empty() { return None; } // 空
    (start..chars.len()).find(|&i| i + target.len() <= chars.len() && chars[i..i + target.len()] == target[..]) // 查找
}
fn parse_link(chars: &[char], start: usize) -> Option<(String, String, usize)> { // 解析 [text](url)
    let close = find_char(chars, start + 1, ']')?; // 文本结束
    if close + 1 >= chars.len() || chars[close + 1] != '(' { return None; } // 需要 (
    let end = find_char(chars, close + 2, ')')?; // 地址结束
    let label: String = chars[start + 1..close].iter().collect(); // 文本
    let url: String = chars[close + 2..end].iter().collect(); // 地址
    Some((label, url, end + 1)) // 返回
}
#[cfg(test)] // 单元测试
mod tests { // 测试模块
    use super::*; // 引入
    #[test] // 块解析
    fn parses_blocks() { // 标题/列表/表格/代码
        let blocks = parse("# 标题\n\n- 项目\n\n| a | b |\n|---|---|\n| 1 | 2 |\n\n```\ncode\n```"); // 输入
        assert!(matches!(blocks.first(), Some(Block::Heading(1, _)))); // 标题
        assert!(blocks.iter().any(|block| matches!(block, Block::Bullet(_)))); // 列表
        assert!(blocks.iter().any(|block| matches!(block, Block::Table(rows) if rows.len() == 2))); // 表格
        assert!(blocks.iter().any(|block| matches!(block, Block::Code(_)))); // 代码
    }
    #[test] // 行内解析
    fn parses_inline() { // 粗体/代码/链接
        let spans = parse_inline("普通**粗体**和`代码`与[链接](https://example.com)"); // 输入
        assert!(spans.iter().any(|span| span.bold && span.text == "粗体")); // 粗体
        assert!(spans.iter().any(|span| span.code && span.text == "代码")); // 代码
        assert!(spans.iter().any(|span| span.link.as_deref() == Some("https://example.com") && span.text == "链接")); // 链接
    }
    #[test] // 纯文本
    fn strips_marks() { // 去标记
        assert_eq!(strip_inline("**粗体**与`代码`"), "粗体与代码"); // 断言
    }
}