// browser_select_option 实现：单选走 HTMLSelectElement.prototype value setter 并同步 selectedIndex/option.selected，多选遍历 option.selected，同时派发 input + change，经 FinishAction 支持 includeSnapshot
#include "browser_select_option.h" // 自身声明
#include "browser_tool/tool_helpers.h" // NeedRef/ReplyError/FinishAction
#include "server/cdp_client.h" // Evaluate
#include "uis/app_window.h" // Tabs()
#include "uis/tab_manager.h" // BrowserTab
#include "utils/js_builder.h" // js::Element
#include "utils/json_util.h" // ArgGetList/WriteJsonList/MakeDict
void BrowserSelectOptionTool::Run(const ToolRequest& req) {
    std::string ref;
    if (!tool::NeedRef(req, ref)) return;
    CefRefPtr<CefListValue> values = ArgGetList(req.args, "values");
    if (values == nullptr) { tool::ReplyError(req.id, "缺少参数 values"); return; }
    const std::string script = js::Element(ref,
        "if(el.tagName.toLowerCase()!=='select'){throw new Error('不是下拉框元素');}"
        "var want=" + WriteJsonList(values) + ";var picked=[];var firstVal=null;var firstIdx=-1;"
        "for(var i=0;i<el.options.length;i++){var o=el.options[i];var hit=false;"
        "for(var j=0;j<want.length;j++){var w=String(want[j]);"
        "if(o.value===w||o.text===w||((o.textContent||'').trim()===w)){hit=true;break;}}"
        "if(hit){if(firstIdx<0){firstIdx=i;firstVal=o.value;}picked.push(o.text);if(!el.multiple){break;}}"
        "if(el.multiple){o.selected=hit;}}"
        "if(!picked.length){throw new Error('未匹配到选项');}"
        "if(!el.multiple){"
        "var desc=Object.getOwnPropertyDescriptor(HTMLSelectElement.prototype,'value');"
        "if(desc&&desc.set){desc.set.call(el,firstVal);}else{el.value=firstVal;}"
        "if(firstIdx>=0){el.options[firstIdx].selected=true;el.selectedIndex=firstIdx;}"
        "}"
        "el.dispatchEvent(new Event('input',{bubbles:true}));"
        "el.dispatchEvent(new Event('change',{bubbles:true}));"
        "return '已选择: '+picked.join(', ');");
    CefRefPtr<CefBrowser> browser = req.tab != nullptr ? req.tab->browser : nullptr;
    ToolRequest safe_req = req;
    safe_req.tab = nullptr;
    safe_req.cdp->Evaluate(script, false, [safe_req, browser](bool ok, CefRefPtr<CefValue> value, const std::string& error) {
        if (!ok) {
            tool::ReplyError(safe_req.id, error.empty() ? "选择下拉项失败" : error);
            return;
        }
        CefRefPtr<CefDictionaryValue> out = MakeDict();
        out->SetString("message", value != nullptr ? value->GetString().ToString() : "已选择");
        BrowserTab* cur_tab = (safe_req.window != nullptr && safe_req.window->Tabs() != nullptr && browser != nullptr) ? safe_req.window->Tabs()->Find(browser) : nullptr;
        ToolRequest next_req = safe_req;
        next_req.tab = cur_tab;
        if (cur_tab != nullptr && cur_tab->cdp != nullptr) next_req.cdp = cur_tab->cdp;
        tool::FinishAction(next_req, out, true);
    });
}
