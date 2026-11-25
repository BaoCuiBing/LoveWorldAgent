// browser_navigate_back 实现
#include "browser_navigate_back.h" // 自身声明
#include "include/cef_browser.h" // CefBrowser
#include "browser_tool/tool_helpers.h" // StateOf/Reply
#include "uis/tab_manager.h" // BrowserTab
void BrowserNavigateBackTool::Run(const ToolRequest& req) { req.tab->browser->GoBack(); tool::Reply(req.id, tool::StateOf(req.tab)); } // 后退并回状态
