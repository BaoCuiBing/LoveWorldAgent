// 日志实现：exe 目录向上探测 src-tauri/static（开发态）或 app_dir/static（打包态），目录与阈值取主应用 config.json，行格式与主应用 app.log 一致
#include "utils/logging.h" // 自身声明
#include <windows.h> // GetModuleFileNameW
#include <cstdio> // stderr 提示
#include <cstdlib> // getenv
#include <mutex> // 线程安全
#include <QDateTime> // 时间戳
#include <QDir> // 目录创建与遍历
#include <QFile> // 文件读写
#include <QFileInfo> // 路径信息
#include <QJsonDocument> // 配置解析
#include <QJsonObject> // 配置对象
namespace {
const char* kLogName = "browser_panel.log"; // 日志文件名
const int kMinMaxLines = 100; // 归档阈值下限
const int kDefaultMaxLines = 30000; // 归档阈值默认值（与主应用一致）
const int kMaxUpLevels = 8; // 开发态向上探测层数
struct State { QString dir; QString file; QString level; int max_lines = kDefaultMaxLines; int lines = 0; QFile* handle = nullptr; }; // 全局日志状态
State g_state; // 全局状态实例
std::mutex g_lock; // 写锁（UI 线程 + 管道读写线程 + CEF 回调并发）
int Rank(const QString& level) { const QString text = level.trimmed().toUpper(); if (text == "DEBUG") return 0; if (text.startsWith("WARN")) return 2; if (text == "ERROR") return 3; return 1; } // 级别排序值
QString ExeDir() { wchar_t buffer[MAX_PATH] = {0}; const DWORD size = ::GetModuleFileNameW(nullptr, buffer, MAX_PATH); return size > 0 ? QFileInfo(QString::fromWCharArray(buffer, static_cast<int>(size))).absolutePath() : QDir::currentPath(); } // exe 所在目录
QString DevRoot(const QString& from) { QDir dir(from); for (int i = 0; i < kMaxUpLevels; ++i) { const QString candidate = dir.absoluteFilePath(QStringLiteral("src-tauri/static")); if (QFileInfo::exists(candidate + QStringLiteral("/config.json"))) return QDir::cleanPath(dir.absolutePath()); if (!dir.cdUp()) break; } return QString(); } // 开发态：向上命中 src-tauri/static/config.json，返回项目根
QString PackagedStaticDir(const QString& exe_dir) { const QString app = QDir(exe_dir).absoluteFilePath(QStringLiteral("..")); const QStringList candidates = { QDir::cleanPath(app + QStringLiteral("/static")), exe_dir + QStringLiteral("/static"), QDir::cleanPath(app + QStringLiteral("/resources/static")), QDir::cleanPath(app + QStringLiteral("/resource/static")) }; for (const QString& candidate : candidates) { if (QFileInfo::exists(candidate + QStringLiteral("/config.json"))) return candidate; } return QString(); } // 打包态：可写 app_dir/static 优先，安装资源回退
QString Env(const char* key) { const char* value = std::getenv(key); return value != nullptr ? QString::fromLocal8Bit(value).trimmed() : QString(); } // 读取非空环境变量
int CountLines(const QString& path) { QFile file(path); if (!file.open(QIODevice::ReadOnly)) return 0; return static_cast<int>(file.readAll().count('\n')); } // 统计现有行数
int NextArchiveNum(const QString& dir) { const QString suffix = QStringLiteral("_") + QString::fromLatin1(kLogName); const QStringList names = QDir(dir).entryList(QStringList() << QStringLiteral("old_*"), QDir::Files); int max = 0; for (const QString& name : names) { if (!name.endsWith(suffix)) continue; const QString middle = name.mid(4, name.size() - 4 - suffix.size()); bool ok = false; const int value = middle.toInt(&ok); if (ok) max = qMax(max, value); } return max + 1; } // 计算下一个归档编号
void OpenFile() { g_state.handle = new QFile(g_state.file); if (!g_state.handle->open(QIODevice::WriteOnly | QIODevice::Append)) { delete g_state.handle; g_state.handle = nullptr; fprintf(stderr, "[LOGGING] 打开日志失败: %s\n", g_state.file.toLocal8Bit().constData()); return; } g_state.lines = CountLines(g_state.file); } // 追加模式打开并统计行数
void ArchiveLocked() { if (g_state.handle != nullptr) { g_state.handle->close(); delete g_state.handle; g_state.handle = nullptr; } const QString target = g_state.dir + QStringLiteral("/old_%1_%2").arg(NextArchiveNum(g_state.dir)).arg(QString::fromLatin1(kLogName)); if (QFile::rename(g_state.file, target)) fprintf(stderr, "[LOGGING] 日志已归档: %s\n", target.toLocal8Bit().constData()); else fprintf(stderr, "[LOGGING] 归档失败: %s\n", g_state.file.toLocal8Bit().constData()); OpenFile(); } // 归档并重建
void WriteLocked(const char* level, const char* module, const QString& message) { if (Rank(QString::fromLatin1(level)) < Rank(g_state.level)) return; const QString line = QStringLiteral("%1 [%2] %3: %4\n").arg(QDateTime::currentDateTime().toString(QStringLiteral("yyyy-MM-dd HH:mm:ss")), QString::fromLatin1(level), QString::fromLatin1(module), message); if (g_state.handle != nullptr) { g_state.handle->write(line.toUtf8()); g_state.handle->flush(); } ++g_state.lines; if (g_state.max_lines > 0 && g_state.lines >= g_state.max_lines) ArchiveLocked(); } // 已持锁写入
}
void logging::Init() { const QString exe_dir = ExeDir(); const QString dev_root = DevRoot(exe_dir); const bool dev = !dev_root.isEmpty(); QString static_dir = dev ? dev_root + QStringLiteral("/src-tauri/static") : PackagedStaticDir(exe_dir); const QString root = dev ? dev_root : (static_dir.isEmpty() ? exe_dir : QFileInfo(static_dir).absolutePath()); QJsonObject logging_obj; QJsonObject app_obj; if (!static_dir.isEmpty()) { QFile config(static_dir + QStringLiteral("/config.json")); if (config.open(QIODevice::ReadOnly)) { const QJsonObject top = QJsonDocument::fromJson(config.readAll()).object(); logging_obj = top.value(QStringLiteral("logging")).toObject(); app_obj = top.value(QStringLiteral("app")).toObject(); } } const QString dir_value = logging_obj.value(QStringLiteral("dir")).toString().trimmed(); const QString level_env = Env("LOG_LEVEL"); const QString level_cfg = app_obj.value(QStringLiteral("log_level")).toString().trimmed(); const QString level = !level_env.isEmpty() ? level_env : (!level_cfg.isEmpty() ? level_cfg : QStringLiteral("INFO")); const QString dir_env = Env("LOG_DIR"); QString logs; if (!dir_env.isEmpty()) logs = dir_env; else if (dir_value.isEmpty()) logs = static_dir.isEmpty() ? exe_dir + QStringLiteral("/logs") : static_dir + QStringLiteral("/logs"); else logs = QFileInfo(dir_value).isAbsolute() ? dir_value : QDir(root).absoluteFilePath(dir_value); { std::lock_guard<std::mutex> lock(g_lock); g_state.dir = QDir::cleanPath(logs); g_state.file = g_state.dir + QLatin1Char('/') + QString::fromLatin1(kLogName); g_state.level = level; g_state.max_lines = qMax(logging_obj.value(QStringLiteral("max_lines")).toInt(kDefaultMaxLines), kMinMaxLines); g_state.lines = 0; QDir().mkpath(g_state.dir); OpenFile(); } if (static_dir.isEmpty()) logging::Write("WARNING", "logging", QStringLiteral("未探测到 static/config.json，日志目录回退 %1").arg(QDir::cleanPath(logs))); logging::Write("INFO", "logging", QStringLiteral("日志初始化完成：目录=%1，级别=%2，阈值=%3，根=%4，模式=%5").arg(QDir::cleanPath(logs), level).arg(g_state.max_lines).arg(root, dev ? QStringLiteral("开发态") : QStringLiteral("打包态"))); }
void logging::Write(const char* level, const char* module, const QString& message) { std::lock_guard<std::mutex> lock(g_lock); WriteLocked(level, module, message); } // 一行一锁：避免多线程日志交错
QString logging::LogFile() { return g_state.file; } // 当前日志文件
QString logging::Level() { return g_state.level; } // 当前级别
