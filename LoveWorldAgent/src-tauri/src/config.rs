use std::path::{Path, PathBuf}; // 路径类型
use once_cell::sync::Lazy; // 惰性全局配置
use serde_json::{json, Map, Value}; // JSON 构造
pub const CONFIG_NAME: &str = "config.json"; // 配置中心：复刻 config.py 的运行期路径、配置分层与原子读写
pub const AGENT_CONFIG_NAME: &str = "agent_config.json"; // 智能体配置文件名
pub const PERMISSION_NAME: &str = "agent_permission.json"; // 权限配置文件名
pub const MCP_CONFIG_NAME: &str = "agent_mcp.json"; // MCP 配置文件名
pub const LEGACY_SECTIONS: [&str; 3] = ["agent", "llm", "permission"]; // 旧版 config.json 中待迁移的段
pub const DEFAULT_LOG_MAX_LINES: usize = 30000; // 日志归档默认阈值
fn exe_dir() -> PathBuf { // 可执行文件所在目录
    std::env::current_exe().ok().and_then(|path| path.parent().map(Path::to_path_buf)).unwrap_or_else(|| PathBuf::from(".")) // 取父目录，失败用当前目录
}
fn detect_project_root() -> Option<PathBuf> { // 开发态检测项目根（含 src-tauri 与 frontend）
    let dir = exe_dir(); // 若 exe 同级已含打包态 webdist 与 static，则强制走冻结态（防止本机分发验证回退源码树）
    if dir.join("webdist").is_dir() && dir.join("static").is_dir() { return None; }
    let manifest = option_env!("CARGO_MANIFEST_DIR")?; // 编译时注入的清单目录
    let root = Path::new(manifest).parent()?.to_path_buf(); // 上一级即项目根
    if root.join("src-tauri").is_dir() && root.join("frontend").is_dir() { Some(root) } else { None } // 校验目录结构
}
pub static PROJECT_ROOT: Lazy<Option<PathBuf>> = Lazy::new(detect_project_root); // 项目根（开发态有值）
pub fn is_frozen() -> bool { // 是否打包发布态
    PROJECT_ROOT.is_none() // 无项目根即打包态
}
pub fn app_dir() -> PathBuf { // 应用目录
    if let Some(root) = PROJECT_ROOT.as_ref() { return root.clone(); } // 开发态用项目根
    if let Some(raw) = std::env::var("APP_DIR").ok().filter(|text| !text.trim().is_empty()) { return PathBuf::from(raw); } // 环境变量
    exe_dir() // 回退可执行文件目录
}
pub fn resource_dir() -> PathBuf { // 资源目录
    if let Some(root) = PROJECT_ROOT.as_ref() { return root.clone(); } // 开发态用项目根
    if let Some(raw) = std::env::var("RESOURCE_DIR").ok().filter(|text| !text.trim().is_empty()) { return PathBuf::from(raw); } // 环境变量
    let base = exe_dir(); // 可执行文件目录
    let nested = base.join("resources"); // 子资源目录
    if nested.is_dir() { nested } else { base } // 存在则用子目录
}
pub fn static_dir() -> PathBuf { // 静态资源目录：开发态 src-tauri/static，打包态资源目录/static
    if let Some(root) = PROJECT_ROOT.as_ref() { return root.join("src-tauri").join("static"); } // 开发态
    resource_dir().join("static") // 打包态
}
pub fn writable_static_dir() -> PathBuf { // 可写静态目录：开发态同 static，打包态为 exe 同级 static
    if let Some(root) = PROJECT_ROOT.as_ref() { return root.join("src-tauri").join("static"); } // 开发态
    app_dir().join("static") // 打包态
}
pub static WEB_DIST_DIR: Lazy<PathBuf> = Lazy::new(|| resource_dir().join("webdist")); // 前端构建产物目录
pub static STATIC_DIR: Lazy<PathBuf> = Lazy::new(static_dir); // 静态目录
pub static WEBVIEW_STORAGE_DIR: Lazy<PathBuf> = Lazy::new(|| writable_static_dir().join("additional").join("webview")); // WebView 用户数据目录
pub static LOVEWORD_DIR: Lazy<PathBuf> = Lazy::new(|| STATIC_DIR.join(".loveword")); // 随包 loveword 目录
pub static WRITABLE_LOVEWORD_DIR: Lazy<PathBuf> = Lazy::new(|| writable_static_dir().join(".loveword")); // 可写 loveword 目录
pub static WRITABLE_CONFIG_FILE: Lazy<PathBuf> = Lazy::new(|| writable_static_dir().join(CONFIG_NAME)); // 可写主配置
pub static BUNDLED_CONFIG_FILE: Lazy<PathBuf> = Lazy::new(|| STATIC_DIR.join(CONFIG_NAME)); // 随包主配置
pub static WRITABLE_AGENT_CONFIG_FILE: Lazy<PathBuf> = Lazy::new(|| WRITABLE_LOVEWORD_DIR.join(AGENT_CONFIG_NAME)); // 可写智能体配置
pub static BUNDLED_AGENT_CONFIG_FILE: Lazy<PathBuf> = Lazy::new(|| LOVEWORD_DIR.join(AGENT_CONFIG_NAME)); // 随包智能体配置
pub static WRITABLE_PERMISSION_FILE: Lazy<PathBuf> = Lazy::new(|| WRITABLE_LOVEWORD_DIR.join(PERMISSION_NAME)); // 可写权限配置
pub static BUNDLED_PERMISSION_FILE: Lazy<PathBuf> = Lazy::new(|| LOVEWORD_DIR.join(PERMISSION_NAME)); // 随包权限配置
pub static WRITABLE_MCP_FILE: Lazy<PathBuf> = Lazy::new(|| WRITABLE_LOVEWORD_DIR.join(MCP_CONFIG_NAME)); // 可写 MCP 配置
pub static BUNDLED_MCP_FILE: Lazy<PathBuf> = Lazy::new(|| LOVEWORD_DIR.join(MCP_CONFIG_NAME)); // 随包 MCP 配置
pub const OFFICE_OPTION_NAME: &str = "agent_office.json"; // Office 工具配置文件名
pub static WRITABLE_OFFICE_OPTION_FILE: Lazy<PathBuf> = Lazy::new(|| WRITABLE_LOVEWORD_DIR.join(OFFICE_OPTION_NAME)); // 可写 Office 配置
pub static BUNDLED_OFFICE_OPTION_FILE: Lazy<PathBuf> = Lazy::new(|| LOVEWORD_DIR.join(OFFICE_OPTION_NAME)); // 随包 Office 配置
pub const MOBILE_OPTION_NAME: &str = "agent_mobile.json"; // 移动端配置文件名
pub static WRITABLE_MOBILE_OPTION_FILE: Lazy<PathBuf> = Lazy::new(|| WRITABLE_LOVEWORD_DIR.join(MOBILE_OPTION_NAME)); // 可写移动端配置
pub static BUNDLED_MOBILE_OPTION_FILE: Lazy<PathBuf> = Lazy::new(|| LOVEWORD_DIR.join(MOBILE_OPTION_NAME)); // 随包移动端配置
pub const COMPUTER_USE_NAME: &str = "agent_computer_use.json"; // 电脑控制配置文件名
pub static WRITABLE_COMPUTER_USE_FILE: Lazy<PathBuf> = Lazy::new(|| WRITABLE_LOVEWORD_DIR.join(COMPUTER_USE_NAME)); // 可写电脑控制配置
pub static BUNDLED_COMPUTER_USE_FILE: Lazy<PathBuf> = Lazy::new(|| LOVEWORD_DIR.join(COMPUTER_USE_NAME)); // 随包电脑控制配置
pub static DEFAULT_CONFIG: Lazy<Value> = Lazy::new(|| { // 主配置默认值
    json!({
        "app": {"name": "LoveWorldAgent", "log_level": "INFO", "project_path": ""}, // 应用基础配置
        "window": {"title": "爱满天下", "width": 1600, "height": 1000, "min_size": [1080, 778], "background_color": "#181818"}, // 窗口配置
        "server": {"host": "127.0.0.1", "port": 8866, "dev_server_url": "http://127.0.0.1:5173"}, // 服务配置
        "logging": {"dir": "", "max_lines": 30000}, // 日志配置
        "ui": {"theme": "dark"}, // UI 配置
        "chat": {"ask": true, "todo": true, "notify": {"banner": true, "sound": true, "volume": 100}} // 对话配置
    })
});
pub static DEFAULT_AGENT_CONFIG: Lazy<Value> = Lazy::new(|| { // 智能体配置默认值
    json!({
        "agent": {"enabled": true, "model": "deepseek-chat", "max_turns": 200, "timeout_seconds": 180, "command_timeout_seconds": 60}, // 运行参数
        "llm": [] // 模型列表为空
    })
});
pub static DEFAULT_PERMISSION: Lazy<Value> = Lazy::new(|| { // 权限配置默认值
    json!({"mode": "auto", "groups": {}, "custom": []}) // 自动模式，无分组与白名单
});
pub fn value_at<'a>(root: &'a Value, path: &[&str]) -> Option<&'a Value> { // 按路径读取 JSON 值
    let mut current = root; // 当前节点
    for key in path { current = current.get(*key)?; } // 逐段下钻，缺失返回 None
    Some(current) // 返回值
}
pub fn get_str(root: &Value, path: &[&str], fallback: &str) -> String { // 按路径读取字符串
    value_at(root, path).and_then(Value::as_str).map(str::to_string).unwrap_or_else(|| fallback.to_string()) // 失败用默认
}
pub fn get_i64(root: &Value, path: &[&str], fallback: i64) -> i64 { // 按路径读取整数
    value_at(root, path).and_then(Value::as_i64).unwrap_or(fallback) // 失败用默认
}
pub fn get_bool(root: &Value, path: &[&str], fallback: bool) -> bool { // 按路径读取布尔
    value_at(root, path).and_then(Value::as_bool).unwrap_or(fallback) // 失败用默认
}
fn env_string(name: &str) -> Option<String> { // 读取非空环境变量
    std::env::var(name).ok().map(|text| text.trim().to_string()).filter(|text| !text.is_empty()) // 去空白并过滤空值
}
pub fn merge(base: &Value, over: &Value) -> Value { // 递归合并 JSON 对象
    match (base, over) { // 按类型组合分派
        (Value::Object(base_map), Value::Object(over_map)) => { // 对象递归合并
            let mut result = base_map.clone(); // 以 base 为底
            for (key, value) in over_map { // 遍历覆盖项
                let merged = match result.get(key) { Some(current) => merge(current, value), None => value.clone() }; // 键存在则递归
                result.insert(key.clone(), merged); // 写入
            }
            Value::Object(result) // 返回合并对象
        }
        _ => over.clone(), // 非对象直接覆盖
    }
}
pub fn read_json(path: Option<&Path>) -> Value { // 读取 JSON 文件，失败返回空对象
    let Some(target) = path else { return json!({}); }; // 无路径返回空
    let Ok(text) = std::fs::read_to_string(target) else { return json!({}); }; // 读取失败返回空
    match serde_json::from_str::<Value>(&text) { Ok(value) if value.is_object() => value, _ => json!({}) } // 仅接受对象
}
pub fn atomic_write_json(path: &Path, data: &Value) -> anyhow::Result<()> { // 通过临时文件原子写入 JSON
    if let Some(parent) = path.parent() { std::fs::create_dir_all(parent)?; } // 确保父目录
    let text = format!("{}\n", serde_json::to_string_pretty(data)?); // 美化序列化
    let name = path.file_name().map(|value| value.to_string_lossy().to_string()).unwrap_or_else(|| CONFIG_NAME.to_string()); // 文件名
    let temp = path.with_file_name(format!("{name}.tmp")); // 临时文件
    std::fs::write(&temp, text)?; // 写临时文件
    if path.exists() { std::fs::remove_file(path)?; } // 删除旧文件
    std::fs::rename(&temp, path)?; // 重命名完成替换
    Ok(()) // 成功
}
pub fn resolve_config_file() -> Option<PathBuf> { // 选取存在的配置文件（可写优先）
    [&*WRITABLE_CONFIG_FILE, &*BUNDLED_CONFIG_FILE].into_iter().find(|path| path.exists()).cloned() // 找第一个存在的
}
pub fn load_config() -> Value { // 加载主配置（默认值 + 文件覆盖）
    let Some(path) = resolve_config_file() else { return DEFAULT_CONFIG.clone(); }; // 无文件返回默认
    merge(&DEFAULT_CONFIG, &read_json(Some(&path))) // 合并默认与文件
}
pub static CONFIG_FILE: Lazy<Option<PathBuf>> = Lazy::new(resolve_config_file); // 配置文件路径快照
pub static CONFIG: Lazy<Value> = Lazy::new(load_config); // 配置快照
pub fn expand_user(raw: &str) -> PathBuf { // 展开 ~ 为用户目录
    let text = raw.trim(); // 去空白
    let home = std::env::var("USERPROFILE").or_else(|_| std::env::var("HOME")).ok(); // 取主目录
    if let Some(home) = home { // 有主目录
        if text == "~" { return PathBuf::from(home); } // 纯 ~
        if let Some(rest) = text.strip_prefix("~/").or_else(|| text.strip_prefix("~\\")) { return PathBuf::from(home).join(rest); } // ~/ 或 ~\
    }
    PathBuf::from(text) // 原样返回
}
pub fn strip_verbatim(path: PathBuf) -> PathBuf { // 去掉 Windows canonicalize 的 \\?\ 前缀，保持与 Python 路径一致
    let text = path.to_string_lossy(); // 路径文本
    match text.strip_prefix(r"\\?\") { Some(rest) => PathBuf::from(rest), None => path } // 去前缀
}
pub fn project_path() -> Option<PathBuf> { // 读取当前项目目录
    let config = load_config(); // 加载配置
    let raw = get_str(&config, &["app", "project_path"], ""); // 取配置路径
    if raw.trim().is_empty() { return None; } // 未配置
    let target = expand_user(&raw); // 展开 ~
    if !target.is_dir() { return None; } // 目录不存在
    Some(strip_verbatim(target.canonicalize().unwrap_or(target))) // 规范化并返回
}
pub fn save_project_path(path: &str) -> anyhow::Result<PathBuf> { // 保存项目目录
    let text = path.trim(); // 去空白
    if text.is_empty() { anyhow::bail!("项目目录不能为空"); } // 空路径
    let target = expand_user(text); // 展开 ~
    if !target.is_dir() { anyhow::bail!("项目目录不存在: {text}"); } // 不存在
    let resolved = strip_verbatim(target.canonicalize().unwrap_or(target)); // 规范化
    let file = resolve_config_file().unwrap_or_else(|| WRITABLE_CONFIG_FILE.clone()); // 目标配置
    let mut data = read_json(Some(&file)); // 读取配置
    let mut app = data.get("app").and_then(Value::as_object).cloned().unwrap_or_default(); // 取 app 段
    app.insert("project_path".to_string(), Value::String(resolved.to_string_lossy().to_string())); // 写入路径
    data["app"] = Value::Object(app); // 写回 app 段
    atomic_write_json(&file, &data)?; // 落盘
    Ok(resolved) // 返回路径
}
pub fn resolve_agent_config_file() -> Option<PathBuf> { // 选取智能体配置文件
    [&*WRITABLE_AGENT_CONFIG_FILE, &*BUNDLED_AGENT_CONFIG_FILE].into_iter().find(|path| path.exists()).cloned() // 可写优先
}
pub fn resolve_permission_file() -> Option<PathBuf> { // 选取权限配置文件
    [&*WRITABLE_PERMISSION_FILE, &*BUNDLED_PERMISSION_FILE].into_iter().find(|path| path.exists()).cloned() // 可写优先
}
pub fn resolve_mcp_file() -> Option<PathBuf> { // 选取 MCP 配置文件
    [&*WRITABLE_MCP_FILE, &*BUNDLED_MCP_FILE].into_iter().find(|path| path.exists()).cloned() // 可写优先
}
pub fn load_agent_config() -> Value { // 加载智能体配置（默认值 + 文件覆盖）
    merge(&DEFAULT_AGENT_CONFIG, &read_json(resolve_agent_config_file().as_deref())) // 合并
}
pub fn ensure_agent_files() -> Value { // 确保所有 agent 配置文件存在并返回迁移报告
    let mut report = json!({"agent_config": "exists", "permission": "exists", "agent_mcp": "exists", "stripped": []}); // 初始报告
    if let Err(err) = migrate_agent_files(&mut report) { report["error"] = Value::String(err.to_string()); } // 迁移失败记录错误
    report // 返回
}
fn migrate_agent_files(report: &mut Value) -> anyhow::Result<()> { // 从旧配置/随包资源迁移 agent 配置文件
    let legacy = read_json(resolve_config_file().as_deref()); // 旧主配置
    if !WRITABLE_AGENT_CONFIG_FILE.exists() { // 智能体配置缺失
        let bundled = read_json(if BUNDLED_AGENT_CONFIG_FILE.exists() { Some(BUNDLED_AGENT_CONFIG_FILE.as_path()) } else { None }); // 随包配置
        let mut override_map = Map::new(); // 覆盖项
        for key in ["agent", "llm"] { if let Some(value) = bundled.get(key) { override_map.insert(key.to_string(), value.clone()); } } // 优先随包段
        if override_map.is_empty() { for key in ["agent", "llm"] { if let Some(value) = legacy.get(key) { override_map.insert(key.to_string(), value.clone()); } } } // 回退旧配置
        atomic_write_json(&WRITABLE_AGENT_CONFIG_FILE, &merge(&DEFAULT_AGENT_CONFIG, &Value::Object(override_map)))?; // 写入
        report["agent_config"] = json!("created"); // 记录新建
    }
    if !WRITABLE_PERMISSION_FILE.exists() { // 权限配置缺失
        let bundled = read_json(if BUNDLED_PERMISSION_FILE.exists() { Some(BUNDLED_PERMISSION_FILE.as_path()) } else { None }); // 随包配置
        let mut permission = if bundled.as_object().map_or(false, |map| !map.is_empty()) { bundled } else { legacy.get("permission").cloned().unwrap_or(Value::Null) }; // 优先随包
        if !permission.as_object().map_or(false, |map| !map.is_empty()) { permission = DEFAULT_PERMISSION.clone(); } // 回退默认
        atomic_write_json(&WRITABLE_PERMISSION_FILE, &permission)?; // 写入
        report["permission"] = json!("created"); // 记录新建
    }
    if !WRITABLE_MCP_FILE.exists() { // MCP 配置缺失
        let bundled = read_json(if BUNDLED_MCP_FILE.exists() { Some(BUNDLED_MCP_FILE.as_path()) } else { None }); // 随包配置
        let mut servers = bundled.get("servers").and_then(Value::as_array).cloned().unwrap_or_default(); // servers 数组
        if servers.is_empty() { // 兼容 mcpServers 映射格式
            if let Some(map) = bundled.get("mcpServers").and_then(Value::as_object) { // 映射
                for (name, item) in map { // 逐项
                    let mut entry = item.as_object().cloned().unwrap_or_default(); // 配置项
                    entry.insert("name".to_string(), Value::String(name.clone())); // 注入名称
                    servers.push(Value::Object(entry)); // 写入数组
                }
            }
        }
        atomic_write_json(&WRITABLE_MCP_FILE, &json!({"servers": servers}))?; // 写入
        report["agent_mcp"] = json!("created"); // 记录新建
    }
    if !WRITABLE_OFFICE_OPTION_FILE.exists() { // Office 工具配置缺失：生成默认
        let bundled = read_json(if BUNDLED_OFFICE_OPTION_FILE.exists() { Some(BUNDLED_OFFICE_OPTION_FILE.as_path()) } else { None }); // 随包配置
        let defaults = json!({"backup": {"keep": 5}, "auto_screenshot": {"word": true, "ppt": true, "excel": true}}); // 默认允许所有合法路径并各自默认开启自动截图
        let merged = merge(&defaults, &bundled); // 随包覆盖默认
        atomic_write_json(&WRITABLE_OFFICE_OPTION_FILE, &merged)?; // 写入
        report["office_option"] = json!("created"); // 记录新建
    }
    if !WRITABLE_MOBILE_OPTION_FILE.exists() { // 移动端配置缺失：生成默认
        let bundled = read_json(if BUNDLED_MOBILE_OPTION_FILE.exists() { Some(BUNDLED_MOBILE_OPTION_FILE.as_path()) } else { None }); // 随包配置
        let defaults = json!({"adbPath": "", "defaultDevice": "", "screenshotMaxSize": 1280}); // 默认值
        let merged = merge(&defaults, &bundled); // 随包覆盖默认
        atomic_write_json(&WRITABLE_MOBILE_OPTION_FILE, &merged)?; // 写入
        report["mobile_option"] = json!("created"); // 记录新建
    }
    if !WRITABLE_COMPUTER_USE_FILE.exists() { // 电脑控制配置缺失：生成默认
        let bundled = read_json(if BUNDLED_COMPUTER_USE_FILE.exists() { Some(BUNDLED_COMPUTER_USE_FILE.as_path()) } else { None }); // 随包配置
        let defaults = json!({"auto_screenshot": true}); // 默认值
        let merged = merge(&defaults, &bundled); // 随包覆盖默认
        atomic_write_json(&WRITABLE_COMPUTER_USE_FILE, &merged)?; // 写入
        report["computer_use"] = json!("created"); // 记录新建
    }
    let created = report["agent_config"] == json!("created") || report["permission"] == json!("created"); // 是否有新建
    if created && resolve_config_file().as_deref() == Some(WRITABLE_CONFIG_FILE.as_path()) { // 从可写主配置剥离旧段
        let present: Vec<&str> = LEGACY_SECTIONS.iter().copied().filter(|key| legacy.get(*key).is_some()).collect(); // 存在的旧段
        if !present.is_empty() { // 有旧段
            let mut remaining = legacy.as_object().cloned().unwrap_or_default(); // 剩余配置
            for key in &present { remaining.remove(*key); } // 移除旧段
            atomic_write_json(&WRITABLE_CONFIG_FILE, &Value::Object(remaining))?; // 写回
            report["stripped"] = json!(present); // 记录剥离
        }
    }
    Ok(()) // 迁移完成
}
pub static MIGRATION_REPORT: Lazy<Value> = Lazy::new(ensure_agent_files); // 迁移报告快照
pub static AGENT_CONFIG: Lazy<Value> = Lazy::new(load_agent_config); // 智能体配置快照
pub static LOG_DIR: Lazy<PathBuf> = Lazy::new(|| { // 日志目录：环境变量 > 配置 > 默认
    if let Some(raw) = env_string("LOG_DIR") { return PathBuf::from(raw); } // 环境变量
    let dir = get_str(&CONFIG, &["logging", "dir"], ""); // 配置
    if !dir.trim().is_empty() { return PathBuf::from(dir); } // 非空
    writable_static_dir().join("logs") // 默认
});
pub static LOG_LEVEL: Lazy<String> = Lazy::new(|| env_string("LOG_LEVEL").unwrap_or_else(|| get_str(&CONFIG, &["app", "log_level"], "INFO"))); // 日志级别
pub static LOG_MAX_LINES: Lazy<usize> = Lazy::new(|| { // 日志归档阈值
    let raw = get_i64(&CONFIG, &["logging", "max_lines"], DEFAULT_LOG_MAX_LINES as i64); // 配置值
    if raw >= 100 { raw as usize } else { DEFAULT_LOG_MAX_LINES } // 至少 100
});
pub static DEV_SERVER_URL: Lazy<String> = Lazy::new(|| env_string("DEV_SERVER_URL").unwrap_or_else(|| get_str(&CONFIG, &["server", "dev_server_url"], "http://127.0.0.1:5173"))); // 开发服务器地址
pub static HTTP_HOST: Lazy<String> = Lazy::new(|| env_string("HTTP_HOST").unwrap_or_else(|| get_str(&CONFIG, &["server", "host"], "127.0.0.1"))); // HTTP 主机
pub static HTTP_PORT: Lazy<u16> = Lazy::new(|| { // HTTP 端口
    if let Some(port) = env_string("HTTP_PORT").and_then(|text| text.parse::<u16>().ok()) { return port; } // 环境变量
    get_i64(&CONFIG, &["server", "port"], 8866) as u16 // 配置值
});
pub static WINDOW_TITLE: Lazy<String> = Lazy::new(|| get_str(&CONFIG, &["window", "title"], "爱满天下")); // 窗口标题
pub static WINDOW_WIDTH: Lazy<f64> = Lazy::new(|| get_i64(&CONFIG, &["window", "width"], 1600) as f64); // 窗口宽
pub static WINDOW_HEIGHT: Lazy<f64> = Lazy::new(|| get_i64(&CONFIG, &["window", "height"], 1000) as f64); // 窗口高
pub static WINDOW_MIN_SIZE: Lazy<(f64, f64)> = Lazy::new(|| { // 窗口最小尺寸
    let raw = value_at(&CONFIG, &["window", "min_size"]).and_then(Value::as_array).cloned().unwrap_or_default(); // 配置数组
    let width = raw.first().and_then(Value::as_f64).unwrap_or(1080.0); // 宽
    let height = raw.get(1).and_then(Value::as_f64).unwrap_or(778.0); // 高
    (width, height) // 返回元组
});
pub static BACKGROUND_COLOR: Lazy<String> = Lazy::new(|| get_str(&CONFIG, &["window", "background_color"], "#181818")); // 窗口背景色
pub static WINDOW_ICON: Lazy<PathBuf> = Lazy::new(|| STATIC_DIR.join("imgs").join("dragon.png")); // 窗口图标路径
pub static ICON_CACHE_DIR: Lazy<PathBuf> = Lazy::new(|| writable_static_dir().join("imgs")); // 图标缓存目录
pub fn init() { // 强制初始化全部惰性配置
    Lazy::force(&CONFIG); // 主配置
    Lazy::force(&CONFIG_FILE); // 配置文件路径
    Lazy::force(&MIGRATION_REPORT); // 迁移报告
    Lazy::force(&AGENT_CONFIG); // 智能体配置
}
