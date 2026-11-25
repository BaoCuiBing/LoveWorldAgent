// browser_scroll：滚动（必填 direction，可选 amount/ref）
#pragma once // 防止头文件重复包含
#include "browser_tool/browser_tool.h" // 工具接口
class BrowserScrollTool : public IBrowserTool { // 滚动工具（页面或元素内部）
public:
    const char* name() const override { return "browser_scroll"; } // 指令名
    void Run(const ToolRequest& req) override; // 执行滚动
};
