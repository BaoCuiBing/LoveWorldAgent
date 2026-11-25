// browser_press_key 实现：可见窗口在 CEF UI 线程走 SendNativeKey（支持组合键 Ctrl+a / F1-F12 / 数字 / 标点，isTrusted=true），隐藏/最小化窗口走 JS KeyboardEvent 兜底，经 FinishAction 支持 includeSnapshot
#include "browser_press_key.h" // 自身声明
#include "browser_tool/input_helper.h" // SendNativeKey
#include "browser_tool/tool_helpers.h" // NeedString/ReplyError/FinishAction
#include "server/cdp_client.h" // Evaluate fallback
#include "uis/app_window.h" // IsVisible/Tabs
#include "uis/tab_manager.h" // BrowserTab
#include "utils/js_builder.h" // js::QueryKey/Literal
#include "utils/json_util.h" // MakeDict
void BrowserPressKeyTool::Run(const ToolRequest& req) {
    std::string key;
    if (!tool::NeedString(req, "key", key)) return;
    js::KeyInfo info;
    if (!js::QueryKey(key, info)) { tool::ReplyError(req.id, "无法识别的按键: " + key); return; }

    CefRefPtr<CefBrowser> browser = req.tab != nullptr ? req.tab->browser : nullptr;
    if (req.window != nullptr && req.window->IsVisible() && browser != nullptr && browser->GetHost() != nullptr) {
        input::SendNativeKey(browser, info, info.modifiers, info.printable);
        CefRefPtr<CefDictionaryValue> out = MakeDict();
        out->SetString("pressed", key);
        tool::FinishAction(req, out, true);
        return;
    }

    const bool ctrl = (info.modifiers & EVENTFLAG_CONTROL_DOWN) != 0;
    const bool alt = (info.modifiers & EVENTFLAG_ALT_DOWN) != 0;
    const bool shift = (info.modifiers & EVENTFLAG_SHIFT_DOWN) != 0;
    const bool meta = (info.modifiers & EVENTFLAG_COMMAND_DOWN) != 0;
    const std::string script =
        "(function(){var el=document.activeElement||document.body;"
        "var o={key:" + js::Literal(info.name) + ",code:" + js::Literal(info.code) + ",keyCode:" + std::to_string(info.vk) + ",which:" + std::to_string(info.vk) + ","
        "ctrlKey:" + std::string(ctrl ? "true" : "false") + ",altKey:" + (alt ? "true" : "false") + ","
        "shiftKey:" + (shift ? "true" : "false") + ",metaKey:" + (meta ? "true" : "false") + ",bubbles:true,cancelable:true};"
        "el.dispatchEvent(new KeyboardEvent('keydown',o));"
        "el.dispatchEvent(new KeyboardEvent('keypress',o));"
        "el.dispatchEvent(new KeyboardEvent('keyup',o));"
        "return o.key;})()";
    ToolRequest safe_req = req;
    safe_req.tab = nullptr;
    safe_req.cdp->Evaluate(script, false, [safe_req, browser, key](bool ok, CefRefPtr<CefValue>, const std::string& error) {
        if (!ok) { tool::ReplyError(safe_req.id, error.empty() ? "按键失败" : error); return; }
        CefRefPtr<CefDictionaryValue> out = MakeDict();
        out->SetString("pressed", key);
        BrowserTab* cur_tab = (safe_req.window != nullptr && safe_req.window->Tabs() != nullptr && browser != nullptr) ? safe_req.window->Tabs()->Find(browser) : nullptr;
        ToolRequest next_req = safe_req;
        next_req.tab = cur_tab;
        if (cur_tab != nullptr && cur_tab->cdp != nullptr) next_req.cdp = cur_tab->cdp;
        tool::FinishAction(next_req, out, true);
    });
}
