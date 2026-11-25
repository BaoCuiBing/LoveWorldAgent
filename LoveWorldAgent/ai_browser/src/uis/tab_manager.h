// 标签页管理：面板内可同时存在多个 CefBrowser（每标签一个 AppHandler + CdpClient）；只有活动标签可见，其余隐藏以省渲染与输入
#pragma once // 防止头文件重复包含
#include <string> // 标准字符串
#include <vector> // 标签数组
#include <QIcon> // 标签页图标
#include "include/cef_browser.h" // CefBrowser
#include "server/app_handler.h" // AppHandler
class AppWindow; // 前置声明
class CdpClient; // 前置声明
struct BrowserTab { CefRefPtr<AppHandler> handler; CefRefPtr<CefBrowser> browser; CefRefPtr<CdpClient> cdp; HWND hwnd = nullptr; std::wstring title; std::wstring url; bool loading = false; bool can_back = false; bool can_forward = false; QIcon icon; }; // 单个标签的浏览器上下文；icon 为该标签网页图标（默认空，加载完成后异步填充）
class TabManager { // 标签管理器
public:
    explicit TabManager(AppWindow* window); // 绑定外壳窗口
    int AddTab(const std::wstring& url, int index); // 新建标签页；index<0 追加到末尾并激活，否则在 index 处插入且不切换活动标签
    bool Select(int index); // 切换活动标签
    bool Close(int index); // 关闭标签（浏览器异步销毁）
    int active() const { return active_; } // 活动下标
    int count() const { return static_cast<int>(tabs_.size()); } // 标签数
    BrowserTab* Active(); // 活动标签
    BrowserTab* At(int index); // 按下标取标签
    BrowserTab* Find(CefRefPtr<CefBrowser> browser); // 按浏览器找标签
    void OnBrowserCreated(CefRefPtr<CefBrowser> browser, CefRefPtr<AppHandler> handler); // CEF 生命周期回调（OnAfterCreated 可能在建页内部重入触发）
    void OnBrowserClosed(CefRefPtr<CefBrowser> browser); // 浏览器销毁回调
    void ApplyVisibility(); // 只显示活动标签窗口
    void LayoutActive(); // 按内容区矩形移动标签窗口
    void CloseAll(); // 关闭全部标签（之后的 OnBrowserClosed 不再自动补建）
    bool closing_all() const { return closing_all_; } // 是否处于关闭全部流程
    void EnsureOneTab(); // 标签数为 0 且非关闭流程时补建起始页标签（由窗口消息循环触发）
private:
    int InsertSlot(const CefRefPtr<AppHandler>& handler, const std::wstring& url, int index); // 在指定位置插入槽位
    AppWindow* window_; // 宿主窗口
    std::vector<BrowserTab> tabs_; // 标签数组
    int active_ = -1; // 活动下标
    bool closing_all_ = false; // 关闭全部标记
};
