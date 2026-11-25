use std::fs::{File, OpenOptions}; // 日志工具：写入 logs/app.log，按行数滚动归档，兼容 LOG_DIR / LOG_LEVEL 环境变量
use std::io::Write; // 写入 trait
use std::path::{Path, PathBuf}; // 路径类型
use chrono::{FixedOffset, Utc}; // 时间格式化
use once_cell::sync::Lazy; // 全局惰性状态
use parking_lot::Mutex; // 互斥锁
pub const LOG_FILE_NAME: &str = "app.log"; // 日志文件名
pub const LOG_FORMAT_TIME: &str = "%Y-%m-%d %H:%M:%S"; // 时间格式
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)] // 级别可比较
pub enum Level { // 日志级别，数值越大越严重
    Debug, // 调试
    Info, // 信息
    Warn, // 警告
    Error, // 错误
}
impl Level { // 级别解析与标签
    pub fn parse(text: &str) -> Level { // 从字符串解析级别
        match text.trim().to_uppercase().as_str() { // 归一化大写
            "DEBUG" => Level::Debug, // 调试
            "WARN" | "WARNING" => Level::Warn, // 警告
            "ERROR" => Level::Error, // 错误
            _ => Level::Info, // 默认信息
        }
    }
    pub fn label(self) -> &'static str { // 级别标签文本
        match self { // 按级别分派
            Level::Debug => "DEBUG", // 调试
            Level::Info => "INFO", // 信息
            Level::Warn => "WARNING", // 警告
            Level::Error => "ERROR", // 错误
        }
    }
}
struct State { // 日志全局状态
    dir: PathBuf, // 日志目录
    level: Level, // 最低级别
    max_lines: usize, // 归档阈值
    lines: usize, // 当前行数
    file: Option<File>, // 日志文件句柄
}
static STATE: Lazy<Mutex<State>> = Lazy::new(|| { // 全局日志状态
    Mutex::new(State { dir: PathBuf::from("logs"), level: Level::Info, max_lines: 30000, lines: 0, file: None }) // 默认配置
});
fn count_lines(path: &Path) -> usize { // 统计文件行数
    std::fs::read(path).map(|bytes| bytes.iter().filter(|byte| **byte == b'\n').count()).unwrap_or(0) // 数换行符
}
fn open_file(state: &mut State) { // 打开日志文件并统计行数
    if let Err(err) = std::fs::create_dir_all(&state.dir) { eprintln!("[LOGGER] 创建日志目录失败: {err}"); return; } // 创建目录
    let path = state.dir.join(LOG_FILE_NAME); // 日志路径
    state.lines = count_lines(&path); // 统计现有行数
    match OpenOptions::new().create(true).append(true).open(&path) { // 追加模式打开
        Ok(file) => state.file = Some(file), // 成功保存句柄
        Err(err) => { // 失败
            state.file = None; // 清空句柄
            eprintln!("[LOGGER] 打开日志文件失败: {err}"); // 报错到 stderr
        }
    }
}
pub fn init(dir: &Path, level: &str, max_lines: usize) { // 初始化日志配置
    let mut state = STATE.lock(); // 加锁
    state.dir = dir.to_path_buf(); // 设置目录
    state.level = Level::parse(level); // 设置级别
    state.max_lines = max_lines.max(100); // 归档阈值至少 100
    state.file = None; // 重置句柄
    state.lines = 0; // 重置行数
    open_file(&mut state); // 打开文件
}
fn next_archive_num(dir: &Path) -> u32 { // 计算下一个归档编号
    let Ok(entries) = std::fs::read_dir(dir) else { return 1; }; // 读目录失败返回 1
    let mut max = 0u32; // 当前最大编号
    for entry in entries.flatten() { // 遍历条目
        let name = entry.file_name().to_string_lossy().to_string(); // 文件名
        let Some(middle) = name.strip_prefix("old_").and_then(|rest| rest.strip_suffix(&format!("_{LOG_FILE_NAME}"))) else { continue; }; // 匹配 old_N_app.log
        if let Ok(value) = middle.parse::<u32>() { max = max.max(value); } // 取编号最大值
    }
    max + 1 // 下一个编号
}
fn archive(state: &mut State) { // 归档当前日志并重建
    let path = state.dir.join(LOG_FILE_NAME); // 当前日志路径
    let target = state.dir.join(format!("old_{}_{LOG_FILE_NAME}", next_archive_num(&state.dir))); // 归档路径
    state.file = None; // 释放句柄
    match std::fs::rename(&path, &target) { // 重命名归档
        Ok(()) => { // 成功
            state.lines = 0; // 重置行数
            open_file(state); // 重建日志
            eprintln!("[LOGGER] 日志已归档: {}，重建 {LOG_FILE_NAME} 继续写入", target.display()); // 记录
        }
        Err(err) => { // 失败
            state.lines = 0; // 重置行数
            eprintln!("[LOGGER] 归档 {LOG_FILE_NAME} 失败: {err}"); // 报错
        }
    }
}
pub fn log(level: Level, module: &str, message: &str) { // 写一条日志
    let mut state = STATE.lock(); // 加锁
    if level < state.level { return; } // 低于阈值丢弃
    let stamp = Utc::now().with_timezone(&FixedOffset::east_opt(8 * 3600).unwrap()).format(LOG_FORMAT_TIME).to_string(); // 东八区时间
    let line = format!("{stamp} [{}] {module}: {message}\n", level.label()); // 格式化日志行
    let mut stdout = std::io::stdout(); // 标准输出
    let _ = stdout.write_all(line.as_bytes()); // 输出到控制台
    let _ = stdout.flush(); // 刷送
    if let Some(file) = state.file.as_mut() { // 写文件
        let _ = file.write_all(line.as_bytes()); // 追加
        let _ = file.flush(); // 刷送
    }
    state.lines += 1; // 行数递增
    if state.lines >= state.max_lines { archive(&mut state); } // 达阈值归档
}
pub fn debug(module: &str, message: &str) { // 调试日志
    log(Level::Debug, module, message); // 委托
}
pub fn info(module: &str, message: &str) { // 信息日志
    log(Level::Info, module, message); // 委托
}
pub fn warn(module: &str, message: &str) { // 警告日志
    log(Level::Warn, module, message); // 委托
}
pub fn error(module: &str, message: &str) { // 错误日志
    log(Level::Error, module, message); // 委托
}
