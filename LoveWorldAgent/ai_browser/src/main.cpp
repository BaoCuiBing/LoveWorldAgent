#include <windows.h>
#include <shobjidl.h>
#include <shellscalingapi.h>
#include <fstream>
#include <sstream>
#include <string>
#include <QApplication>
#include <QDir> // 图标路径拼接
#include <QFile> // 图标文件存在件判定
#include <QIcon> // 窗口图标
#include <QImage> // 内嵌图标转 QImage（fromHICON）
#include <QPixmap> // QImage → QPixmap 中转后才能构造 QIcon
#include <QTimer>
#include "resource.h" // IDI_APPICON（include 根已含 resources 目录）
#include "uis/qt_app_window.h"
#include "server/client_app.h"
#include "server/pipe_client.h"
#include "utils/json_util.h"
#include "utils/logging.h" // 文件日志（static/logs/browser_panel.log）
namespace {
void EnableDpiAwareness() { // 每显示器 DPI 感知（缺 shcore 静默跳过）
    HINSTANCE shcore = LoadLibraryW(L"shcore.dll"); // 动态加载 shcore
    if (shcore) {
        using SetProcessDpiAwareness_t = HRESULT(WINAPI*)(PROCESS_DPI_AWARENESS); // 函数指针类型
        auto SetProcessDpiAwareness = reinterpret_cast<SetProcessDpiAwareness_t>(GetProcAddress(shcore, "SetProcessDpiAwareness")); // 取函数地址
        if (SetProcessDpiAwareness) SetProcessDpiAwareness(PROCESS_PER_MONITOR_DPI_AWARE); // 声明 DPI 感知
        FreeLibrary(shcore); // 释放库
    }
}
void ApplyWindowIcon(QApplication& app) { // 标题栏与任务栏图标：.rc 里的 IDI_APPICON 只决定 exe 文件图标，窗口图标必须由 Qt 单独下发
    const QString file = QDir(QCoreApplication::applicationDirPath()).filePath(QStringLiteral("resources/dragon.ico")); // exe 同级随包图标
    if (QFile::exists(file)) { app.setWindowIcon(QIcon(file)); return; } // 命中则直接用 ICO（CMake POST_BUILD 已把 resources 整体拷到 exe 同级）
    HICON icon = LoadIconW(GetModuleHandleW(nullptr), MAKEINTRESOURCEW(IDI_APPICON)); // 兜底：读 exe 内嵌图标
    if (icon) app.setWindowIcon(QIcon(QPixmap::fromImage(QImage::fromHICON(icon)))); // 内嵌 HICON 经 QImage→QPixmap→QIcon（QIcon 无 QImage 直接构造）；两者都失败则保持默认，不打噪声
}
void ParseOptions(AppOptions& options) { // 解析命令行 --xxx= 参数
    const std::wstring cmd = GetCommandLineW(); // 完整命令行（含 exe 路径）
    const std::wstring parent_flag = L"--parent="; const std::wstring rect_flag = L"--rect="; const std::wstring pipe_flag = L"--pipe="; const std::wstring url_flag = L"--url="; const std::wstring cache_flag = L"--cache="; const std::wstring root_cache_flag = L"--root-cache="; const std::wstring settings_flag = L"--settings="; const std::wstring ua_flag = L"--user-agent="; const std::wstring browser_flag = L"--default-browser="; const std::wstring zoom_flag = L"--zoom="; const std::wstring size_flag = L"--size="; // 参数前缀表
    size_t pos = 0; // 扫描位置
    while ((pos = cmd.find(L"--", pos)) != std::wstring::npos) { // 逐段提取 -- 开头的参数
        size_t end = cmd.find(L" ", pos); std::wstring arg = cmd.substr(pos, end - pos); // 取到下一个空格为一段
        if (arg.rfind(parent_flag, 0) == 0) { // 父窗口句柄（仅记录，不再贴子窗口）
            std::wstring v = arg.substr(parent_flag.size());
            options.parent = reinterpret_cast<HWND>(static_cast<intptr_t>(_wcstoui64(v.c_str(), nullptr, 0)));
        }
        else if (arg.rfind(rect_flag, 0) == 0) { // --rect=x,y,w,h 四元组
            std::wstring v = arg.substr(rect_flag.size()); // 取值
            int vals[4] = {0, 0, 0, 0}; int i = 0; size_t p = 0; // 逐段取数
            while (i < 4 && p < v.size()) { size_t c = v.find(L",", p); vals[i++] = std::stoi(v.substr(p, c - p)); p = (c == std::wstring::npos) ? v.size() : c + 1; } // 逗号分隔解析
            options.has_rect = true; options.rect = {vals[0], vals[1], vals[2], vals[3]}; // 记录矩形
        } else if (arg.rfind(pipe_flag, 0) == 0) options.pipe_name = arg.substr(pipe_flag.size()); // 管道名（不含 \\.\pipe\ 前缀）
        else if (arg.rfind(url_flag, 0) == 0) options.start_url = arg.substr(url_flag.size()); // 起始页
        else if (arg.rfind(cache_flag, 0) == 0) options.cache_path = arg.substr(cache_flag.size()); // CEF 缓存目录
        else if (arg.rfind(root_cache_flag, 0) == 0) options.root_cache_path = arg.substr(root_cache_flag.size()); // CEF 根缓存目录（GPU/Dawn）
        else if (arg.rfind(settings_flag, 0) == 0) options.settings_path = arg.substr(settings_flag.size()); // 主程序下发设置 JSON
        else if (arg.rfind(ua_flag, 0) == 0) options.user_agent = arg.substr(ua_flag.size()); // 自定义 UA
        else if (arg.rfind(browser_flag, 0) == 0) options.default_browser = arg.substr(browser_flag.size()); // 默认浏览器标记
        else if (arg.rfind(zoom_flag, 0) == 0) options.zoom_percent = std::stoi(arg.substr(zoom_flag.size())); // 初始缩放
        else if (arg.rfind(size_flag, 0) == 0) { // --size=WxH（等价 rect）
            std::wstring v = arg.substr(size_flag.size()); int w = 0, h = 0; size_t c = v.find(L"x"); // 取宽高
            if (c != std::wstring::npos) { w = std::stoi(v.substr(0, c)); h = std::stoi(v.substr(c + 1)); } // 按小写 x 分隔
            options.has_rect = true; options.rect = {100, 100, 100 + w, 100 + h}; // 换算为矩形
        }
        pos = (end == std::wstring::npos) ? std::wstring::npos : end; // 继续下一段
    }
    if (options.start_url.empty()) options.start_url = L"https://www.bing.com"; // 起始页缺省
    if (options.cache_path.empty()) options.cache_path = L"./additional/browser_cache"; // 缓存缺省
    if (options.root_cache_path.empty()) options.root_cache_path = options.cache_path; // 根缓存缺省与缓存同目录，避免 GPU/Dawn 落到工作目录
    if (options.settings_path.empty()) options.settings_path = L"./browser_settings.json"; // 设置缺省
}
void ApplySettings(AppOptions& options) { // 从 settings JSON 继承开关与缩放
    try {
        std::wifstream f(options.settings_path);
        if (f.good()) {
            std::wstringstream ss; ss << f.rdbuf();
            std::wstring content = ss.str();
            if (content.find(L"\"keep_focus\":true") != std::wstring::npos) options.keep_focus = true;
            size_t p = content.find(L"\"zoom_percent\"");
            if (p != std::wstring::npos) {
                size_t c = content.find(L":", p);
                size_t e = content.find(L",", c);
                if (e == std::wstring::npos) e = content.find(L"}", c);
                options.zoom_percent = std::stoi(content.substr(c + 1, e - c - 1));
            }
        }
    } catch (...) { /* 忽略配置读取错误，使用默认值 */ }
}
}  // namespace
int main(int argc, char* argv[]) { // 启动链：DPI → AUMID → Qt App → CEF → 窗口 → 消息泵 → 管道 → 事件循环
    EnableDpiAwareness(); // 先声明 DPI 感知再建窗口
    ::SetCurrentProcessExplicitAppUserModelID(L"com.loveworld.agent"); // 与主应用共用 AppUserModelID 实现任务栏双窗分组
    QApplication app(argc, argv); // Qt 应用（Qt6 与 CEF 统一 /MD）
    ApplyWindowIcon(app); // 先于建窗设置应用图标，子窗口继承
    CefMainArgs main_args(GetModuleHandle(nullptr)); // 进程内 CEF 参数
    CefRefPtr<ClientApp> client_app(new ClientApp); // CEF 应用对象
    int exit_code = CefExecuteProcess(main_args, client_app.get(), nullptr); // 副进程走 CEF 子进程逻辑
    if (exit_code >= 0) return exit_code; // 子进程到此返回
    logging::Init(); // 仅主进程初始化文件日志：定位根目录、读取主应用 logging 配置、按阈值归档（子进程不写同一文件）
    ParseOptions(g_options); // 解析启动参数
    ApplySettings(g_options); // 叠加 settings JSON
    LOG_INFO("main", QStringLiteral("启动 browser_panel：管道=%1，起始页=%2，缓存=%3，缩放=%4").arg(QString::fromStdWString(g_options.pipe_name), QString::fromStdWString(g_options.start_url), QString::fromStdWString(g_options.cache_path)).arg(g_options.zoom_percent)); // 启动参数留痕
    CefSettings settings; // CEF 配置
    CefString(&settings.cache_path) = g_options.cache_path; // 缓存目录
    CefString(&settings.root_cache_path) = g_options.root_cache_path; // 根缓存目录（GPUCache/DawnCache 落点，固定到 cache）
    CefString(&settings.user_agent) = g_options.user_agent; // UA（空则默认）
    settings.no_sandbox = true; // 关沙箱（桌面 sidecar 必需）
    settings.multi_threaded_message_loop = false; // 自管消息循环（QTimer 泵）
    settings.log_severity = LOGSEVERITY_DISABLE; // 关 CEF 日志
    CefInitialize(main_args, settings, client_app.get(), nullptr); // 初始化 CEF（触发 OnContextInitialized 建 Qt 窗口）
    QtAppWindow* win = QtAppWindow::Create(); // 创建 Qt 主窗口（标题固定「内置浏览器」）
    if (!win) { LOG_ERROR("main", QStringLiteral("创建主窗口失败，进程退出")); CefShutdown(); return 1; } // 窗口失败则退出
    QTimer timer; // CEF 消息泵
    QObject::connect(&timer, &QTimer::timeout, []() { CefDoMessageLoopWork(); }); // 每 10ms 泵一次（替代 DuiLib 的 WM_TIMER）
    timer.start(10);
    if (!g_options.pipe_name.empty()) { PipeClient::Start(g_options.pipe_name); LOG_INFO("main", QStringLiteral("管道客户端已启动：%1").arg(QString::fromStdWString(g_options.pipe_name))); } // 启动管道客户端
    else LOG_WARN("main", QStringLiteral("未指定 --pipe，sidecar 仅可独立调试")); // 无管道提示
    QTimer::singleShot(0, win, [win]() { win->Tabs()->AddTab(g_options.start_url, -1); }); // 事件循环启动后立即异步建首标签（OnContextInitialized 内不能建页）
    const int rc = app.exec(); // Qt 事件循环
    LOG_INFO("main", QStringLiteral("事件循环结束，退出码=%1").arg(rc)); // 退出留痕
    PipeClient::Stop(); // 停管道
    CefShutdown(); // 关 CEF
    return rc; // 返回退出码
}
