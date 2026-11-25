// browser_press_key：按键（必填 key，如 Enter/Escape/ArrowDown/单字符）
#pragma once // 防止头文件重复包含
#include "browser_tool/browser_tool.h" // 工具接口
class BrowserPressKeyTool : public IBrowserTool { // 按键工具
public:
    const char* name() const override { return "browser_press_key"; } // 指令名
    void Run(const ToolRequest& req) override; // 执行按键
};
