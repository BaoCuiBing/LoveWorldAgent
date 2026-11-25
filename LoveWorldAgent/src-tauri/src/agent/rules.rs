use std::path::{Path, PathBuf}; // 规则仓库：relus/*.md 的列举、读取、保存与删除，解析 frontmatter 的 alwaysApply/description
use std::collections::BTreeMap; // frontmatter 键值映射
use serde_json::{json, Value}; // JSON 构造
use crate::agent::AgentError; // 规则操作错误
use crate::utils::logger; // 操作日志
pub const DEFAULT_TRIGGER: &str = "always_on"; // 默认始终注入的触发方式
pub const PREVIEW_LIMIT: usize = 160; // 规则预览字符上限
fn valid_chars(filename: &str) -> bool { // 校验文件名只含允许字符
    filename.chars().all(|ch| ch.is_alphanumeric() || matches!(ch, ' ' | '.' | '-' | '_')) // 字母数字与空格点横线
}
pub fn normalize_name(name: &str) -> Result<String, AgentError> { // 归一化并校验规则文件名
    let raw = name.split_whitespace().collect::<Vec<&str>>().join(" "); // 压缩空白
    if raw.is_empty() { return Err(AgentError::new("规则名不能为空")); } // 空名拒绝
    let filename = if raw.to_lowercase().ends_with(".md") { raw.clone() } else { format!("{raw}.md") }; // 补 .md 后缀
    if filename.contains('/') || filename.contains('\\') || filename.starts_with('.') { return Err(AgentError::new("规则名不能包含路径")); } // 禁止路径字符
    let count = filename.chars().count(); // 文件名长度
    let first_ok = filename.chars().next().map(|ch| ch.is_alphanumeric() || ch == '_').unwrap_or(false); // 首字符合法
    if count > 60 || !filename.to_lowercase().ends_with(".md") || !first_ok || !valid_chars(&filename) { // 综合校验
        return Err(AgentError::new("规则名只允许中英文、数字、空格、点、横线与下划线，且不超过 60 字")); // 校验失败报错
    }
    Ok(filename) // 返回合法文件名
}
pub fn parse_frontmatter(text: &str) -> (BTreeMap<String, String>, String) { // 解析规则文件的 frontmatter 与正文
    let mut meta = BTreeMap::new(); // 元数据映射
    if !text.starts_with("---") { return (meta, text.trim_start_matches('\n').to_string()); } // 无 frontmatter 全文作正文
    let parts: Vec<&str> = text.splitn(3, "---").collect(); // 按 --- 切三段
    if parts.len() < 3 { return (meta, text.to_string()); } // 段数不足按无 frontmatter 处理
    for line in parts[1].split('\n') { // 遍历 frontmatter 行
        if let Some((key, value)) = line.split_once(':') { meta.insert(key.trim().to_string(), value.trim().to_string()); } // 按 key: value 解析
    }
    (meta, parts[2].trim_start_matches('\n').to_string()) // 返回元数据与正文
}
pub fn rule_trigger(meta: &BTreeMap<String, String>) -> String { // 计算规则触发方式
    if let Some(value) = meta.get("alwaysApply") { // 兼容 alwaysApply 字段
        return if value.trim().to_lowercase() == "true" { DEFAULT_TRIGGER.to_string() } else { "model_decision".to_string() }; // true 为始终注入
    }
    meta.get("trigger").map(|text| text.trim().to_string()).filter(|text| !text.is_empty()).unwrap_or_else(|| DEFAULT_TRIGGER.to_string()) // 否则取 trigger，缺省始终注入
}
fn dump(trigger: &str, body: &str, description: &str) -> String { // 序列化规则为带 frontmatter 的文本
    let front = if trigger == DEFAULT_TRIGGER { // 始终注入用简化 frontmatter
        "---\nalwaysApply: true\n---\n".to_string() // 简化头
    } else { // 按需注入需带描述
        let mut lines = vec!["---".to_string(), "alwaysApply: false".to_string()]; // frontmatter 头
        let desc = description.trim(); // 描述
        if !desc.is_empty() { lines.push(format!("description: {desc}")); } // 非空才写描述
        lines.push("---".to_string()); // frontmatter 尾
        format!("{}\n", lines.join("\n")) // 拼接
    };
    format!("{front}{}\n", body.trim_end()) // 头 + 正文
}
pub fn preview_text(text: &str, limit: usize) -> String { // 生成规则文本预览
    let (_, body) = parse_frontmatter(text); // 取正文
    let lines: Vec<String> = body.split('\n').map(|line| line.replace('#', "").replace('*', "").replace('`', "").split_whitespace().collect::<Vec<&str>>().join(" ")).filter(|line| !line.is_empty()).collect(); // 去 markdown 符号并压缩
    lines.join(" ").chars().take(limit).collect() // 拼接后截断
}
fn meta_of(path: &Path) -> Value { // 由文件路径构造规则元数据
    let text = std::fs::read_to_string(path).unwrap_or_default(); // 读取规则正文
    let (meta, _) = parse_frontmatter(&text); // 解析 frontmatter
    let mut size = 0u64; // 文件大小
    let mut updated = String::new(); // 修改时间
    if let Ok(info) = std::fs::metadata(path) { // 读取文件元信息
        size = info.len(); // 文件大小
        if let Ok(time) = info.modified() { updated = crate::agent::store_sqls::db::stamp_of(time); } // 格式化修改时间
    }
    json!({"name": path.file_name().map(|name| name.to_string_lossy().to_string()).unwrap_or_default(), "size": size, "updated_at": updated, "trigger": rule_trigger(&meta), "preview": preview_text(&text, PREVIEW_LIMIT)}) // 组装元数据
}
fn md_files(rule_dir: &Path) -> Vec<PathBuf> { // 列举目录下所有 .md 文件
    let mut paths: Vec<PathBuf> = std::fs::read_dir(rule_dir).map(|entries| entries.flatten().map(|entry| entry.path()).filter(|path| path.is_file() && path.extension().map(|ext| ext.to_string_lossy().eq_ignore_ascii_case("md")).unwrap_or(false)).collect()).unwrap_or_default(); // 过滤 .md 文件
    paths.sort_by_key(|path| path.file_name().map(|name| name.to_string_lossy().to_lowercase()).unwrap_or_default()); // 按文件名排序
    paths // 返回路径列表
}
pub fn list_rules(rule_dir: &Path) -> Vec<Value> { // 列出所有规则元数据
    if !rule_dir.exists() { return Vec::new(); } // 目录不存在返回空
    md_files(rule_dir).iter().map(|path| meta_of(path)).collect() // 逐文件构造元数据
}
pub fn read_rule(rule_dir: &Path, name: &str) -> Result<Value, AgentError> { // 读取单条规则（含正文）
    let filename = normalize_name(name)?; // 校验规则名
    let path = rule_dir.join(&filename); // 定位文件
    if !path.exists() { return Err(AgentError::new(format!("规则不存在: {filename}"))); } // 不存在报错
    let text = std::fs::read_to_string(&path).unwrap_or_default(); // 读取正文
    let (meta, body) = parse_frontmatter(&text); // 解析 frontmatter
    let mut data = meta_of(&path); // 构造元数据
    data["body"] = Value::String(body); // 注入正文
    data["trigger"] = Value::String(rule_trigger(&meta)); // 注入触发方式
    data["description"] = Value::String(meta.get("description").map(|text| text.trim().to_string()).unwrap_or_default()); // 注入描述
    Ok(data) // 返回完整规则
}
pub fn save_rule(rule_dir: &Path, name: &str, content: &str, trigger: Option<&str>, description: Option<&str>) -> Result<Value, AgentError> { // 新建或覆盖规则
    let filename = normalize_name(name)?; // 校验规则名
    let path = rule_dir.join(&filename); // 定位文件
    let (incoming_meta, body) = parse_frontmatter(content); // 解析提交内容
    let existing_meta = if path.exists() { parse_frontmatter(&std::fs::read_to_string(&path).unwrap_or_default()).0 } else { BTreeMap::new() }; // 读取原有元数据
    let candidate = trigger.map(|text| text.trim().to_string()).filter(|text| !text.is_empty()) // 显式触发方式优先
        .or_else(|| incoming_meta.get("trigger").map(|text| text.trim().to_string()).filter(|text| !text.is_empty())) // 其次提交内容的 trigger
        .or_else(|| existing_meta.get("trigger").map(|text| text.trim().to_string()).filter(|text| !text.is_empty())); // 最后原有 trigger
    let chosen = candidate.unwrap_or_else(|| rule_trigger(&existing_meta)); // 都没有则按原元数据推断
    let desc = match description { // 描述取值优先级
        Some(text) => text.trim().to_string(), // 显式描述
        None => incoming_meta.get("description").map(|text| text.trim().to_string()).filter(|text| !text.is_empty()).or_else(|| existing_meta.get("description").map(|text| text.trim().to_string())).unwrap_or_default(), // 其次提交内容，最后原有
    };
    std::fs::create_dir_all(rule_dir).map_err(|err| AgentError::new(err.to_string()))?; // 确保目录存在
    std::fs::write(&path, dump(&chosen, &body, &desc)).map_err(|err| AgentError::new(err.to_string()))?; // 落盘
    logger::info("rules", &format!("保存规则: {filename}")); // 记录保存日志
    Ok(meta_of(&path)) // 返回元数据
}
pub fn delete_rule(rule_dir: &Path, name: &str) -> Result<Value, AgentError> { // 删除单条规则
    let filename = normalize_name(name)?; // 校验规则名
    let path = rule_dir.join(&filename); // 定位文件
    if !path.exists() { return Err(AgentError::new(format!("规则不存在: {filename}"))); } // 不存在报错
    std::fs::remove_file(&path).map_err(|err| AgentError::new(err.to_string()))?; // 删除文件
    Ok(json!({"name": filename})) // 返回被删规则名
}
pub fn seed_rules(rule_dir: &Path, bundled_dir: &Path) { // 从随包目录播种默认规则
    if rule_dir.exists() || !bundled_dir.exists() || rule_dir == bundled_dir { return; } // 目标已存在或源缺失时跳过
    let _ = std::fs::create_dir_all(rule_dir); // 创建目标目录
    for path in md_files(bundled_dir) { // 遍历随包规则
        if let Some(name) = path.file_name() { // 取文件名
            let target = rule_dir.join(name); // 目标路径
            if !target.exists() { let _ = std::fs::copy(&path, &target); } // 不存在则复制
        }
    }
}
pub fn load_rules(rule_dir: &Path) -> String { // 加载所有始终注入的规则正文
    if !rule_dir.exists() { return String::new(); } // 目录不存在返回空
    let mut blocks: Vec<String> = Vec::new(); // 规则块缓冲
    for path in md_files(rule_dir) { // 遍历规则文件
        let (meta, body) = parse_frontmatter(&std::fs::read_to_string(&path).unwrap_or_default()); // 解析规则
        if rule_trigger(&meta) != DEFAULT_TRIGGER || body.trim().is_empty() { continue; } // 只取始终注入且非空的规则
        blocks.push(format!("### 规则：{}\n{}", path.file_name().map(|name| name.to_string_lossy().to_string()).unwrap_or_default(), body.trim())); // 生成规则块
    }
    blocks.join("\n\n") // 拼接所有规则
}
pub fn model_decision_index(rule_dir: &Path) -> String { // 生成按需读取规则的索引
    if !rule_dir.exists() { return String::new(); } // 目录不存在返回空
    let mut lines: Vec<String> = Vec::new(); // 索引行缓冲
    for path in md_files(rule_dir) { // 遍历规则文件
        let (meta, _) = parse_frontmatter(&std::fs::read_to_string(&path).unwrap_or_default()); // 解析 frontmatter
        let description = meta.get("description").map(|text| text.trim().to_string()).unwrap_or_default(); // 取描述
        if rule_trigger(&meta) != "model_decision" || description.is_empty() { continue; } // 只取按需且带描述的规则
        lines.push(format!("- {}：{description}", path.file_name().map(|name| name.to_string_lossy().to_string()).unwrap_or_default())); // 生成索引项
    }
    lines.join("\n") // 拼接索引
}
