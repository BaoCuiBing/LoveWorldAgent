use std::path::PathBuf; // 规则工具：rule_list/rule_read 只读，规则的增删改由设置页与 /api/relus 接口负责
use serde_json::Value; // 参数与结果动态值
use crate::agent::rules; // 规则仓库读取
use crate::agent::tools::{params, blocking, ToolDef}; // 工具定义与同步包装
pub fn build_rule_tools(rule_dir: PathBuf) -> Vec<ToolDef> { // 构造两个只读规则工具
    let dir = rule_dir.clone(); // 克隆目录供 list 使用
    let list = ToolDef::new("rule_list", "列出规则目录中的全部规则（名称、大小、更新时间），只读", params(&[]), blocking(move |_args: String| { // 列规则工具
        let items = rules::list_rules(&dir); // 读取规则列表
        if items.is_empty() { return "（当前没有任何规则）".to_string(); } // 空列表给占位
        serde_json::to_string(&items).unwrap_or_else(|_| "（当前没有任何规则）".to_string()) // 序列化规则列表
    }));
    let dir = rule_dir; // 移动目录供 read 使用
    let read = ToolDef::new("rule_read", "按文件名读取一条规则的完整正文，只读", params(&[("name", "string", "规则文件名，可省略 .md")]), blocking(move |args: String| { // 读规则工具
        let value: Value = serde_json::from_str(&args).unwrap_or_else(|_| serde_json::json!({})); // 解析参数
        let name = value.get("name").and_then(Value::as_str).unwrap_or_default(); // 规则文件名
        match rules::read_rule(&dir, name) { // 读取规则
            Ok(data) => format!("# {}\n\n{}", data.get("name").and_then(Value::as_str).unwrap_or_default(), data.get("body").and_then(Value::as_str).unwrap_or_default()), // 成功返回标题与正文
            Err(err) => format!("读取失败：{err}"), // 失败返回错误文案
        }
    }));
    vec![list, read] // 返回两个只读工具
}