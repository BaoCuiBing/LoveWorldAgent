// browser_select_option：选择下拉项（必填 ref/values）
#pragma once // 防止头文件重复包含
#include "browser_tool/browser_tool.h" // 工具接口
class BrowserSelectOptionTool : public IBrowserTool { // 下拉选择工具
public:
    const char* name() const override { return "browser_select_option"; } // 指令名
    void Run(const ToolRequest& req) override; // 执行选择
};
