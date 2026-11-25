// browser_get_attribute 实现：读取元素指定属性
#include "browser_get_attribute.h" // 自身声明
#include "server/cdp_client.h" // Evaluate
#include "browser_tool/tool_helpers.h" // NeedRef/NeedString
#include "utils/js_builder.h" // js::Literal/Element
#include "utils/json_util.h" // MakeDict
void BrowserGetAttributeTool::Run(const ToolRequest& req) { std::string ref; if (!tool::NeedRef(req, ref)) return; std::string attribute; if (!tool::NeedString(req, "attribute", attribute)) return; const std::string script = js::Element(ref, "var v=el.getAttribute(" + js::Literal(attribute) + ");" "return v==null?'':String(v);"); const int id = req.id; req.cdp->Evaluate(script, false, [id, attribute](bool ok, CefRefPtr<CefValue> value, const std::string& error) { if (!ok) { tool::ReplyError(id, error); return; } CefRefPtr<CefDictionaryValue> data = MakeDict(); data->SetString("attribute", attribute); data->SetString("value", value != nullptr ? value->GetString().ToString() : ""); tool::Reply(id, data); }); } // 读属性并回 {attribute,value}，属性缺失回空串
