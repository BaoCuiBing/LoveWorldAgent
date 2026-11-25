// browser_type 实现：React/Vue 原生属性描述符 setter + InputEvent/change（contenteditable 走 execCommand('insertText')），submit 支持原生 Enter 键，经 FinishAction 支持 includeSnapshot
#include "browser_type.h" // 自身声明
#include "browser_tool/input_helper.h" // SendNativeKey
#include "browser_tool/tool_helpers.h" // NeedRef/ReplyError/FinishAction
#include "server/cdp_client.h" // Evaluate
#include "uis/app_window.h" // IsVisible/ShowAgentCursor
#include "uis/tab_manager.h" // BrowserTab
#include "utils/js_builder.h" // js::Literal/Element/QueryKey
#include "utils/json_util.h" // ArgGetBool/ParseJsonObject/MakeDict
void BrowserTypeTool::Run(const ToolRequest& req) {
    std::string ref;
    if (!tool::NeedRef(req, ref)) return;
    std::string text;
    if (!ArgGetString(req.args, "text", text)) { tool::ReplyError(req.id, "缺少参数 text"); return; }
    bool slowly = false;
    bool submit = false;
    ArgGetBool(req.args, "slowly", slowly);
    ArgGetBool(req.args, "submit", submit);

    const bool use_native_enter = submit && (req.window != nullptr && req.window->IsVisible() && req.tab != nullptr && req.tab->browser != nullptr);

    std::string tail =
        "el.scrollIntoView({block:'center'});"
        "if(el.focus){try{el.focus();}catch(e){}}"
        "var text=" + js::Literal(text) + ";"
        "function setNativeValue(target,next){"
        "if(target.tagName==='TEXTAREA'){"
        "var desc=Object.getOwnPropertyDescriptor(HTMLTextAreaElement.prototype,'value');"
        "if(desc&&desc.set){desc.set.call(target,next);return;}"
        "}else if(target.tagName==='INPUT'){"
        "var desc=Object.getOwnPropertyDescriptor(HTMLInputElement.prototype,'value');"
        "if(desc&&desc.set){desc.set.call(target,next);return;}"
        "}"
        "target.value=next;"
        "}"
        "function fireInput(chunk){"
        "try{el.dispatchEvent(new InputEvent('input',{bubbles:true,cancelable:true,inputType:'insertText',data:chunk}));}"
        "catch(e){el.dispatchEvent(new Event('input',{bubbles:true}));}"
        "}"
        "function put(chunk){"
        "if(el.isContentEditable){"
        "var ok=false;try{ok=document.execCommand('insertText',false,chunk);}catch(e){ok=false;}"
        "if(!ok){el.textContent=(el.textContent||'')+chunk;fireInput(chunk);}"
        "}else if(el.tagName==='INPUT'||el.tagName==='TEXTAREA'){"
        "setNativeValue(el,(el.value||'')+chunk);fireInput(chunk);"
        "}else{"
        "el.textContent=(el.textContent||'')+chunk;fireInput(chunk);"
        "}"
        "}";

    if (slowly) {
        tail +=
            "for(var i=0;i<text.length;i++){"
            "var ch=text[i];"
            "el.dispatchEvent(new KeyboardEvent('keydown',{key:ch,bubbles:true,cancelable:true}));"
            "put(ch);"
            "el.dispatchEvent(new KeyboardEvent('keyup',{key:ch,bubbles:true,cancelable:true}));"
            "}"
            "el.dispatchEvent(new Event('change',{bubbles:true}));";
    } else {
        tail +=
            "put(text);"
            "el.dispatchEvent(new Event('change',{bubbles:true}));";
    }

    if (submit && !use_native_enter) {
        tail +=
            "var eo={key:'Enter',code:'Enter',keyCode:13,which:13,bubbles:true,cancelable:true};"
            "el.dispatchEvent(new KeyboardEvent('keydown',eo));"
            "el.dispatchEvent(new KeyboardEvent('keypress',eo));"
            "el.dispatchEvent(new KeyboardEvent('keyup',eo));"
            "if(el.form&&el.form.requestSubmit){try{el.form.requestSubmit();}catch(e){}}";
    }

    tail +=
        "var r=el.getBoundingClientRect();"
        "return JSON.stringify({value:String(el.value!==undefined?el.value:(el.textContent||'')),x:Math.round(r.left+r.width/2),y:Math.round(r.top+r.height/2),vw:innerWidth,vh:innerHeight});";

    CefRefPtr<CefBrowser> browser = req.tab != nullptr ? req.tab->browser : nullptr;
    ToolRequest safe_req = req;
    safe_req.tab = nullptr;
    safe_req.cdp->Evaluate(js::Element(ref, tail), false, [safe_req, submit, use_native_enter, browser](bool ok, CefRefPtr<CefValue> value, const std::string& error) {
        if (!ok) {
            tool::ReplyError(safe_req.id, error.empty() ? "输入失败" : error);
            return;
        }
        const std::string raw = value != nullptr ? value->GetString().ToString() : "";
        CefRefPtr<CefDictionaryValue> parsed = ParseJsonObject(raw);
        const std::string val = parsed != nullptr ? StrOf(parsed, "value") : raw;
        if (parsed != nullptr && safe_req.window != nullptr) {
            safe_req.window->ShowAgentCursor(IntOf(parsed, "x"), IntOf(parsed, "y"), IntOf(parsed, "vw"), IntOf(parsed, "vh"), false);
        }
        if (use_native_enter) {
            js::KeyInfo enter;
            if (js::QueryKey("Enter", enter)) {
                input::SendNativeKey(browser, enter, 0, true);
            }
        }
        CefRefPtr<CefDictionaryValue> out = MakeDict();
        out->SetString("value", val);
        BrowserTab* cur_tab = (safe_req.window != nullptr && safe_req.window->Tabs() != nullptr && browser != nullptr) ? safe_req.window->Tabs()->Find(browser) : nullptr;
        ToolRequest next_req = safe_req;
        next_req.tab = cur_tab;
        if (cur_tab != nullptr && cur_tab->cdp != nullptr) next_req.cdp = cur_tab->cdp;
        tool::FinishAction(next_req, out, submit);
    });
}
