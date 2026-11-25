// tool_helpers 实现：统一回包、参数校验、状态字典、JS 注入回包与 FinishAction（includeSnapshot）
#include "tool_helpers.h" // 自身声明
#include "browser_tool/wait_helper.h" // WaitForHelper
#include "server/app_handler.h" // dialog_pending
#include "server/cdp_client.h" // Evaluate
#include "server/pipe_client.h" // 管道响应
#include "uis/app_window.h" // ShowAgentCursor
#include "uis/tab_manager.h" // BrowserTab
#include "utils/js_builder.h" // js::kSnapshot
#include "utils/json_util.h" // JSON 工具
#include "utils/logging.h" // 文件日志
namespace tool {
void Reply(int id, CefRefPtr<CefDictionaryValue> data) { PipeClient::Respond(id, data); } // 回包
void ReplyOk(int id) { PipeClient::Respond(id, nullptr); } // 回空数据成功包
void ReplyText(int id, const std::string& text) { PipeClient::RespondText(id, text); } // 回长文本
void ReplyError(int id, const std::string& error) { LOG_WARN("tool", QStringLiteral("指令 %1 失败：%2").arg(id).arg(QString::fromStdString(error))); PipeClient::RespondError(id, error); } // 回错误包并记录失败原因
bool NeedRef(const ToolRequest& req, std::string& ref) { ref.clear(); ArgGetString(req.args, "ref", ref); if (ref.empty()) { ReplyError(req.id, "缺少参数 ref"); return false; } return true; } // ref 缺失即回错误
bool NeedString(const ToolRequest& req, const char* key, std::string& out) { if (!ArgGetString(req.args, key, out) || out.empty()) { ReplyError(req.id, std::string("缺少参数 ") + key); return false; } return true; } // 字符串参数缺失即回错误
CefRefPtr<CefDictionaryValue> StateOf(BrowserTab* tab) { CefRefPtr<CefDictionaryValue> data = MakeDict(); if (tab == nullptr) { data->SetBool("ready", false); return data; } data->SetBool("ready", tab->browser != nullptr); data->SetString("url", WideToUtf8(tab->url)); data->SetString("title", WideToUtf8(tab->title)); data->SetBool("loading", tab->loading); data->SetBool("canGoBack", tab->can_back); data->SetBool("canGoForward", tab->can_forward); if (tab->handler != nullptr) data->SetBool("dialogPending", tab->handler->dialog_pending()); return data; } // 状态字典：ready/url/title/loading/后退前进/弹窗待答
void EvalReply(const ToolRequest& req, const std::string& js, const std::string& key, bool await_promise, const std::string& fail_hint) { const int id = req.id; req.cdp->Evaluate(js, await_promise, [id, key, fail_hint](bool ok, CefRefPtr<CefValue> value, const std::string& error) { if (!ok) { ReplyError(id, error.empty() ? (fail_hint.empty() ? std::string("脚本执行失败") : fail_hint) : error); return; } const std::string text = value != nullptr ? value->GetString().ToString() : ""; if (key.empty()) { PipeClient::RespondText(id, text); return; } CefRefPtr<CefDictionaryValue> data = MakeDict(); data->SetString(key, text); PipeClient::Respond(id, data); }); } // 执行 JS 并按 key 决定回包形态；失败用 fail_hint 兜底
void EvalReplyCursor(const ToolRequest& req, const std::string& js, const std::string& key, bool click, const std::string& fail_hint) { const int id = req.id; AppWindow* window = req.window; req.cdp->Evaluate(js, false, [id, key, click, fail_hint, window](bool ok, CefRefPtr<CefValue> value, const std::string& error) { if (!ok) { ReplyError(id, error.empty() ? (fail_hint.empty() ? std::string("脚本执行失败") : fail_hint) : error); return; } const std::string text = value != nullptr ? value->GetString().ToString() : ""; CefRefPtr<CefDictionaryValue> parsed = ParseJsonObject(text); const std::string message = parsed != nullptr ? StrOf(parsed, "message") : text; if (parsed != nullptr && window != nullptr) window->ShowAgentCursor(IntOf(parsed, "x"), IntOf(parsed, "y"), IntOf(parsed, "vw"), IntOf(parsed, "vh"), click); if (key.empty()) { PipeClient::RespondText(id, message); return; } CefRefPtr<CefDictionaryValue> data = MakeDict(); data->SetString(key, message); PipeClient::Respond(id, data); }); } // 解析 {message,x,y,vw,vh}：先显示模拟鼠标再按 key 回包
void FinishAction(const ToolRequest& req, CefRefPtr<CefDictionaryValue> payload, bool maybe_navigates) {
    CefRefPtr<CefDictionaryValue> result = payload != nullptr ? payload : MakeDict();
    bool include_snapshot = false;
    ArgGetBool(req.args, "includeSnapshot", include_snapshot);
    if (!include_snapshot) {
        Reply(req.id, result);
        return;
    }
    CefRefPtr<AppHandler> handler = req.tab != nullptr ? req.tab->handler : nullptr;
    CefRefPtr<CdpClient> cdp = req.cdp;
    const int id = req.id;
    WaitForHelper::Settle(req, maybe_navigates, [id, result, handler, cdp](bool dialog_pending) {
        if (dialog_pending || (handler != nullptr && handler->dialog_pending())) {
            result->SetBool("dialogPending", true);
            if (handler != nullptr) {
                CefRefPtr<CefDictionaryValue> dlg = MakeDict();
                dlg->SetString("type", handler->dialog_type());
                dlg->SetString("message", handler->dialog_message());
                result->SetDictionary("dialog", dlg);
            }
            Reply(id, result);
            return;
        }
        if (cdp == nullptr) {
            Reply(id, result);
            return;
        }
        cdp->Evaluate(js::kSnapshot, true, [id, result](bool ok, CefRefPtr<CefValue> value, const std::string&) {
            if (ok && value != nullptr && value->GetType() == VTYPE_STRING) {
                result->SetString("snapshot", value->GetString().ToString());
            }
            Reply(id, result);
        });
    });
}
}
