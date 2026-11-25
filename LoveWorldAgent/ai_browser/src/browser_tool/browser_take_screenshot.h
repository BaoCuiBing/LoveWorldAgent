// browser_take_screenshot：截图（可选 filename/type/fullPage/element/ref，ref 时仅截该元素）
#pragma once // 防止头文件重复包含
#include "browser_tool/browser_tool.h" // 工具接口
class BrowserTakeScreenshotTool : public IBrowserTool { // 截图工具（页面或元素）
public:
    const char* name() const override { return "browser_take_screenshot"; } // 指令名
    void Run(const ToolRequest& req) override; // 执行截图
};
