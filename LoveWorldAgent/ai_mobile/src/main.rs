mod config; // 配置模块
mod logger; // 日志模块
mod mobile; // adb 与 Android 设备能力
mod server; // 内置工具服务与管道
use anyhow::{bail, Result}; // 错误类型
use clap::{Parser, Subcommand}; // 命令行解析
#[derive(Parser)] // 命令行参数
#[command(name = "mobile_use", version, about = "Android 移动端自动化 sidecar（内置工具，28 个 mobile_* 工具）")] // 命令元信息
struct Cli { // 顶层命令
    #[command(subcommand)] // 子命令
    command: Option<Command>, // 可选子命令
}
#[derive(Subcommand)] // 子命令定义
enum Command { // 支持的子命令
    Pipe { // 以命名管道客户端运行
        #[arg(long)] // 管道名
        pipe: String, // 不含 \\.\pipe\ 前缀
    },
}
fn main() -> Result<()> { // 程序入口
    let cli = Cli::parse(); // 解析命令行
    let runtime = config::runtime(); // 解析根目录与日志配置
    logger::init(&runtime.log_dir, &runtime.log_level, runtime.log_max_lines); // 初始化文件日志（stderr + 落盘）
    log_info!("main", "启动 mobile_use {}，日志级别={}，pid={}", env!("CARGO_PKG_VERSION"), runtime.log_level, std::process::id()); // 启动记录（含 pid）
    match &runtime.paths { // 根目录探测结果
        Some(paths) => log_info!("main", "根目录={}，静态目录={}，配置文件={}", paths.root.display(), paths.static_dir.display(), runtime.config_file.as_ref().map(|path| path.display().to_string()).unwrap_or_default()), // 命中主应用配置
        None => log_warn!("main", "未探测到 static/config.json，日志目录回退 {}", runtime.log_dir.display()), // 未命中告警
    }
    log_info!("main", "日志文件={}", runtime.log_dir.join(logger::LOG_FILE_NAME).display()); // 落盘位置
    match cli.command { // 分派子命令
        Some(Command::Pipe { pipe }) => { // 命名管道模式
            if pipe.trim().is_empty() { bail!("--pipe 不能为空"); } // 校验管道名
            log_info!("main", "以管道模式启动内置工具服务：\\\\.\\pipe\\{pipe}"); // 启动记录（含管道名）
            server::pipe_client::run(&pipe) // 运行管道客户端（内部自动重连）
        }
        None => { // 无子命令打印用法
            println!("mobile_use 用法:\n  mobile_use pipe --pipe=<名称>"); // 用法提示
            Ok(()) // 正常退出
        }
    }
}