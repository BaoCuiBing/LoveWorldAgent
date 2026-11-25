// browser_snapshot 实现：脚本内部等页面就绪，长文本经分片回传
#include "browser_snapshot.h" // 自身声明
#include "browser_tool/tool_helpers.h" // EvalReply
#include "utils/js_builder.h" // js::kSnapshot
void BrowserSnapshotTool::Run(const ToolRequest& req) { tool::EvalReply(req, js::kSnapshot, "", true, "页面快照失败"); } // 注入快照脚本，awaitPromise 等待构建完成
