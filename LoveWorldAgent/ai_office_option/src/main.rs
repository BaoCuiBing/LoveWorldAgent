mod com; // COM 晚绑定层
mod common; // 共享层（路径/备份/信封/STA/ProgID）
mod config; // 配置模块
mod excel; // 10 个 excel_* 工具
mod logger; // 日志模块
mod ppt; // 10 个 ppt_* 工具
mod server; // 内置工具服务与管道
mod tools; // 12 个 word_* 工具
mod word; // Word 会话与实例
use anyhow::{bail, Result}; // 错误类型
use clap::{Parser, Subcommand}; // 命令行解析
use windows::Win32::UI::HiDpi::{SetProcessDpiAwarenessContext, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2}; // DPI 感知
#[derive(Parser)] // 命令行参数
#[command(name = "office_option", version, about = "Office 内置 AI 操作工具 sidecar（32 个 word_/ppt_/excel_ 工具）")] // 命令元信息
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
    let dpi_aware = unsafe { SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2).is_ok() }; // 声明逐显示器 DPI 感知，保证窗口坐标按物理像素换算
    let cli = Cli::parse(); // 解析命令行
    let runtime = config::runtime(); // 解析根目录与日志配置
    logger::init(&runtime.log_dir, &runtime.log_level, runtime.log_max_lines); // 初始化文件日志（stderr + 落盘）
    log_info!("main", "启动 office_option {}，日志级别={}，pid={}，DPI感知={}", env!("CARGO_PKG_VERSION"), runtime.log_level, std::process::id(), dpi_aware); // 启动记录（含 pid 与 DPI）
    log_info!("main", "日志文件={}", runtime.log_dir.join(logger::LOG_FILE_NAME).display()); // 落盘位置
    match cli.command { // 分派子命令
        Some(Command::Pipe { pipe }) => { // 命名管道模式
            if pipe.trim().is_empty() { bail!("--pipe 不能为空"); } // 校验管道名
            log_info!("main", "以管道模式启动内置工具服务：\\\\.\\pipe\\{pipe}"); // 启动记录（含管道名）
            server::pipe_client::run(&pipe) // 运行管道客户端（内部自动重连）
        }
        None => { // 无子命令打印用法
            println!("office_option 用法:\n  office_option pipe --pipe=<名称>"); // 用法提示
            Ok(()) // 正常退出
        }
    }
}
