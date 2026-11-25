// 浏览器外壳接口：原 DuiLib AppWindow 抽象为纯接口，具体实现见 uis/qt_app_window.h 的 QtAppWindow；TabManager / AgentApi / UiChrome 仅依赖此接口，不感知 UI 框架
#pragma once // 防止头文件重复包含
#include <windows.h> // HWND/RECT
#include <functional> // std::function
#include <string> // 标准字符串
#include <vector> // 标准容器
#include "include/cef_browser.h" // CefBrowser
#include "server/agent_api.h" // AgentApi
#include "ui_chrome.h" // UiChrome
#include "tab_manager.h" // TabManager
class CdpClient; // 前置声明
class AppWindow { // 外壳纯虚接口
public:
    virtual ~AppWindow() = default; // 虚析构
    virtual HWND hwnd() const = 0; // 原生窗口句柄
    virtual HWND HostHwnd() const = 0; // CEF 承载区原生句柄（浏览器子窗口的父级）
    virtual TabManager* Tabs() = 0; // 标签管理器
    virtual AgentApi* Api() = 0; // 指令入口
    virtual UiChrome* Chrome() = 0; // 外壳动作集
    virtual bool IsVisible() const = 0; // 窗口是否可见
    virtual RECT BrowserArea() const = 0; // CEF 内容区相对承载区的矩形
    virtual CefRefPtr<CefBrowser> ActiveBrowser() = 0; // 活动标签浏览器
    virtual CefRefPtr<CdpClient> ActiveCdp() = 0; // 活动标签 CDP
    virtual std::wstring ActiveUrl() = 0; // 活动标签地址
    virtual void SetFindVisible(bool visible) = 0; // 查找条显隐
    virtual void SetFindText(const std::wstring& text) = 0; // 写入查找词
    virtual bool find_visible() const = 0; // 查找条是否可见
    virtual void BeginBrowserClose() = 0; // 抑制 CloseBrowser 引发的宿主 WM_CLOSE 误退出
    virtual void EndBrowserClose() = 0; // 解除关闭抑制
    virtual std::wstring FindInputText() const = 0; // 读取查找输入框
    virtual void SyncZoomUi(int percent) = 0; // 同步缩放值到 UI
    virtual void Defer(std::function<void()> task) = 0; // 投递到 UI 线程执行
    virtual void OnBrowserCreated(CefRefPtr<CefBrowser> browser, CefRefPtr<AppHandler> handler) = 0; // 浏览器创建回调
    virtual void OnBrowserClosed(CefRefPtr<CefBrowser> browser) = 0; // 浏览器关闭回调
    virtual void OnTitle(CefRefPtr<CefBrowser> browser, const std::wstring& title) = 0; // 标题变化
    virtual void OnAddress(CefRefPtr<CefBrowser> browser, const std::wstring& address) = 0; // 地址变化
    virtual void OnLoadingState(CefRefPtr<CefBrowser> browser, bool loading, bool can_go_back, bool can_go_forward) = 0; // 加载状态
    virtual void OnConsole(CefRefPtr<CefBrowser> browser, cef_log_severity_t level, const std::string& text, const std::string& source, int line) = 0; // 控制台消息
    virtual void OpenPopup(const std::wstring& url) = 0; // 弹窗转新标签
    virtual void OnTabsChanged() = 0; // 标签集变化（同步 UI 与上报）
    virtual void OnAllTabsClosed() = 0; // 全部标签关闭
    virtual void ScheduleTabRestore() = 0; // 补建起始页标签（经消息循环）
    virtual void ShowSelf(bool visible) = 0; // 显示/隐藏窗口
    virtual void FocusSelf() = 0; // 聚焦窗口
    virtual void CloseSelf() = 0; // 仅隐藏（保活）
    virtual void HideSelf() = 0; // 隐藏
    virtual void QuitSelf() = 0; // 真正退出进程
    virtual void SetRect(const RECT& rect) = 0; // 设置窗口矩形
    virtual void ToggleFullscreen() = 0; // 切全屏
    virtual void ShowAgentCursor(double x, double y, double vw, double vh, bool click) = 0; // 在页面视口坐标处显示模拟鼠标（click 播放涟漪）
    virtual void HideAgentCursor() = 0; // 隐藏模拟鼠标
};
#ifndef LWA_APP_COMMANDS // 防重定义
#define LWA_APP_COMMANDS // 消息宏守卫
#define WM_APP_COMMAND (WM_APP + 4) // 指令消息：lParam 携带 CmdRequest*
#define WM_APP_ENSURE_TAB (WM_APP + 5) // 补建一个空标签
#define WM_APP_DEFER (WM_APP + 6) // UI 线程回调：lParam 携带 std::function*
#endif
