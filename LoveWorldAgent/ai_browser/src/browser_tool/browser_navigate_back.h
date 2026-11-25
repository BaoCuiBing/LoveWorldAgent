// browser_navigate_back：后退
#pragma once // 防止头文件重复包含
#include "browser_tool/browser_tool.h" // 工具接口
class BrowserNavigateBackTool : public IBrowserTool { // 后退工具
public:
    const char* name() const override { return "browser_navigate_back"; } // 指令名
    void Run(const ToolRequest& req) override; // 执行后退
};
