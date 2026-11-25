// browser_hover 实现：可见窗口走 SendNativeMove，隐藏窗口走 PointerEvent+MouseEvent，同时显示模拟鼠标
#include "browser_hover.h" // 自身声明
#include "browser_tool/input_helper.h" // SendNativeMove
#include "browser_tool/tool_helpers.h" // NeedRef/Reply/ReplyError
#include "server/cdp_client.h" // Evaluate
#include "uis/app_window.h" // IsVisible/ShowAgentCursor
#include "uis/tab_manager.h" // BrowserTab
#include "utils/js_builder.h" // js::Element
#include "utils/json_util.h" // ParseJsonObject/MakeDict
void BrowserHoverTool::Run(const ToolRequest& req) {
    std::string ref;
    if (!tool::NeedRef(req, ref)) return;
    const std::string locate_js = js::Element(ref,
        "el.scrollIntoView({block:'center',inline:'center'});"
        "var r=el.getBoundingClientRect();"
        "return JSON.stringify({x:Math.round(r.left+r.width/2),y:Math.round(r.top+r.height/2),vw:innerWidth,vh:innerHeight});");
    CefRefPtr<CefBrowser> browser = req.tab != nullptr ? req.tab->browser : nullptr;
    ToolRequest safe_req = req;
    safe_req.tab = nullptr;
    safe_req.cdp->Evaluate(locate_js, false, [safe_req, ref, browser](bool ok, CefRefPtr<CefValue> value, const std::string& error) {
        if (!ok) {
            tool::ReplyError(safe_req.id, error.empty() ? "悬停失败" : error);
            return;
        }
        const std::string text = value != nullptr ? value->GetString().ToString() : "";
        CefRefPtr<CefDictionaryValue> pos = ParseJsonObject(text);
        const int x = pos != nullptr ? IntOf(pos, "x") : 0;
        const int y = pos != nullptr ? IntOf(pos, "y") : 0;
        const int vw = pos != nullptr ? IntOf(pos, "vw") : 0;
        const int vh = pos != nullptr ? IntOf(pos, "vh") : 0;
        if (safe_req.window != nullptr) safe_req.window->ShowAgentCursor(x, y, vw, vh, false);

        if (safe_req.window != nullptr && safe_req.window->IsVisible() && browser != nullptr) {
            input::SendNativeMove(browser, x, y, 0);
            CefRefPtr<CefDictionaryValue> out = MakeDict();
            out->SetString("message", "已悬停");
            tool::Reply(safe_req.id, out);
            return;
        }

        const std::string hover_js = js::Element(ref,
            "var r=el.getBoundingClientRect();"
            "var cx=Math.round(r.left+r.width/2),cy=Math.round(r.top+r.height/2);"
            "var o={bubbles:true,cancelable:true,composed:true,view:window,clientX:cx,clientY:cy};"
            "if(typeof PointerEvent==='function'){el.dispatchEvent(new PointerEvent('pointerover',o));el.dispatchEvent(new PointerEvent('pointermove',o));}"
            "el.dispatchEvent(new MouseEvent('mouseover',o));"
            "el.dispatchEvent(new MouseEvent('mousemove',o));"
            "el.dispatchEvent(new MouseEvent('mouseenter',o));"
            "return '已悬停';");
        safe_req.cdp->Evaluate(hover_js, false, [safe_req](bool ok2, CefRefPtr<CefValue>, const std::string& error2) {
            if (!ok2) {
                tool::ReplyError(safe_req.id, error2.empty() ? "悬停失败" : error2);
                return;
            }
            CefRefPtr<CefDictionaryValue> out = MakeDict();
            out->SetString("message", "已悬停");
            tool::Reply(safe_req.id, out);
        });
    });
}
