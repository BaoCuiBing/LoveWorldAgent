// 统一文件日志：定位根目录、读取主应用 logging 配置、写 static/logs/browser_panel.log（线程安全、逐行 flush、按 max_lines 归档）
#pragma once // 防止头文件重复包含
#include <QString> // 日志正文（中文）
namespace logging {
void Init(); // 初始化：定位根目录 → 读取 config.json → 打开日志文件；缺失时回退 exe/logs 并写 WARN
void Write(const char* level, const char* module, const QString& message); // 写一行日志（低于阈值丢弃）
QString LogFile(); // 当前日志文件绝对路径（未初始化时为空）
QString Level(); // 当前日志级别文本
}
#define LOG_DEBUG(module, message) ::logging::Write("DEBUG", module, (message)) // 调试日志
#define LOG_INFO(module, message) ::logging::Write("INFO", module, (message)) // 信息日志
#define LOG_WARN(module, message) ::logging::Write("WARNING", module, (message)) // 警告日志
#define LOG_ERROR(module, message) ::logging::Write("ERROR", module, (message)) // 错误日志
