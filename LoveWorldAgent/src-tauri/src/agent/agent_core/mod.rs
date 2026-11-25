pub mod assistant; // 智能体核心：状态机装配、goose 会话/工具/审批、提示词与上下文压缩
pub mod context; // 上下文压缩：历史摘要与关键事实的抽取、落盘与注入
pub mod goose_approval; // 审批门：Allow/Deny/Ask 判定与工具提供者包装
pub mod goose_session; // 会话适配：对话落 SQLite 与状态机效果处理
pub mod goose_tools; // 工具适配：ToolDef 转 rmcp Tool 并分发执行
pub mod state_machine; // 状态机装配：步骤编排与 SSE 事件驱动
