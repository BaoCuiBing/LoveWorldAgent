// browser_navigate 实现：缺协议时补 https://，通过 WaitForHelper 等待主框架 OnLoadEnd/OnLoadError/OnJSDialog（有界 ~7s）后经 FinishAction 回包
#include "browser_navigate.h" // 自身声明
#include <cctype> // tolower
#include <cstring> // strlen
#include "include/cef_browser.h" // CefFrame
#include "browser_tool/tool_helpers.h" // NeedString/StateOf/FinishAction
#include "browser_tool/wait_helper.h" // WaitForNavigation
#include "server/app_handler.h" // nav_seq
#include "server/cdp_client.h" // CdpClient (ToolRequest copy/destroy in lambda)
#include "uis/app_window.h" // Tabs()
#include "uis/tab_manager.h" // BrowserTab
#include "utils/json_util.h" // Utf8ToWide
namespace {
bool StartsWithIgnoreCase(const std::string& s, const char* prefix) {
    const size_t n = std::strlen(prefix);
    if (s.size() < n) return false;
    for (size_t i = 0; i < n; ++i) {
        if (std::tolower(static_cast<unsigned char>(s[i])) != std::tolower(static_cast<unsigned char>(prefix[i]))) return false;
    }
    return true;
}
bool HasSchemeOrSpecial(const std::string& url) {
    return url.find("://") != std::string::npos
        || StartsWithIgnoreCase(url, "about:")
        || StartsWithIgnoreCase(url, "data:")
        || StartsWithIgnoreCase(url, "blob:")
        || StartsWithIgnoreCase(url, "file:")
        || StartsWithIgnoreCase(url, "javascript:");
}
}  // namespace
void BrowserNavigateTool::Run(const ToolRequest& req) {
    std::string url;
    if (!tool::NeedString(req, "url", url)) return;
    if (!HasSchemeOrSpecial(url)) url = "https://" + url;
    CefRefPtr<AppHandler> handler = req.tab != nullptr ? req.tab->handler : nullptr;
    CefRefPtr<CefBrowser> browser = req.tab != nullptr ? req.tab->browser : nullptr;
    if (browser == nullptr || handler == nullptr) { tool::ReplyError(req.id, "无可用标签页"); return; }
    const uint64_t prev_seq = handler->nav_seq();
    browser->GetMainFrame()->LoadURL(CefString(Utf8ToWide(url)));
    ToolRequest safe_req = req;
    safe_req.tab = nullptr;
    WaitForHelper::WaitForNavigation(handler, prev_seq, 7000, [safe_req, browser]() {
        BrowserTab* cur_tab = (safe_req.window != nullptr && safe_req.window->Tabs() != nullptr) ? safe_req.window->Tabs()->Find(browser) : nullptr;
        ToolRequest next_req = safe_req;
        next_req.tab = cur_tab;
        if (cur_tab != nullptr && cur_tab->cdp != nullptr) next_req.cdp = cur_tab->cdp;
        tool::FinishAction(next_req, tool::StateOf(cur_tab), false);
    });
}
