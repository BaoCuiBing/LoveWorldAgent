// browser_evaluate 实现：函数体包成立即执行函数，支持双层 ref 解析，结果按 JSON 文本回传（长则分片）
#include "browser_evaluate.h" // 自身声明
#include "server/cdp_client.h" // Evaluate
#include "browser_tool/tool_helpers.h" // NeedString/ReplyError/ReplyText
#include "utils/js_builder.h" // js::SafeRef/ResolveRefExpr
#include "utils/json_util.h" // WriteJson
void BrowserEvaluateTool::Run(const ToolRequest& req) {
    std::string body;
    if (!tool::NeedString(req, "function", body)) return;
    std::string ref;
    ArgGetString(req.args, "ref", ref);
    std::string script;
    if (!js::SafeRef(ref).empty()) {
        script = "(function(el){" + body + "})(" + js::ResolveRefExpr(ref) + ")";
    } else {
        script = "(function(){" + body + "})()";
    }
    const int id = req.id;
    req.cdp->Evaluate(script, true, [id](bool ok, CefRefPtr<CefValue> value, const std::string& error) {
        if (!ok) { tool::ReplyError(id, error); return; }
        tool::ReplyText(id, value != nullptr ? WriteJson(value) : "null");
    });
}
