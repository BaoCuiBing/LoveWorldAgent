use std::path::{Path, PathBuf}; // 技能仓库：skills/<name>/SKILL.md，frontmatter 存 name/description/enabled
use std::collections::BTreeMap; // frontmatter 键值映射
use serde_json::{json, Value}; // JSON 构造
use crate::agent::AgentError; // 技能操作错误
pub const LOCKED_SKILLS: [(&str, &str); 6] = [("browser-automation", "browser"), ("computer-control", "computer"), ("word-control", "word"), ("ppt-control", "ppt"), ("excel-control", "excel"), ("mobile-control", "mobile")]; // 随浏览器/电脑/Office/移动端开关联动的锁定技能
pub fn locked_driver(name: &str) -> Option<&'static str> { // 返回锁定技能的驱动开关名
    LOCKED_SKILLS.iter().find(|(skill, _)| *skill == name).map(|(_, driver)| *driver) // 命中返回 browser/computer
}
pub fn normalize_name(name: &str) -> Result<String, AgentError> { // 校验技能名合法性
    let raw = name.trim(); // 去首尾空白
    if raw.is_empty() { return Err(AgentError::new("技能名不能为空")); } // 空名拒绝
    if raw.contains('/') || raw.contains('\\') || raw.starts_with('.') { return Err(AgentError::new("技能名不能包含路径")); } // 禁止路径字符
    let mut chars = raw.chars(); // 字符迭代器
    let first_ok = chars.next().map(|ch| ch.is_ascii_alphanumeric()).unwrap_or(false); // 首字符须字母数字
    let rest_ok = chars.all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '.' | '-' | '_')); // 其余字符须字母数字点横线
    if !first_ok || !rest_ok || raw.chars().count() > 41 { return Err(AgentError::new("技能名只允许英文字母、数字、点、横线与下划线，且不超过 41 字")); } // 规则不满足报错
    Ok(raw.to_string()) // 返回合法名称
}
fn skill_file(skill_dir: &Path, name: &str) -> Result<PathBuf, AgentError> { // 由名称定位 SKILL.md
    Ok(skill_dir.join(normalize_name(name)?).join("SKILL.md")) // 校验名称后拼接路径
}
pub fn parse_frontmatter(text: &str) -> (BTreeMap<String, String>, String) { // 解析技能文件的 frontmatter 与正文
    let mut meta = BTreeMap::new(); // 元数据映射
    if !text.starts_with("---") { return (meta, text.trim_start_matches('\n').to_string()); } // 无 frontmatter 全文作正文
    let parts: Vec<&str> = text.splitn(3, "---").collect(); // 按 --- 切三段
    if parts.len() < 3 { return (meta, text.to_string()); } // 段数不足按无 frontmatter 处理
    for line in parts[1].split('\n') { // 遍历 frontmatter 行
        if let Some((key, value)) = line.split_once(':') { meta.insert(key.trim().to_string(), value.trim().to_string()); } // 按 key: value 解析
    }
    (meta, parts[2].trim_start_matches('\n').to_string()) // 返回元数据与正文
}
fn dump(meta: &BTreeMap<String, String>, body: &str) -> String { // 序列化技能为带 frontmatter 的文本
    let mut lines = vec!["---".to_string()]; // frontmatter 起始
    for key in ["name", "description", "enabled"] { // 固定字段顺序
        lines.push(format!("{key}: {}", meta.get(key).cloned().unwrap_or_default())); // 逐字段输出
    }
    lines.push("---".to_string()); // frontmatter 结束
    format!("{}\n\n{}\n", lines.join("\n"), body.trim_end()) // 拼接头与正文
}
fn enabled_of(meta: &BTreeMap<String, String>) -> bool { // 解析启用标志，缺省 true
    let text = meta.get("enabled").cloned().unwrap_or_else(|| "true".to_string()); // 缺省 true
    !matches!(text.trim().to_lowercase().as_str(), "false" | "0" | "no" | "off") // 非假值即为真
}
fn meta_of(path: &Path, text: Option<&str>) -> Value { // 由路径与可选文本构造技能元数据
    let content = text.map(str::to_string).unwrap_or_else(|| std::fs::read_to_string(path).unwrap_or_default()); // 取文本内容
    let (meta, _) = parse_frontmatter(&content); // 解析 frontmatter
    let mut size = 0u64; // 文件大小
    let mut updated = String::new(); // 修改时间
    if let Ok(info) = std::fs::metadata(path) { // 读取文件元信息
        size = info.len(); // 文件大小
        if let Ok(time) = info.modified() { updated = crate::agent::store_sqls::db::stamp_of(time); } // 格式化修改时间
    }
    let name = meta.get("name").filter(|text| !text.is_empty()).cloned().unwrap_or_else(|| path.parent().and_then(|parent| parent.file_name()).map(|name| name.to_string_lossy().to_string()).unwrap_or_default()); // 名称缺省用目录名
    let driver = locked_driver(&name); // 是否锁定技能
    json!({"name": name, "description": meta.get("description").cloned().unwrap_or_default(), "enabled": enabled_of(&meta), "updated_at": updated, "size": size, "locked": driver.is_some(), "driven_by": driver}) // 组装元数据（含锁定标志与驱动开关）
}
fn skill_files(skill_dir: &Path) -> Vec<PathBuf> { // 列举目录下所有 SKILL.md
    let mut paths: Vec<PathBuf> = std::fs::read_dir(skill_dir).map(|entries| entries.flatten().map(|entry| entry.path().join("SKILL.md")).filter(|path| path.is_file()).collect()).unwrap_or_default(); // 收集 SKILL.md 路径
    paths.sort_by_key(|path| path.parent().and_then(|parent| parent.file_name()).map(|name| name.to_string_lossy().to_lowercase()).unwrap_or_default()); // 按目录名排序
    paths // 返回路径列表
}
pub fn list_skills(skill_dir: &Path, keyword: &str) -> Vec<Value> { // 列举技能，可按关键字过滤
    if !skill_dir.exists() { return Vec::new(); } // 目录不存在返回空
    let word = keyword.trim().to_lowercase(); // 归一化关键字
    let mut items: Vec<Value> = Vec::new(); // 结果缓冲
    for path in skill_files(skill_dir) { // 遍历技能文件
        let text = std::fs::read_to_string(&path).unwrap_or_default(); // 读取正文
        if !word.is_empty() { // 有关键字才过滤
            let (meta, _) = parse_frontmatter(&text); // 解析 frontmatter
            let name = meta.get("name").filter(|value| !value.is_empty()).cloned().unwrap_or_else(|| path.parent().and_then(|parent| parent.file_name()).map(|name| name.to_string_lossy().to_string()).unwrap_or_default()); // 技能名
            let haystack = format!("{name}\n{}", meta.get("description").cloned().unwrap_or_default()).to_lowercase(); // 名称+描述作匹配源
            if !haystack.contains(&word) { continue; } // 不含关键字跳过
        }
        items.push(meta_of(&path, Some(&text))); // 写入元数据
    }
    items // 返回技能列表
}
pub fn read_skill(skill_dir: &Path, name: &str) -> Result<Value, AgentError> { // 读取单个技能（含正文）
    let path = skill_file(skill_dir, name)?; // 定位文件
    if !path.exists() { return Err(AgentError::new(format!("技能不存在: {}", normalize_name(name)?))); } // 不存在报错
    let (_, body) = parse_frontmatter(&std::fs::read_to_string(&path).unwrap_or_default()); // 提取正文
    let mut data = meta_of(&path, None); // 构造元数据
    data["body"] = Value::String(body); // 注入正文
    Ok(data) // 返回完整技能
}
pub fn save_skill(skill_dir: &Path, name: &str, description: &str, content: &str, enabled: bool) -> Result<Value, AgentError> { // 新建或覆盖技能
    let folder = skill_dir.join(normalize_name(name)?); // 校验名称并定位目录
    std::fs::create_dir_all(&folder).map_err(|err| AgentError::new(err.to_string()))?; // 确保目录存在
    let enabled = if locked_driver(name).is_some() { // 锁定技能的 enabled 由驱动开关决定，忽略传入值
        parse_frontmatter(&std::fs::read_to_string(folder.join("SKILL.md")).unwrap_or_default()).0.get("enabled").map(|text| !matches!(text.trim().to_lowercase().as_str(), "false" | "0" | "no" | "off")).unwrap_or(enabled) // 保留现有开关
    } else { enabled }; // 非锁定技能用传入值
    let mut meta = BTreeMap::new(); // 元数据映射
    meta.insert("name".to_string(), folder.file_name().map(|value| value.to_string_lossy().to_string()).unwrap_or_default()); // 名称取目录名
    meta.insert("description".to_string(), description.split_whitespace().collect::<Vec<&str>>().join(" ")); // 描述压缩空白
    meta.insert("enabled".to_string(), if enabled { "true".to_string() } else { "false".to_string() }); // 启用标志
    let path = folder.join("SKILL.md"); // 文件路径
    std::fs::write(&path, dump(&meta, content)).map_err(|err| AgentError::new(err.to_string()))?; // 落盘
    Ok(meta_of(&path, None)) // 返回元数据
}
pub fn delete_skill(skill_dir: &Path, name: &str) -> Result<Value, AgentError> { // 删除整个技能目录
    let folder = skill_dir.join(normalize_name(name)?); // 校验名称并定位目录
    if !folder.join("SKILL.md").exists() { return Err(AgentError::new(format!("技能不存在: {}", folder.file_name().map(|value| value.to_string_lossy().to_string()).unwrap_or_default()))); } // 不存在报错
    std::fs::remove_dir_all(&folder).map_err(|err| AgentError::new(err.to_string()))?; // 递归删除目录
    Ok(json!({"name": folder.file_name().map(|value| value.to_string_lossy().to_string()).unwrap_or_default()})) // 返回被删名称
}
pub fn seed_skills(skill_dir: &Path, bundled_dir: &Path) { // 从随包目录播种默认技能
    if !bundled_dir.exists() || skill_dir == bundled_dir { return; } // 源缺失或同目录跳过
    let _ = std::fs::create_dir_all(skill_dir); // 创建目标目录
    let Ok(entries) = std::fs::read_dir(bundled_dir) else { return; }; // 读取随包目录失败跳过
    for entry in entries.flatten() { // 遍历随包条目
        let folder = entry.path(); // 条目路径
        if !folder.is_dir() || !folder.join("SKILL.md").exists() { continue; } // 只处理含 SKILL.md 的目录
        let target = skill_dir.join(entry.file_name()); // 目标路径
        if !target.exists() { let _ = crate::agent::copy_tree(&folder, &target); } // 不存在则复制目录
    }
}
pub fn set_enabled(skill_dir: &Path, name: &str, enabled: bool) -> Result<Value, AgentError> { // 切换技能启用状态（锁定技能拒绝手工修改）
    if let Some(driver) = locked_driver(name) { return Err(AgentError::new(format!("技能「{name}」已锁定，随「{driver}」开关自动变化，不能手动修改"))); } // 锁定技能拒绝
    let path = skill_file(skill_dir, name)?; // 定位文件
    if !path.exists() { return Err(AgentError::new(format!("技能不存在: {}", normalize_name(name)?))); } // 不存在报错
    let (mut meta, body) = parse_frontmatter(&std::fs::read_to_string(&path).unwrap_or_default()); // 解析 frontmatter
    if meta.get("name").filter(|text| !text.is_empty()).is_none() { // 名称为空时补默认值
        meta.insert("name".to_string(), path.parent().and_then(|parent| parent.file_name()).map(|value| value.to_string_lossy().to_string()).unwrap_or_default()); // 用目录名
    }
    meta.insert("enabled".to_string(), if enabled { "true".to_string() } else { "false".to_string() }); // 写入启用标志
    std::fs::write(&path, dump(&meta, &body)).map_err(|err| AgentError::new(err.to_string()))?; // 落盘
    Ok(meta_of(&path, None)) // 返回元数据
}
pub fn sync_locked(skill_dir: &Path, driver: &str, enabled: bool) { // 内部同步：按 browser/computer 开关写入对应锁定技能的 enabled
    for (skill, key) in LOCKED_SKILLS { // 遍历锁定技能
        if key != driver { continue; } // 只处理该驱动对应技能
        let path = skill_dir.join(skill).join("SKILL.md"); // 技能文件路径
        if !path.exists() { continue; } // 不存在跳过
        let text = std::fs::read_to_string(&path).unwrap_or_default(); // 读取正文
        let (mut meta, body) = parse_frontmatter(&text); // 解析元数据
        meta.insert("enabled".to_string(), if enabled { "true".to_string() } else { "false".to_string() }); // 写入开关
        let _ = std::fs::write(&path, dump(&meta, &body)); // 落盘，失败忽略
    }
}
pub fn has_enabled(skill_dir: &Path) -> bool { // 是否存在启用技能，用于决定系统提示词是否注入技能段
    list_skills(skill_dir, "").into_iter().any(|item| item.get("enabled").and_then(Value::as_bool).unwrap_or(false)) // 复用列举结果按启用标志判断
}
pub fn skill_index_text(skill_dir: &Path) -> String { // 生成启用技能的索引文本供系统提示词注入
    let items: Vec<Value> = list_skills(skill_dir, "").into_iter().filter(|item| item.get("enabled").and_then(Value::as_bool).unwrap_or(false)).collect(); // 只保留启用技能
    if items.is_empty() { return "（当前没有启用的技能）".to_string(); } // 无启用技能给占位
    items.iter().map(|item| format!("- {}：{}", item.get("name").and_then(Value::as_str).unwrap_or_default(), item.get("description").and_then(Value::as_str).unwrap_or_default())).collect::<Vec<String>>().join("\n") // 逐技能输出名称与描述
}
pub fn read_skill_body(skill_dir: &Path, name: &str) -> Result<String, AgentError> { // 只读取技能正文
    Ok(read_skill(skill_dir, name)?.get("body").and_then(Value::as_str).unwrap_or_default().to_string()) // 取 body 字段
}
