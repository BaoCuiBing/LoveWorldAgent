// browser_get_attribute：读取元素属性（必填 ref/attribute）
#pragma once // 防止头文件重复包含
#include "browser_tool/browser_tool.h" // 工具接口
class BrowserGetAttributeTool : public IBrowserTool { // 读属性工具
public:
    const char* name() const override { return "browser_get_attribute"; } // 指令名
    void Run(const ToolRequest& req) override; // 执行读取
};
