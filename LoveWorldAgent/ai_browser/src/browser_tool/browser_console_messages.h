// browser_console_messages：读取控制台日志（可选 level/onlyErrors）
#pragma once // 防止头文件重复包含
#include "browser_tool/browser_tool.h" // 工具接口
class BrowserConsoleMessagesTool : public IBrowserTool { // 控制台消息工具
public:
    const char* name() const override { return "browser_console_messages"; } // 指令名
    void Run(const ToolRequest& req) override; // 读取消息
};
