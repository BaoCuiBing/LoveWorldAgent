// browser_* 工具的公共动作：统一回包、参数校验、状态字典、注入 JS 并回结果
#pragma once // 防止头文件重复包含
#include <string> // 标准字符串
#include "browser_tool/browser_tool.h" // ToolRequest
#include "include/cef_values.h" // CEF 值类型
struct BrowserTab; // 前置声明
namespace tool {
void Reply(int id, CefRefPtr<CefDictionaryValue> data); // 回包：data 为本帧独占的字典，禁止同一字典用于多帧（CefWriteJSON 需要节点独占访问）
void ReplyOk(int id); // 回空数据成功包
void ReplyText(int id, const std::string& text); // 回长文本（自动分片）
void ReplyError(int id, const std::string& error); // 回错误包
bool NeedRef(const ToolRequest& req, std::string& ref); // 校验 ref 参数：缺失即回错误并返回 false
bool NeedString(const ToolRequest& req, const char* key, std::string& out); // 校验字符串参数：缺失即回错误并返回 false
CefRefPtr<CefDictionaryValue> StateOf(BrowserTab* tab); // 活动标签页状态字典
void EvalReply(const ToolRequest& req, const std::string& js, const std::string& key, bool await_promise = false, const std::string& fail_hint = ""); // 执行 JS：key 非空回 {"key": 字符串结果}；key 为空回长文本（自动分片）
void EvalReplyCursor(const ToolRequest& req, const std::string& js, const std::string& key, bool click, const std::string& fail_hint = ""); // 执行 JS 并解析 {message,x,y,vw,vh}：回包同时驱动模拟鼠标（click 播放涟漪）
void FinishAction(const ToolRequest& req, CefRefPtr<CefDictionaryValue> payload, bool maybe_navigates); // 若 args.includeSnapshot==true 则稳定等待并附带最新快照，否则直接回包
}
