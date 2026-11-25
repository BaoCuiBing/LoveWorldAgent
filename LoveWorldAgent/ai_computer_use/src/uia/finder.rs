use anyhow::{anyhow, Result}; // 错误类型
use std::time::{Duration, Instant}; // 截止时间
use windows::Win32::System::Variant::VARIANT; // 条件值
use windows::Win32::UI::Accessibility::{IUIAutomationCondition, IUIAutomationElement, PropertyConditionFlags, PropertyConditionFlags_IgnoreCase, PropertyConditionFlags_MatchSubstring, TreeScope_Descendants, UIA_AutomationIdPropertyId, UIA_NamePropertyId}; // UIA 类型与条件
use crate::uia::desktop::{list_windows, WindowFilter}; // 顶层窗口过滤
use crate::uia::element::{element_info, ElementInfo}; // 元素属性
use crate::uia::worker::UiaWorker; // UIA 工作器
#[derive(Debug, Clone, Default)] // 查找参数
pub struct FindParams { // 对应 find_element 参数
    pub window_handle: Option<i64>, // 直接指定窗口句柄
    pub window_name: Option<String>, // 窗口标题模糊匹配
    pub window_class: Option<String>, // 窗口类名
    pub process_name: Option<String>, // 进程名
    pub automation_id: Option<String>, // AutomationId 精确匹配
    pub name: Option<String>, // Name 模糊匹配
    pub class_name: Option<String>, // ClassName 精确匹配
    pub control_type: Option<String>, // ControlType 名称
    pub max_results: usize, // 最大返回数量
    pub max_scan: usize, // 宽扫描最多元素数
}
pub fn find_with_retry(worker: &mut UiaWorker, params: &FindParams, timeout_ms: u64, interval_ms: u64) -> Result<Vec<ElementInfo>> { // 条件查找 + 轮询，整体受 deadline 约束
    let deadline = Instant::now() + Duration::from_millis(timeout_ms.max(1)); // 总截止时刻
    loop { // 轮询
        let results = find(worker, params, deadline)?; // 单轮查找（内部逐批检查 deadline）
        if !results.is_empty() { return Ok(results); } // 命中即返回
        if Instant::now() >= deadline { return Ok(Vec::new()); } // 总超时返回空
        worker.wait(interval_ms.max(1)); // 等待下一个轮询周期
    }
}
pub fn find(worker: &mut UiaWorker, params: &FindParams, deadline: Instant) -> Result<Vec<ElementInfo>> { // 单轮查找：原生条件优先，宽扫描兜底
    let root = resolve_root(worker, params)?; // 解析窗口根元素
    let limit = params.max_results.max(1); // 结果上限
    let mut results: Vec<ElementInfo> = Vec::new(); // 结果容器
    if let Some(condition) = build_condition(worker, params)? { // 可构造原生条件
        scan_condition(worker, &root, params, &condition, limit, deadline, &mut results)?; // 条件扫描
    }
    if results.is_empty() && Instant::now() < deadline { // 条件未命中且仍有余量
        scan_broad(worker, &root, params, limit, deadline, &mut results)?; // 有界宽扫描
    }
    Ok(results) // 返回命中元素
}
fn build_condition(worker: &UiaWorker, params: &FindParams) -> Result<Option<IUIAutomationCondition>> { // 构造 UIA 原生条件
    let mut conditions: Vec<IUIAutomationCondition> = Vec::new(); // 条件列表
    if let Some(name) = params.name.as_ref().filter(|text| !text.trim().is_empty()) { // 名称模糊匹配
        let value = VARIANT::from(name.trim()); // 字符串变体
        let condition = unsafe { worker.automation().CreatePropertyConditionEx(UIA_NamePropertyId, &value, PropertyConditionFlags(PropertyConditionFlags_MatchSubstring.0 | PropertyConditionFlags_IgnoreCase.0))? }; // 子串 + 忽略大小写
        conditions.push(condition); // 收集
    }
    if let Some(automation_id) = params.automation_id.as_ref().filter(|text| !text.trim().is_empty()) { // AutomationId 精确匹配
        let value = VARIANT::from(automation_id.trim()); // 字符串变体
        let condition = unsafe { worker.automation().CreatePropertyCondition(UIA_AutomationIdPropertyId, &value)? }; // 精确条件
        conditions.push(condition); // 收集
    }
    match conditions.len() { // 组合条件
        0 => Ok(None), // 无条件
        1 => Ok(Some(conditions.remove(0))), // 单条件
        _ => { let first = conditions.remove(0); let second = conditions.remove(0); Ok(Some(unsafe { worker.automation().CreateAndCondition(&first, &second)? })) } // 与条件
    }
}
fn scan_condition(worker: &mut UiaWorker, root: &IUIAutomationElement, params: &FindParams, condition: &IUIAutomationCondition, limit: usize, deadline: Instant, results: &mut Vec<ElementInfo>) -> Result<()> { // 用原生条件扫描
    let elements = unsafe { root.FindAll(TreeScope_Descendants, condition)? }; // UIA 侧过滤
    let total = unsafe { elements.Length()? }.max(0) as usize; // 候选数量
    for index in 0..total { // 逐候选
        if results.len() >= limit || index % 32 == 0 && Instant::now() >= deadline { break; } // 达上限或超时
        let Ok(element) = (unsafe { elements.GetElement(index as i32) }) else { continue; }; // 跳过读取失败
        let probe = element_info(0, &element); // 读取属性
        if !matches(&probe, params) { continue; } // 二次过滤
        let element_id = worker.store(&element); // 缓存并分配 ID
        results.push(element_info(element_id, &element)); // 收集结果
    }
    Ok(())
}
fn scan_broad(worker: &mut UiaWorker, root: &IUIAutomationElement, params: &FindParams, limit: usize, deadline: Instant, results: &mut Vec<ElementInfo>) -> Result<()> { // 有界宽扫描兜底
    let condition = unsafe { worker.automation().CreateTrueCondition()? }; // 全量条件
    let elements = unsafe { root.FindAll(TreeScope_Descendants, &condition)? }; // 全部后代
    let total = unsafe { elements.Length()? }.max(0) as usize; // 后代总数
    let scan_limit = total.min(params.max_scan.max(1)); // 限制扫描数量（默认 600）
    for index in 0..scan_limit { // 逐元素
        if results.len() >= limit || index % 32 == 0 && Instant::now() >= deadline { break; } // 达上限或超时
        let Ok(element) = (unsafe { elements.GetElement(index as i32) }) else { continue; }; // 跳过读取失败
        let probe = element_info(0, &element); // 读取用于匹配的属性
        if !matches(&probe, params) { continue; } // 未命中跳过
        let element_id = worker.store(&element); // 缓存元素并分配 ID
        results.push(element_info(element_id, &element)); // 收集结果
    }
    Ok(())
}
fn resolve_root(worker: &UiaWorker, params: &FindParams) -> Result<IUIAutomationElement> { // 解析查找根元素
    if let Some(handle) = params.window_handle { return worker.element_from_handle(handle); } // 直接指定句柄
    if params.window_name.is_none() && params.window_class.is_none() && params.process_name.is_none() { // 缺少窗口定位条件
        return Err(anyhow!("find_element 需要 window_handle 或 window_name/window_class/process_name 之一")); // 明确报错
    }
    let filter = WindowFilter { title: params.window_name.clone(), class_name: params.window_class.clone(), process_name: params.process_name.clone(), only_visible: true }; // 构造窗口过滤
    let windows = list_windows(&filter); // 枚举匹配窗口
    let first = windows.first().ok_or_else(|| anyhow!("未找到匹配的顶层窗口"))?; // 取第一个窗口
    worker.element_from_handle(first.hwnd) // 转为 UIA 元素
}
fn matches(info: &ElementInfo, params: &FindParams) -> bool { // 判断属性是否命中
    if let Some(automation_id) = params.automation_id.as_ref() { // AutomationId 精确匹配
        if info.automation_id != *automation_id { return false; } // 不匹配
    }
    if let Some(name) = params.name.as_ref() { // Name 模糊匹配
        if !info.name.to_lowercase().contains(&name.to_lowercase()) { return false; } // 不匹配
    }
    if let Some(class_name) = params.class_name.as_ref() { // ClassName 精确匹配
        if !info.class_name.eq_ignore_ascii_case(class_name) { return false; } // 不匹配
    }
    if let Some(control_type) = params.control_type.as_ref() { // ControlType 匹配
        if !info.control_type.eq_ignore_ascii_case(control_type) { return false; } // 不匹配
    }
    true // 全部命中
}
