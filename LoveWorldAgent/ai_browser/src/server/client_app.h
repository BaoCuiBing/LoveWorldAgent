// ClientApp：CEF 应用对象。浏览器进程上下文就绪后创建面板窗口（AppWindow）。
#pragma once // 防止头文件重复包含
#include <windows.h> // HWND/RECT
#include <string> // 标准字符串
#include "include/cef_app.h" // CefApp/CefBrowserProcessHandler
// 启动参数（由 main.cpp 解析，全局共享；部分字段由 --settings 指向的 JSON 覆盖）
struct AppOptions { HWND parent = nullptr; RECT rect = {0, 0, 480, 720}; bool has_rect = false; std::wstring pipe_name; std::wstring start_url = L"https://www.bing.com"; std::wstring cache_path; std::wstring root_cache_path; std::wstring settings_path; std::wstring user_agent; std::wstring default_browser = L"builtin"; int zoom_percent = 100; bool keep_focus = false; bool auto_screenshot = true; }; // settings_path=主程序下发设置；root_cache_path=CEF 根缓存（GPU/Dawn 落点）；user_agent 空则用 CEF 默认 UA
extern AppOptions g_options; // 全局启动参数
class ClientApp : public CefApp, public CefBrowserProcessHandler { // CEF 应用与浏览器进程处理器
public:
    ClientApp(); // 构造
    CefRefPtr<CefBrowserProcessHandler> GetBrowserProcessHandler() override { return this; } // 自身即浏览器进程处理器
    void OnContextInitialized() override; // 上下文就绪：创建 Qt 面板窗口
    static HWND PanelHandle() { return panel_hwnd_; } // 面板窗口句柄：供 PipeClient::Start 之后补设管道目标
    IMPLEMENT_REFCOUNTING(ClientApp); // CEF 引用计数
    DISALLOW_COPY_AND_ASSIGN(ClientApp); // 禁止拷贝
private:
    static HWND panel_hwnd_; // 已创建面板窗口的原生句柄
};
