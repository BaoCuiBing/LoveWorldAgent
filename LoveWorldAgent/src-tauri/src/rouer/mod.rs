pub mod agent_router; // 路由模块入口：汇总全部业务路由并交由 app.rs 挂载到 HTTP 服务
pub mod browser_router; // 内置浏览器状态与操作路由
pub mod chat_sse_router; // 对话 SSE 流式路由
pub mod computer_use_router; // 电脑控制总开关路由
pub mod mcp_config_router; // MCP 服务配置路由
pub mod mobile_router; // 移动端设置路由
pub mod model_router; // 模型配置路由
pub mod other_router; // 窗口/文档等杂项路由
pub mod permission_router; // 权限设置路由
pub mod project_router; // 项目目录与文件树路由
pub mod relus_router; // 规则管理路由
pub mod session_router; // 会话管理路由
pub mod skill_router; // 技能管理路由
pub mod system_router; // 系统信息与配置路由
pub mod terminal_router; // 终端会话管理路由
pub mod upload_router; // 文件上传路由
pub mod office_option_router; // Office 工具设置路由（Word/PPT/Excel）
use axum::Router; // axum 路由器类型
use crate::state::AppState; // 全局应用状态
pub fn register() -> Router<AppState> { // 汇总全部子路由并合并
    Router::new() // 创建空路由器
        .merge(relus_router::router()) // 合并规则路由
        .merge(skill_router::router()) // 合并技能路由
        .merge(agent_router::router()) // 合并子智能体路由
        .merge(permission_router::router()) // 合并权限路由
        .merge(model_router::router()) // 合并模型路由
        .merge(system_router::router()) // 合并系统路由
        .merge(session_router::router()) // 合并会话路由
        .merge(mcp_config_router::router()) // 合并 MCP 配置路由
        .merge(upload_router::router()) // 合并上传路由
        .merge(project_router::router()) // 合并项目路由
        .merge(terminal_router::router()) // 合并终端路由
        .merge(browser_router::router()) // 合并浏览器路由
        .merge(computer_use_router::router()) // 合并电脑控制开关路由
        .merge(office_option_router::router()) // 合并 Office 工具设置路由
        .merge(mobile_router::router()) // 合并移动端设置路由
        .merge(other_router::router()) // 合并杂项路由
        .merge(chat_sse_router::router()) // 合并对话 SSE 路由
}
