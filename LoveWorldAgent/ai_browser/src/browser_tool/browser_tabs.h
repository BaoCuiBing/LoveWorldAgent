// browser_tabs：标签页管理（必填 action=list/new/close/select，可选 index/url）
#pragma once // 防止头文件重复包含
#include "browser_tool/browser_tool.h" // 工具接口
class BrowserTabsTool : public IBrowserTool { // 标签管理工具
public:
    const char* name() const override { return "browser_tabs"; } // 指令名
    void Run(const ToolRequest& req) override; // 执行 list/new/close/select
};
