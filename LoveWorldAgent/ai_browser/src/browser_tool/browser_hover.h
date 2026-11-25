// browser_hover：悬停元素（必填 ref）
#pragma once // 防止头文件重复包含
#include "browser_tool/browser_tool.h" // 工具接口
class BrowserHoverTool : public IBrowserTool { // 悬停工具
public:
    const char* name() const override { return "browser_hover"; } // 指令名
    void Run(const ToolRequest& req) override; // 执行悬停
};
