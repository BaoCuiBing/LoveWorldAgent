// browser_click 实现：可见窗口走 CEF UI 线程 SendNativeClick（isTrusted=true），隐藏窗口走 PointerEvent+MouseEvent+el.click() 兜底（绝不双发），经 FinishAction 支持 includeSnapshot
#include "browser_click.h" // 自身声明
#include "browser_tool/input_helper.h" // SendNativeClick/ParseCefModifiers/ParseMouseButton
#include "browser_tool/tool_helpers.h" // NeedRef/ReplyError/FinishAction
#include "server/cdp_client.h" // Evaluate
#include "uis/app_window.h" // IsVisible/ShowAgentCursor
#include "uis/tab_manager.h" // BrowserTab
#include "utils/js_builder.h" // js::MouseButtonCode/Element
#include "utils/json_util.h" // ArgGetBool/ArgGetList/WriteJsonList/ParseJsonObject
void BrowserClickTool::Run(const ToolRequest& req) {
    std::string ref;
    if (!tool::NeedRef(req, ref)) return;
    std::string button = "left";
    ArgGetString(req.args, "button", button);
    bool double_click = false;
    ArgGetBool(req.args, "doubleClick", double_click);
    CefRefPtr<CefListValue> modifiers = ArgGetList(req.args, "modifiers");

    const std::string locate_js = js::Element(ref,
        "el.scrollIntoView({block:'center',inline:'center'});"
        "var r=el.getBoundingClientRect();"
        "if(r.width<=0&&r.height<=0){throw new Error('元素不可见或尺寸为0');}"
        "if(el.disabled===true){throw new Error('元素已禁用');}"
        "return JSON.stringify({x:Math.round(r.left+r.width/2),y:Math.round(r.top+r.height/2),vw:innerWidth,vh:innerHeight});");

    CefRefPtr<CefBrowser> browser = req.tab != nullptr ? req.tab->browser : nullptr;
    const uint32_t cef_mods = input::ParseCefModifiers(modifiers);
    const cef_mouse_button_type_t cef_btn = input::ParseMouseButton(button);
    const int btn_code = js::MouseButtonCode(button);
    const std::string mods_json = modifiers != nullptr ? WriteJsonList(modifiers) : "[]";
    ToolRequest safe_req = req;
    safe_req.tab = nullptr;

    safe_req.cdp->Evaluate(locate_js, false, [safe_req, ref, double_click, browser, cef_mods, cef_btn, btn_code, mods_json](bool ok, CefRefPtr<CefValue> value, const std::string& error) {
        if (!ok) {
            tool::ReplyError(safe_req.id, error.empty() ? "点击定位失败" : error);
            return;
        }
        const std::string text = value != nullptr ? value->GetString().ToString() : "";
        CefRefPtr<CefDictionaryValue> pos = ParseJsonObject(text);
        const int x = pos != nullptr ? IntOf(pos, "x") : 0;
        const int y = pos != nullptr ? IntOf(pos, "y") : 0;
        const int vw = pos != nullptr ? IntOf(pos, "vw") : 0;
        const int vh = pos != nullptr ? IntOf(pos, "vh") : 0;
        if (safe_req.window != nullptr) safe_req.window->ShowAgentCursor(x, y, vw, vh, true);

        const bool use_native = (safe_req.window != nullptr && safe_req.window->IsVisible() && browser != nullptr);
        if (use_native) {
            input::SendNativeClick(browser, x, y, cef_btn, double_click ? 2 : 1, cef_mods);
            CefRefPtr<CefDictionaryValue> out = MakeDict();
            out->SetString("message", "已点击");
            BrowserTab* cur_tab = (safe_req.window != nullptr && safe_req.window->Tabs() != nullptr) ? safe_req.window->Tabs()->Find(browser) : nullptr;
            ToolRequest next_req = safe_req;
            next_req.tab = cur_tab;
            if (cur_tab != nullptr && cur_tab->cdp != nullptr) next_req.cdp = cur_tab->cdp;
            tool::FinishAction(next_req, out, true);
            return;
        }

        // 隐藏窗口兜底：仅走 JS PointerEvent + MouseEvent + el.click()，不与原生点击双发
        std::string tail =
            "var r=el.getBoundingClientRect();"
            "var cx=Math.round(r.left+r.width/2),cy=Math.round(r.top+r.height/2);"
            "var m=(" + mods_json + ").map(function(s){return String(s).toLowerCase();});"
            "var opts={bubbles:true,cancelable:true,composed:true,view:window,clientX:cx,clientY:cy,button:" + std::to_string(btn_code) + ",detail:" + (double_click ? "2" : "1") + ","
            "ctrlKey:m.indexOf('ctrl')>=0||m.indexOf('control')>=0,"
            "altKey:m.indexOf('alt')>=0,shiftKey:m.indexOf('shift')>=0,"
            "metaKey:m.indexOf('meta')>=0||m.indexOf('command')>=0||m.indexOf('cmd')>=0};"
            "if(el.focus){try{el.focus();}catch(e){}}"
            "if(typeof PointerEvent==='function'){el.dispatchEvent(new PointerEvent('pointerdown',opts));}"
            "el.dispatchEvent(new MouseEvent('mousedown',opts));"
            "if(typeof PointerEvent==='function'){el.dispatchEvent(new PointerEvent('pointerup',opts));}"
            "el.dispatchEvent(new MouseEvent('mouseup',opts));"
            "if(typeof el.click==='function'&&" + std::to_string(btn_code) + "===0&&!m.length){el.click();}"
            "else{el.dispatchEvent(new MouseEvent('click',opts));}";
        if (double_click) tail += "el.dispatchEvent(new MouseEvent('dblclick',opts));";
        tail += "return '已点击';";

        safe_req.cdp->Evaluate(js::Element(ref, tail), false, [safe_req, browser](bool ok2, CefRefPtr<CefValue>, const std::string& error2) {
            if (!ok2) {
                tool::ReplyError(safe_req.id, error2.empty() ? "点击失败" : error2);
                return;
            }
            CefRefPtr<CefDictionaryValue> out = MakeDict();
            out->SetString("message", "已点击");
            BrowserTab* cur_tab = (safe_req.window != nullptr && safe_req.window->Tabs() != nullptr) ? safe_req.window->Tabs()->Find(browser) : nullptr;
            ToolRequest next_req = safe_req;
            next_req.tab = cur_tab;
            if (cur_tab != nullptr && cur_tab->cdp != nullptr) next_req.cdp = cur_tab->cdp;
            tool::FinishAction(next_req, out, true);
        });
    });
}
