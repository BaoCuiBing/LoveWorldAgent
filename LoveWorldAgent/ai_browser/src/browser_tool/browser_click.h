// browser_click：点击元素（必填 ref，可选 doubleClick/button/modifiers）
#pragma once // 防止头文件重复包含
#include "browser_tool/browser_tool.h" // 工具接口
class BrowserClickTool : public IBrowserTool { // 点击工具（DOM 事件派发）
public:
    const char* name() const override { return "browser_click"; } // 指令名
    void Run(const ToolRequest& req) override; // 执行点击
};
