// browser_snapshot：页面可访问性树与元素 ref（无参数）
#pragma once // 防止头文件重复包含
#include "browser_tool/browser_tool.h" // 工具接口
class BrowserSnapshotTool : public IBrowserTool { // 快照工具
public:
    const char* name() const override { return "browser_snapshot"; } // 指令名
    void Run(const ToolRequest& req) override; // 执行快照
};
