// browser_handle_dialog 实现：应答被 AppHandler 接管的 alert/confirm/prompt
#include "browser_handle_dialog.h" // 自身声明
#include "server/app_handler.h" // dialog_pending/HandleDialog
#include "browser_tool/tool_helpers.h" // ReplyError/Reply
#include "uis/tab_manager.h" // BrowserTab
#include "utils/json_util.h" // 参数读取/MakeDict
void BrowserHandleDialogTool::Run(const ToolRequest& req) { if (req.tab == nullptr || req.tab->handler == nullptr) { tool::ReplyError(req.id, "无可用标签页"); return; } if (!req.tab->handler->dialog_pending()) { tool::ReplyError(req.id, "当前没有待处理弹窗"); return; } bool accept = true; ArgGetBool(req.args, "accept", accept); std::string prompt_text; ArgGetString(req.args, "promptText", prompt_text); req.tab->handler->HandleDialog(accept, prompt_text); CefRefPtr<CefDictionaryValue> data = MakeDict(); data->SetBool("accepted", accept); tool::Reply(req.id, data); } // 无弹窗时报错；按 accept/promptText 应答并回 accepted
