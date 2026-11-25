// WaitForHelper：对齐 chrome-devtools-mcp 的导航/弹窗守卫/MutationObserver 稳定等待
#pragma once
#include <cstdint>
#include <functional>
#include "browser_tool/browser_tool.h"
#include "include/cef_base.h"
class AppHandler;
struct BrowserTab;
class WaitForHelper {
public:
    static void WaitForNavigation(BrowserTab* tab, uint64_t prev_seq, int timeout_ms, std::function<void()> done);
    static void WaitForNavigation(CefRefPtr<AppHandler> handler, uint64_t prev_seq, int timeout_ms, std::function<void()> done);
    static void Settle(const ToolRequest& req, bool maybe_navigates, std::function<void(bool dialog_pending)> done);
};
