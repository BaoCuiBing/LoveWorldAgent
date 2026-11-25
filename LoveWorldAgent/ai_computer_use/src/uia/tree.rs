use anyhow::Result; // 错误类型
use serde::Serialize; // 序列化派生
use crate::uia::element::{element_info, ElementInfo}; // 元素属性
use crate::uia::worker::UiaWorker; // UIA 工作器
use windows::Win32::UI::Accessibility::{IUIAutomationElement, TreeScope_Children}; // UIA 类型
#[derive(Debug, Clone, Serialize)] // 树节点
pub struct TreeNode { // 元素树节点
    #[serde(flatten)] // 展开元素属性
    pub info: ElementInfo, // 节点元素信息
    pub children: Vec<TreeNode>, // 子节点
}
#[derive(Debug, Clone, Serialize)] // 分页树结果
pub struct TreePage { // 元素树一页
    pub root: TreeNode, // 根节点
    pub total_children: usize, // 根节点直接子元素总数
    pub page: usize, // 当前页码
    pub page_size: usize, // 每页数量
    pub has_more: bool, // 是否还有下一页
}
pub fn build_page(worker: &mut UiaWorker, element: &IUIAutomationElement, max_depth: usize, page: usize, page_size: usize) -> Result<TreePage> { // 构建元素树分页
    let page = page.max(1); // 页码至少为 1
    let page_size = page_size.max(1); // 每页至少 1 个
    let root_id = worker.store(element); // 缓存根元素
    let condition = unsafe { worker.automation().CreateTrueCondition()? }; // 全量条件
    let children = unsafe { element.FindAll(TreeScope_Children, &condition)? }; // 直接子元素
    let total = unsafe { children.Length()? }.max(0) as usize; // 子元素总数
    let start = (page - 1).saturating_mul(page_size); // 起始下标
    let end = (start + page_size).min(total); // 结束下标
    let mut nodes = Vec::new(); // 子节点容器
    for index in start..end { // 遍历当前页
        let Ok(child) = (unsafe { children.GetElement(index as i32) }) else { continue; }; // 跳过读取失败
        nodes.push(build_node(worker, &child, max_depth.saturating_sub(1))?); // 递归构建
    }
    Ok(TreePage { root: TreeNode { info: element_info(root_id, element), children: nodes }, total_children: total, page, page_size, has_more: end < total }) // 返回分页结果
}
fn build_node(worker: &mut UiaWorker, element: &IUIAutomationElement, depth: usize) -> Result<TreeNode> { // 递归构建节点
    let element_id = worker.store(element); // 缓存元素
    let mut children = Vec::new(); // 子节点容器
    if depth > 0 { // 未到最大深度
        let condition = unsafe { worker.automation().CreateTrueCondition()? }; // 全量条件
        let found = unsafe { element.FindAll(TreeScope_Children, &condition)? }; // 直接子元素
        let total = unsafe { found.Length()? }.max(0); // 子元素总数
        for index in 0..total { // 遍历子元素
            let Ok(child) = (unsafe { found.GetElement(index) }) else { continue; }; // 跳过读取失败
            children.push(build_node(worker, &child, depth - 1)?); // 递归构建
        }
    }
    Ok(TreeNode { info: element_info(element_id, element), children }) // 返回节点
}
