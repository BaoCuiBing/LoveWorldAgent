// browser_navigate：打开 URL（必填 url）
#pragma once // 防止头文件重复包含
#include "browser_tool/browser_tool.h" // 工具接口
class BrowserNavigateTool : public IBrowserTool { // 导航工具
public:
    const char* name() const override { return "browser_navigate"; } // 指令名
    void Run(const ToolRequest& req) override; // 执行导航
};
