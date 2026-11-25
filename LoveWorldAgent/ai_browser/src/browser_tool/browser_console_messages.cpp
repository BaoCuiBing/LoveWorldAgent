// browser_console_messages 实现：读 CdpClient 的控制台环形缓冲（由 AppHandler::OnConsoleMessage 灌入）
#include "browser_console_messages.h" // 自身声明
#include "server/cdp_client.h" // console 缓冲
#include "browser_tool/tool_helpers.h" // ReplyText
#include "utils/json_util.h" // 参数读取/序列化
void BrowserConsoleMessagesTool::Run(const ToolRequest& req) { std::string level; ArgGetString(req.args, "level", level); bool only_errors = false; ArgGetBool(req.args, "onlyErrors", only_errors); if (only_errors) level = "error"; CefRefPtr<CefListValue> list = MakeList(); for (const ConsoleEntry& entry : req.cdp->console()) { if (!level.empty() && entry.level != level) continue; CefRefPtr<CefDictionaryValue> item = MakeDict(); item->SetString("level", entry.level); item->SetString("text", entry.text); item->SetString("source", entry.source); item->SetInt("line", entry.line); ListAddDict(list, item); } tool::ReplyText(req.id, WriteJsonList(list)); } // 按 level 过滤（onlyErrors 等价 level=error），回 JSON 列表文本
