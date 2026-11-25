use std::path::PathBuf; // 技能工具：只暴露启用技能的索引与按需读取（skill_list/skill_read），命中写旁路元数据供技能卡渲染
use serde_json::{json, Value}; // JSON 构造与动态值
use crate::agent::skills; // 技能仓库读取
use crate::agent::tools::{params, blocking, ToolDef}; // 工具定义与同步包装
use crate::agent::tools_details::common::remember_tool_meta; // 元数据写入
pub fn build_skill_tools(skill_dir: PathBuf) -> Vec<ToolDef> { // 构造两个技能工具
    let dir = skill_dir.clone(); // 克隆目录供 list 使用
    let list = ToolDef::new("skill_list", "列出当前启用的技能（名称与用途）；正文需用 skill_read 按需读取，不要假设技能内容", params(&[]), blocking(move |_args: String| { // 列技能工具
        let items: Vec<Value> = skills::list_skills(&dir, "").into_iter().filter(|item| item.get("enabled").and_then(Value::as_bool).unwrap_or(false)).collect(); // 只保留启用技能
        if items.is_empty() { return "（当前没有启用的技能）".to_string(); } // 空列表给占位
        serde_json::to_string(&items).unwrap_or_else(|_| "（当前没有启用的技能）".to_string()) // 序列化技能列表
    }));
    let dir = skill_dir; // 移动目录供 read 使用
    let read = ToolDef::new("skill_read", "读取指定技能 SKILL.md 的正文；系统提示只给名称与描述，需要细节时才调用，技能只读", params(&[("name", "string", "技能名")]), blocking(move |args: String| { // 读技能工具
        let value: Value = serde_json::from_str(&args).unwrap_or_else(|_| json!({})); // 解析参数
        let name = value.get("name").and_then(Value::as_str).unwrap_or_default(); // 技能名
        match skills::read_skill_body(&dir, name) { // 读取技能正文
            Ok(body) => { // 读取成功
                let mut meta = json!({"skill": name, "ok": true, "bytes": body.len(), "lines": body.matches('\n').count() + 1}); // 构造元数据
                for item in skills::list_skills(&dir, "") { // 查找技能描述
                    if item.get("name").and_then(Value::as_str).unwrap_or_default() == name { // 命中目标技能
                        meta["description"] = item.get("description").cloned().unwrap_or_else(|| json!("")); // 写入描述
                        break; // 找到即退出
                    }
                }
                remember_tool_meta("skill_read", meta); // 写入成功元数据
                body // 返回正文
            }
            Err(err) => { // 读取失败
                remember_tool_meta("skill_read", json!({"skill": name, "ok": false, "error": err.to_string()})); // 写入失败元数据
                format!("读取失败：{err}") // 返回错误文案
            }
        }
    }));
    vec![list, read] // 返回两个工具
}
