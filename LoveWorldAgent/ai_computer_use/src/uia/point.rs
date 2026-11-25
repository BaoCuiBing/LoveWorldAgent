use anyhow::Result; // 错误类型
use crate::uia::element::{element_info, ElementInfo}; // 元素属性
use crate::uia::worker::UiaWorker; // UIA 工作器
use windows::Win32::Foundation::POINT; // 屏幕坐标
#[allow(dead_code)] // P1 将暴露 list_elements_at_point 工具
pub fn elements_at_point(worker: &mut UiaWorker, x: i32, y: i32, max_depth: usize, include_offscreen: bool) -> Result<Vec<ElementInfo>> { // 坐标点元素链路
    let element = unsafe { worker.automation().ElementFromPoint(POINT { x, y })? }; // 取坐标点元素
    let walker = unsafe { worker.automation().ControlViewWalker()? }; // 控件视图遍历器
    let mut chain = Vec::new(); // 元素链
    let mut current = Some(element); // 当前元素
    while let Some(node) = current { // 向上回溯
        if chain.len() >= max_depth.max(1) { break; } // 达到深度上限
        let element_id = worker.store(&node); // 缓存元素
        let info = element_info(element_id, &node); // 读取属性
        if include_offscreen || !info.is_offscreen { chain.push(info); } // 按离屏开关收集
        current = unsafe { walker.GetParentElement(&node).ok() }; // 父元素，根节点返回 None
    }
    chain.reverse(); // 根到叶排序
    Ok(chain) // 返回链路
}
