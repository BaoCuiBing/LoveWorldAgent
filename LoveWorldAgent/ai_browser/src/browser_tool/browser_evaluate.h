// browser_evaluate：执行 JS（必填 function，可选 element/ref 指定元素上下文）
#pragma once // 防止头文件重复包含
#include "browser_tool/browser_tool.h" // 工具接口
class BrowserEvaluateTool : public IBrowserTool { // 执行 JS 工具
public:
    const char* name() const override { return "browser_evaluate"; } // 指令名
    void Run(const ToolRequest& req) override; // 执行 JS
};
