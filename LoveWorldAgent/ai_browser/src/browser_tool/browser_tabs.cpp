// browser_tabs 实现：list/new/close/select，统一回 {active,count,tabs[]}
#include <cctype> // tolower
#include <cstring> // strlen
#include "browser_tabs.h" // 自身声明
#include "browser_tool/tool_helpers.h" // NeedString/回包
#include "uis/app_window.h" // Tabs()
#include "uis/tab_manager.h" // TabManager
#include "utils/json_util.h" // JSON 工具
namespace {
bool StartsWithIgnoreCase(const std::string& s, const char* prefix) {
    const size_t n = std::strlen(prefix);
    if (s.size() < n) return false;
    for (size_t i = 0; i < n; ++i) {
        if (std::tolower(static_cast<unsigned char>(s[i])) != std::tolower(static_cast<unsigned char>(prefix[i]))) return false;
    }
    return true;
}
std::string NormalizeUrl(std::string url) {
    if (url.empty()) return "about:blank";
    if (url.find("://") != std::string::npos
        || StartsWithIgnoreCase(url, "about:")
        || StartsWithIgnoreCase(url, "data:")
        || StartsWithIgnoreCase(url, "blob:")
        || StartsWithIgnoreCase(url, "file:")
        || StartsWithIgnoreCase(url, "javascript:")) {
        return url;
    }
    return "https://" + url;
} // 空回 about:blank，缺协议补 https（保留 data/blob/file/javascript/about）
}
void BrowserTabsTool::Run(const ToolRequest& req) { std::string action; if (!tool::NeedString(req, "action", action)) return; TabManager* tabs = req.window != nullptr ? req.window->Tabs() : nullptr; if (tabs == nullptr) { tool::ReplyError(req.id, "标签管理器不可用"); return; } int index = -1; ArgGetInt(req.args, "index", index); if (action == "new") { std::string url; ArgGetString(req.args, "url", url); if (tabs->AddTab(Utf8ToWide(NormalizeUrl(url)), index) < 0) { tool::ReplyError(req.id, "新建标签页失败"); return; } } else if (action == "close") { if (!tabs->Close(index < 0 ? tabs->active() : index)) { tool::ReplyError(req.id, "标签页序号无效"); return; } } else if (action == "select") { if (!tabs->Select(index)) { tool::ReplyError(req.id, "标签页序号无效"); return; } } else if (action != "list") { tool::ReplyError(req.id, "action 需为 list/new/close/select"); return; } CefRefPtr<CefListValue> list = MakeList(); for (int i = 0; i < tabs->count(); ++i) { BrowserTab* tab = tabs->At(i); CefRefPtr<CefDictionaryValue> item = MakeDict(); item->SetInt("index", i); item->SetString("url", WideToUtf8(tab != nullptr ? tab->url : std::wstring())); item->SetString("title", WideToUtf8(tab != nullptr ? tab->title : std::wstring())); item->SetBool("loading", tab != nullptr ? tab->loading : false); item->SetBool("canGoBack", tab != nullptr ? tab->can_back : false); item->SetBool("canGoForward", tab != nullptr ? tab->can_forward : false); item->SetBool("active", i == tabs->active()); ListAddDict(list, item); } CefRefPtr<CefDictionaryValue> data = MakeDict(); data->SetInt("active", tabs->active()); data->SetInt("count", tabs->count()); data->SetList("tabs", list); tool::Reply(req.id, data); } // 动作分流：new 归一化 URL 建标签，close 缺省关活动标签，select/index 越界报错；统一回标签清单
