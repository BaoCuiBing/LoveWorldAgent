// browser_type：输入文本（必填 ref/text，可选 slowly/submit）
#pragma once // 防止头文件重复包含
#include "browser_tool/browser_tool.h" // 工具接口
class BrowserTypeTool : public IBrowserTool { // 输入工具
public:
    const char* name() const override { return "browser_type"; } // 指令名
    void Run(const ToolRequest& req) override; // 执行输入
};
