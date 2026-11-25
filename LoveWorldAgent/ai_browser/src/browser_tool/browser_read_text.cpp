// browser_read_text 实现
#include "browser_read_text.h" // 自身声明
#include "browser_tool/tool_helpers.h" // EvalReply
#include "utils/js_builder.h" // js::kReadText
void BrowserReadTextTool::Run(const ToolRequest& req) { tool::EvalReply(req, js::kReadText, "", false, "读取页面文本失败"); } // 注入读文本脚本，长文本自动分片
