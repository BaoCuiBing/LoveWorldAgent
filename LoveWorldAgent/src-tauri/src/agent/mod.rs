pub mod agent_core; // 智能体核心：状态机装配、goose 会话/工具/审批、提示词与上下文压缩
pub mod agents_repo; // 自定义子智能体仓库：读写与播种 agents 目录
pub mod chat_prefs; // 对话偏好：提问/待办/通知开关的容错读取
pub mod mcp; // MCP 运行时：连接全部服务并生成工具定义
pub mod model_support; // 模型接入层：OpenAI 兼容协议与华为云 MaaS V2 适配
pub use model_support as models; // 兼容保留 models 别名，供上层路由器与状态机无缝引用
pub mod permission; // 权限判定与白名单：敏感命令审批规则
pub mod rules; // 规则仓库：读写与播种 relus 目录
pub mod settings; // Agent 运行设置：合并配置并定位可写目录
pub mod skills; // 技能仓库：只读扫描与播种 skills 目录
pub mod store_sqls; // 数据库 SQL：会话/消息/用量/上下文读写
pub mod tools; // 工具门面：ToolDef 定义与审批判定
pub mod tools_details; // 具体工具实现：文件/命令/终端/浏览器/MCP 等
#[derive(Debug)] // 错误类型可打印
pub struct AgentError(pub String); // 智能体通用错误包装
impl AgentError { // 错误构造
    pub fn new(text: impl Into<String>) -> Self { // 由文本构造错误
        Self(text.into()) // 转为 String 保存
    }
}
impl std::fmt::Display for AgentError { // 展示接口
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { // 输出错误文本
        f.write_str(&self.0) // 写入内部文本
    }
}
impl std::error::Error for AgentError {} // 标记为标准错误类型
pub fn copy_tree(source: &std::path::Path, target: &std::path::Path) -> std::io::Result<()> { // 递归复制目录树
    std::fs::create_dir_all(target)?; // 确保目标目录存在
    for entry in std::fs::read_dir(source)? { // 遍历源目录条目
        let entry = entry?; // 读取条目，失败上抛
        let path = entry.path(); // 源路径
        let destination = target.join(entry.file_name()); // 目标路径
        if path.is_dir() { copy_tree(&path, &destination)?; } else { std::fs::copy(&path, &destination)?; } // 目录递归，文件直接复制
    }
    Ok(()) // 复制完成
}
