// 进程内 CDP（Chrome DevTools Protocol）客户端：经 CefBrowserHost::ExecuteDevToolsMethod 发方法调用，用 CefDevToolsMessageObserver 收结果与事件。
// 按 CEF 文档使用这三个接口不需要 DevTools 前端与远程调试端口；所有方法只能在 CEF UI 线程（= 面板窗口线程）调用。
#pragma once // 防止头文件重复包含
#include <functional> // 回调
#include <map> // 挂起调用表
#include <string> // 标准字符串
#include <vector> // 环形缓冲
#include "include/cef_browser.h" // CefBrowserHost
#include "include/cef_devtools_message_observer.h" // 消息观察者
#include "include/cef_registration.h" // 注册句柄
#include "include/cef_values.h" // CEF 值类型
std::string ConsoleLevelName(cef_log_severity_t level); // 控制台日志级别名（error / warning / info / debug）
struct ConsoleEntry { std::string level; std::string text; std::string source; int line = 0; }; // 控制台日志（由 AppHandler::OnConsoleMessage 灌入，避免常开 Runtime domain）
struct NetworkEntry { std::string request_id; std::string url; std::string method; std::string type; std::string error; int status = 0; double start_ts = 0.0; double end_ts = 0.0; }; // 网络请求记录（由 CDP Network domain 事件聚合）
using CdpCallback = std::function<void(bool ok, CefRefPtr<CefDictionaryValue> result, const std::string& error)>; // 方法调用回调：ok=false 时 error 为 CDP 错误文本
using EvalCallback = std::function<void(bool ok, CefRefPtr<CefValue> value, const std::string& error)>; // Runtime.evaluate 回调：value 为 returnByValue 后的结果
class CdpClient : public CefDevToolsMessageObserver { // CDP 客户端
public:
    explicit CdpClient(CefRefPtr<CefBrowser> browser); // 绑定浏览器
    void Attach(); // 注册观察者（订阅事件必需）
    void Call(const std::string& method, CefRefPtr<CefDictionaryValue> params, CdpCallback cb); // 发 CDP 方法调用
    void Evaluate(const std::string& expression, bool await_promise, EvalCallback cb); // Runtime.evaluate 封装
    void Screenshot(const std::string& format, bool full_page, CefRefPtr<CefDictionaryValue> clip, CdpCallback cb); // Page.captureScreenshot 封装
    void DispatchMouse(CefRefPtr<CefDictionaryValue> params, CdpCallback cb); // Input.dispatchMouseEvent
    void DispatchKey(CefRefPtr<CefDictionaryValue> params, CdpCallback cb); // Input.dispatchKeyEvent
    void InsertText(const std::string& text, CdpCallback cb); // Input.insertText
    void EnableNetwork(); // 按需订阅 Network domain（只开一次，事件进环形缓冲）
    void AddConsole(const ConsoleEntry& entry); // 控制台缓冲追加（环形，超出上限丢弃最旧）
    const std::vector<ConsoleEntry>& console() const { return console_; } // 控制台只读访问
    const std::vector<NetworkEntry>& network() const { return network_; } // 网络记录只读访问
    bool network_enabled() const { return network_enabled_; } // Network domain 是否已开启
    bool OnDevToolsMessage(CefRefPtr<CefBrowser> browser, const void* message, size_t message_size) override; // CDP 消息回调：分发事件与响应
    IMPLEMENT_REFCOUNTING(CdpClient); // CEF 引用计数
    DISALLOW_COPY_AND_ASSIGN(CdpClient); // 禁止拷贝
private:
    void HandleEvent(const std::string& method, CefRefPtr<CefDictionaryValue> params); // Network 事件聚合
    void PurgeStale(); // 清理超时的挂起调用
    static CefRefPtr<CefDictionaryValue> EvalParams(const std::string& expression, bool await_promise); // 构造 evaluate 参数
    CefRefPtr<CefBrowser> browser_; // 目标浏览器
    CefRefPtr<CefRegistration> registration_; // 观察者注册句柄
    std::map<int, CdpCallback> pending_; // 挂起调用表
    std::map<int, unsigned long long> pending_tick_; // 挂起调用发起时刻（超时判定）
    int next_id_ = 1; // CDP 自增 id
    bool network_enabled_ = false; // Network domain 开关
    std::vector<ConsoleEntry> console_; // 控制台环形缓冲
    std::vector<NetworkEntry> network_; // 网络记录环形缓冲
    std::map<std::string, size_t> network_index_; // requestId → 数组下标
};
