use anyhow::Result; // 错误类型
use std::collections::{HashMap, HashSet}; // 差分索引
use std::time::Instant; // 截止时间
use windows::Win32::UI::Accessibility::{IUIAutomationElement, IUIAutomationTextPattern, IUIAutomationValuePattern, TreeScope_Children, TreeScope_Descendants, UIA_TextPatternId, UIA_ValuePatternId}; // UIA 类型
use crate::uia::element::{element_actions, element_info, element_traits, element_value, role_name}; // 元素属性与内置树字段
use crate::uia::worker::UiaWorker; // UIA 工作器
pub const MAX_TREE_CHARS: usize = 200_000; // 树文本上限
#[derive(Clone, Debug, PartialEq)] // 快照节点（用于差分）
pub struct Node { // 内置树节点
    pub id: i64, // element_id
    pub role: String, // 内置 role
    pub name: String, // 可见文本或名称
    pub traits: Vec<String>, // focused/disabled/selected/expanded 等
    pub actions: Vec<String>, // 内置 6 类动作
    pub value: Option<String>, // val= 值
    pub children: Vec<Node>, // 子节点
}
pub struct Snapshot { // 快照结果
    pub text: String, // 完整树文本
    pub nodes: usize, // 实际节点数
    pub root: Node, // 结构化树（供 diff）
}
pub fn snapshot(worker: &mut UiaWorker, root: &IUIAutomationElement, max_depth: usize, max_nodes: usize, deadline: Instant) -> Result<Snapshot> { // 生成内置格式元素树（带时间与长度上限）
    let mut nodes = 0usize; // 节点计数
    let mut truncated = false; // 是否截断
    let root_node = walk(worker, root, 0, max_depth, max_nodes.max(1), deadline, &mut nodes, &mut truncated)?; // 递归构建
    let mut text = render(&root_node); // 渲染完整树
    if truncated { text.push_str("\n... 已截断（达到时间或节点上限）"); } // 截断标记
    if text.len() > MAX_TREE_CHARS { // 字符上限保护
        let mut cut = MAX_TREE_CHARS; // 初始截断点
        while cut > 0 && !text.is_char_boundary(cut) { cut -= 1; } // 回退到 UTF-8 边界
        text.truncate(cut); // 安全截断
        text.push_str("\n... 已截断（达到长度上限）"); // 长度标记
    }
    Ok(Snapshot { text, nodes, root: root_node }) // 返回结果
}
fn walk(worker: &mut UiaWorker, element: &IUIAutomationElement, depth: usize, max_depth: usize, max_nodes: usize, deadline: Instant, nodes: &mut usize, truncated: &mut bool) -> Result<Node> { // 深度优先遍历
    if *nodes >= max_nodes { *truncated = true; return Err(anyhow::anyhow!("达到节点上限")); } // 达到节点上限
    if Instant::now() >= deadline { *truncated = true; return Err(anyhow::anyhow!("达到时间上限")); } // 达到时间上限
    let element_id = worker.store(element); // 缓存元素并分配稳定 ID
    let info = element_info(element_id, element); // 读取属性
    let node = Node { // 构造节点
        id: element_id, // 稳定 ID
        role: role_name(info.control_type_id).to_string(), // 内置 role
        name: info.name.clone(), // 标签
        traits: element_traits(element), // 状态
        actions: element_actions(element), // 6 类动作
        value: element_value(element), // 当前值
        children: Vec::new(), // 子节点后填
    };
    *nodes += 1; // 节点计数
    if depth >= max_depth { return Ok(node); } // 达到深度上限
    let condition = unsafe { worker.automation().CreateTrueCondition()? }; // 全量条件
    let children = unsafe { element.FindAll(TreeScope_Children, &condition)? }; // 直接子元素
    let total = unsafe { children.Length()? }.max(0); // 子元素数量
    let mut result = node; // 可变节点
    for index in 0..total { // 遍历子元素
        if *nodes >= max_nodes { *truncated = true; break; } // 达到节点上限
        if Instant::now() >= deadline { *truncated = true; break; } // 达到时间上限
        let Ok(child) = (unsafe { children.GetElement(index) }) else { continue; }; // 跳过读取失败
        match walk(worker, &child, depth + 1, max_depth, max_nodes, deadline, nodes, truncated) { Ok(child_node) => result.children.push(child_node), Err(_) => break } // 递归；上限错误时停止
    }
    Ok(result) // 返回节点
}
pub fn render(root: &Node) -> String { // 渲染完整树
    let mut lines: Vec<String> = Vec::new(); // 行缓冲
    render_into(root, 0, &mut lines); // 递归渲染
    lines.join("\n") // 拼接
}
fn render_into(node: &Node, depth: usize, lines: &mut Vec<String>) { // 递归渲染节点
    lines.push(node_line(node, depth)); // 当前节点
    for child in &node.children { render_into(child, depth + 1, lines); } // 子节点
}
fn node_line(node: &Node, depth: usize) -> String { // 生成内置节点行
    let pad = "  ".repeat(depth); // 缩进
    let traits = if node.traits.is_empty() { "()".to_string() } else { format!("({})", node.traits.join(",")) }; // 状态
    let actions = if node.actions.is_empty() { "[]".to_string() } else { format!("[{}]", node.actions.join(",")) }; // 动作
    let value = node.value.as_ref().map(|text| format!(" val={}", escape(text))).unwrap_or_default(); // 值
    format!("{pad}{} \"{}\" {} {}{} id={}", node.role, escape(&node.name), traits, actions, value, node.id) // 节点行
}
fn escape(text: &str) -> String { // 转义标签与值
    text.replace('\\', "\\\\").replace('"', "\\\"").replace(['\r', '\n'], " ") // 保留单行
}
fn fields_changed(old: &Node, new: &Node) -> bool { // 判断节点自身字段是否变化
    old.role != new.role || old.name != new.name || old.traits != new.traits || old.actions != new.actions || old.value != new.value // 不比较 children
}
pub fn diff(old: &Node, new: &Node) -> Option<String> { // 生成内置增量文本；根身份变化返回 None
    if old.id != new.id { return None; } // 根节点不同，调用方应返回全量
    let mut lines: Vec<String> = Vec::new(); // 差分行
    diff_node(old, new, 0, 0, &mut lines); // 递归差分
    if lines.is_empty() { None } else { Some(lines.join("\n")) } // 无变化返回 None
}
fn diff_node(old: &Node, new: &Node, parent_id: i64, child_index: usize, lines: &mut Vec<String>) { // 递归差分
    if fields_changed(old, new) { // 节点字段变化
        lines.push(format!("update parent_id={} child_index={}", parent_id, child_index)); // update 头
        lines.push(node_line(new, 0)); // 更新后的节点行
    }
    let old_map: HashMap<i64, &Node> = old.children.iter().map(|child| (child.id, child)).collect(); // 旧子节点索引
    let new_ids: HashSet<i64> = new.children.iter().map(|child| child.id).collect(); // 新子节点 ID
    for (index, child) in new.children.iter().enumerate() { // 按新顺序遍历
        match old_map.get(&child.id) { // 按稳定 ID 匹配
            Some(old_child) => diff_node(old_child, child, new.id, index, lines), // 递归
            None => { // 新增子树
                lines.push(format!("insert parent_id={} child_index={}", new.id, index)); // insert 头
                render_into(child, 1, lines); // 子树行
            }
        }
    }
    for child in &old.children { // 删除的旧子节点
        if !new_ids.contains(&child.id) { lines.push(format!("remove id={}", child.id)); } // remove 行
    }
}
pub fn scrape(worker: &mut UiaWorker, root: &IUIAutomationElement, max_nodes: usize, deadline: Instant) -> Result<String> { // 抓取窗口可见文本（带截止时间）
    let condition = unsafe { worker.automation().CreateTrueCondition()? }; // 全量条件
    let elements = unsafe { root.FindAll(TreeScope_Descendants, &condition)? }; // 全部后代
    let total = unsafe { elements.Length()? }.max(0) as usize; // 总数
    let mut texts: Vec<String> = Vec::new(); // 文本缓冲
    for index in 0..total.min(max_nodes) { // 限制扫描数量
        if index % 32 == 0 && Instant::now() >= deadline { break; } // 超时退出
        let Ok(element) = (unsafe { elements.GetElement(index as i32) }) else { continue; }; // 跳过读取失败
        let info = element_info(0, &element); // 读取属性
        let mut body = String::new(); // 控件正文（Value/TextPattern）
        if let Ok(pattern) = unsafe { element.GetCurrentPatternAs::<IUIAutomationValuePattern>(UIA_ValuePatternId) } { body = unsafe { pattern.CurrentValue() }.map(|value| value.to_string()).unwrap_or_default().trim().to_string(); } // 读取 Value
        if body.is_empty() { // Value 为空时尝试 TextPattern（记事本等文档控件）
            if let Ok(pattern) = unsafe { element.GetCurrentPatternAs::<IUIAutomationTextPattern>(UIA_TextPatternId) } { if let Ok(range) = unsafe { pattern.DocumentRange() } { body = unsafe { range.GetText(-1) }.map(|value| value.to_string()).unwrap_or_default().trim().to_string(); } } // 读取文档文本
        }
        let text = if body.is_empty() { info.name.trim().to_string() } else { body }; // 正文优先，其次 Name
        if text.is_empty() { continue; } // 空文本跳过
        if info.is_offscreen && text == info.name.trim() { continue; } // 离屏且只有标签名时跳过
        if texts.last() == Some(&text) { continue; } // 过滤相邻重复
        texts.push(text); // 收集文本
    }
    Ok(texts.join("\n")) // 以换行拼接
}
