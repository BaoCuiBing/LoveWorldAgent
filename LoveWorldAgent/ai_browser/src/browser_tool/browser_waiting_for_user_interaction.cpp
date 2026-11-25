// browser_waiting_for_user_interaction 实现：唤起窗口并上报 awaitUser 事件，等智能体后续指令收回控制权
#include "browser_waiting_for_user_interaction.h" // 自身声明
#include "browser_tool/tool_helpers.h" // StateOf/Reply
#include "uis/app_window.h" // ShowSelf/FocusSelf
#include "server/pipe_client.h" // PostEvent
#include "utils/json_util.h" // 参数读取
void BrowserWaitingForUserTool::Run(const ToolRequest& req) { std::string reason; ArgGetString(req.args, "reason", reason); if (req.window != nullptr) { req.window->ShowSelf(true); req.window->FocusSelf(); } CefRefPtr<CefDictionaryValue> event = tool::StateOf(req.tab); event->SetString("reason", reason); event->SetBool("awaitingUser", true); PipeClient::PostEvent("awaitUser", event); CefRefPtr<CefDictionaryValue> data = tool::StateOf(req.tab); data->SetString("reason", reason); data->SetBool("awaitingUser", true); tool::Reply(req.id, data); } // 事件帧与响应帧必须各用一份字典：CefWriteJSON 要求节点独占访问，同一字典连发两帧会让第二帧变空串
