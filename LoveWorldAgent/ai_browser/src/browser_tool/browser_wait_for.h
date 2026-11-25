// browser_wait_for：等待条件（time / text / textGone 三者择一）
#pragma once // 防止头文件重复包含
#include "browser_tool/browser_tool.h" // 工具接口
class BrowserWaitForTool : public IBrowserTool { // 等待工具（时间/文本出现/文本消失）
public:
    const char* name() const override { return "browser_wait_for"; } // 指令名
    void Run(const ToolRequest& req) override; // 执行等待
};
