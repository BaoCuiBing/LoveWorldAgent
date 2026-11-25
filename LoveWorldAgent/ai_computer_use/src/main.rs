mod config; // 配置模块
mod logger; // 日志模块
mod omni; // omni_parser 模块（桥接 ../omni_parser/core/）
mod server; // 内置工具服务与管道
mod sys; // 系统能力（窗口/截图/文件/Shell/注册表等）
mod uia; // UIA 核心模块
use anyhow::{bail, Result}; // 错误类型
use clap::{Parser, Subcommand}; // 命令行解析
use windows::Win32::UI::HiDpi::{SetProcessDpiAwarenessContext, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2}; // DPI 感知
#[derive(Parser)] // 命令行参数
#[command(name = "computer_use", version, about = "Windows 桌面自动化 sidecar（内置工具，23 个 computer_* 工具 + omni_parser 视觉解析）")] // 命令元信息
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
    Omni { // omni_parser 模块（屏幕解析 sidecar）
        #[command(subcommand)] // 二级子命令
        command: omni::OmniCommand, // serve / parse
    },
}
fn main() -> Result<()> { // 程序入口
    let dpi_aware = unsafe { SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2).is_ok() }; // 声明逐显示器 DPI 感知，统一 UIA/输入/截图/覆盖层坐标
    let cli = Cli::parse(); // 解析命令行
    let runtime = config::runtime(); // 解析根目录与日志配置
    logger::init(&runtime.log_dir, &runtime.log_level, runtime.log_max_lines); // 初始化文件日志（stderr + 落盘）
    log_info!("main", "启动 computer_use {}，日志级别={}，pid={}，DPI感知={}", env!("CARGO_PKG_VERSION"), runtime.log_level, std::process::id(), dpi_aware); // 启动记录（含 pid 与 DPI）
    match &runtime.paths { // 根目录探测结果
        Some(paths) => log_info!("main", "根目录={}，静态目录={}，配置文件={}", paths.root.display(), paths.static_dir.display(), runtime.config_file.as_ref().map(|path| path.display().to_string()).unwrap_or_default()), // 命中主应用配置
        None => log_warn!("main", "未探测到 static/config.json，日志目录回退 {}", runtime.log_dir.display()), // 未命中告警
    }
    log_info!("main", "日志文件={}", runtime.log_dir.join(logger::LOG_FILE_NAME).display()); // 落盘位置
    match cli.command { // 分派子命令
        Some(Command::Pipe { pipe }) => { // 命名管道模式
            if pipe.trim().is_empty() { bail!("--pipe 不能为空"); } // 校验管道名
            let uia = uia::worker::spawn()?; // 启动专用 UIA 线程
            sys::overlay::start(); // 预先启动覆盖层线程（Windows 消息循环独立于 UIA 线程）
            log_info!("main", "以管道模式启动内置工具服务：\\\\.\\pipe\\{pipe}，UIA 工作线程已启动"); // 启动记录（含管道名）
            server::pipe_client::run(uia, &pipe) // 运行管道客户端（内部自动重连）
        }
        Some(Command::Omni { command }) => { // omni_parser 模式
            match command { // 分派二级子命令
                omni::OmniCommand::Serve { models_dir } => omni::run_serve(models_dir), // stdio JSON 服务
                omni::OmniCommand::Parse { image, models_dir, model } => omni::run_parse(image, models_dir, model), // 单图解析
            }
        }
        None => { // 无子命令打印用法
            println!("computer_use 用法:\n  computer_use pipe --pipe=<名称>\n  computer_use omni serve [--models-dir=<目录>]\n  computer_use omni parse --image=<png> [--models-dir=<目录>] [--model=v2|v3]"); // 用法提示
            Ok(()) // 正常退出
        }
    }
}
