// 浏览器工具统一接口：一个工具一个类，工具名与 mds/浏览器工具.md 的 browser_* 一一对应；ToolRequest 由 AgentApi 在 UI 线程组装后交给具体工具执行
#pragma once // 防止头文件重复包含
#include <string> // 标准字符串
#include "include/cef_values.h" // CEF 值类型
class AppWindow; // 前置声明
class CdpClient; // 前置声明
struct BrowserTab; // 前置声明
struct ToolRequest { int id = 0; std::string cmd; CefRefPtr<CefDictionaryValue> args; AppWindow* window = nullptr; BrowserTab* tab = nullptr; CefRefPtr<CdpClient> cdp; }; // 工具请求：指令名、参数、窗口、活动标签与其 CDP 会话
class IBrowserTool { // 工具抽象基类
public:
    virtual ~IBrowserTool() = default; // 虚析构
    virtual const char* name() const = 0; // 指令名，等于工具名（如 browser_click）
    virtual void Run(const ToolRequest& req) = 0; // 执行并自行经 PipeClient 回响应（异步回调里回包也算完成）
};
