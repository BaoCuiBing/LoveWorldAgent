// AppHandler：CEF 客户端回调集合。覆盖：生命周期、标题/地址/控制台、加载状态、请求拦截、JS 弹窗接管、下载记录。
#pragma once // 防止头文件重复包含
#include <cstdint> // uint64_t
#include <functional> // std::function
#include <string> // 标准字符串
#include <vector> // 导航等待回调
#include "include/cef_client.h" // CefClient
#include "include/cef_download_handler.h" // 下载回调
#include "include/cef_jsdialog_handler.h" // JS 弹窗回调
class AppWindow; // 前置声明
class AppHandler : public CefClient, public CefLifeSpanHandler, public CefDisplayHandler, public CefLoadHandler, public CefRequestHandler, public CefJSDialogHandler, public CefDownloadHandler { // 聚合各 CEF 处理器
public:
    explicit AppHandler(AppWindow* window); // 绑定面板窗口
    CefRefPtr<CefLifeSpanHandler> GetLifeSpanHandler() override { return this; } // 生命周期
    CefRefPtr<CefDisplayHandler> GetDisplayHandler() override { return this; } // 标题/地址/控制台
    CefRefPtr<CefLoadHandler> GetLoadHandler() override { return this; } // 加载状态
    CefRefPtr<CefRequestHandler> GetRequestHandler() override { return this; } // 请求拦截
    CefRefPtr<CefJSDialogHandler> GetJSDialogHandler() override { return this; } // JS 弹窗
    CefRefPtr<CefDownloadHandler> GetDownloadHandler() override { return this; } // 下载
    void OnAfterCreated(CefRefPtr<CefBrowser> browser) override; // 首个浏览器创建后登记并通知窗口
    void OnBeforeClose(CefRefPtr<CefBrowser> browser) override; // 浏览器关闭前清理并通知窗口
    bool OnBeforePopup(CefRefPtr<CefBrowser> browser, CefRefPtr<CefFrame> frame, const CefString& target_url, const CefString& target_frame_name, cef_window_open_disposition_t target_disposition, bool user_gesture, const CefPopupFeatures& popupFeatures, CefWindowInfo& windowInfo, CefRefPtr<CefClient>& client, CefBrowserSettings& settings, CefRefPtr<CefDictionaryValue>& extra_info, bool* no_javascript_access) override; // 弹窗拦截：统一转面板内新标签
    void OnTitleChange(CefRefPtr<CefBrowser> browser, const CefString& title) override; // 标题变化
    void OnAddressChange(CefRefPtr<CefBrowser> browser, CefRefPtr<CefFrame> frame, const CefString& url) override; // 主框架地址变化
    bool OnConsoleMessage(CefRefPtr<CefBrowser> browser, cef_log_severity_t level, const CefString& message, const CefString& source, int line) override; // 控制台消息采集
    void OnLoadingStateChange(CefRefPtr<CefBrowser> browser, bool isLoading, bool canGoBack, bool canGoForward) override; // 加载状态
    void OnLoadEnd(CefRefPtr<CefBrowser> browser, CefRefPtr<CefFrame> frame, int httpStatusCode) override; // 主框架加载完成
    void OnLoadError(CefRefPtr<CefBrowser> browser, CefRefPtr<CefFrame> frame, ErrorCode errorCode, const CefString& errorText, const CefString& failedUrl) override; // 主框架加载失败
    bool OnBeforeBrowse(CefRefPtr<CefBrowser> browser, CefRefPtr<CefFrame> frame, CefRefPtr<CefRequest> request, bool user_gesture, bool is_redirect) override; // 导航一律放行
    bool OnOpenURLFromTab(CefRefPtr<CefBrowser> browser, CefRefPtr<CefFrame> frame, const CefString& target_url, cef_window_open_disposition_t target_disposition, bool user_gesture) override; // target=_blank 转新标签
    bool OnJSDialog(CefRefPtr<CefBrowser> browser, const CefString& origin_url, JSDialogType dialog_type, const CefString& message_text, const CefString& default_prompt_text, CefRefPtr<CefJSDialogCallback> callback, bool& suppress_message) override; // 弹窗接管：受控模式下转管道事件
    bool OnBeforeUnloadDialog(CefRefPtr<CefBrowser> browser, const CefString& message_text, bool isReload, CefRefPtr<CefJSDialogCallback> callback) override; // beforeunload 直接放行
    void OnResetDialogState(CefRefPtr<CefBrowser> browser) override; // 弹窗状态复位
    void OnBeforeDownload(CefRefPtr<CefBrowser> browser, CefRefPtr<CefDownloadItem> download_item, const CefString& suggested_name, CefRefPtr<CefBeforeDownloadCallback> callback) override; // 记录下载并沿用默认路径
    void OnDownloadUpdated(CefRefPtr<CefBrowser> browser, CefRefPtr<CefDownloadItem> download_item, CefRefPtr<CefDownloadItemCallback> callback) override; // 下载进度转 UI 线程更新
    CefRefPtr<CefBrowser> browser() const { return browser_; } // 当前浏览器
    bool dialog_pending() const { return dialog_callback_ != nullptr; } // 是否有待应答弹窗
    std::string dialog_message() const { return dialog_message_; } // 弹窗文本
    std::string dialog_type() const { return dialog_type_; } // 弹窗类型名
    void HandleDialog(bool accept, const std::string& prompt_text); // 应答弹窗（browser_handle_dialog）
    uint64_t nav_seq() const { return nav_seq_; } // 主框架导航完成序号
    void AddNavWaiter(std::function<void()> cb); // 注册一次导航完成/弹窗中断通知
    IMPLEMENT_REFCOUNTING(AppHandler); // CEF 引用计数
    DISALLOW_COPY_AND_ASSIGN(AppHandler); // 禁止拷贝
private:
    void NotifyNavWaiters(); // 唤醒导航等待回调
    AppWindow* window_; // 宿主窗口
    CefRefPtr<CefBrowser> browser_; // 已登记浏览器
    CefRefPtr<CefJSDialogCallback> dialog_callback_; // 待应答弹窗回调
    std::string dialog_message_; // 弹窗文本缓存
    std::string dialog_type_; // 弹窗类型缓存
    uint64_t nav_seq_ = 0; // 主框架导航完成计数
    std::vector<std::function<void()>> nav_waiters_; // 一次性导航等待回调表
};
