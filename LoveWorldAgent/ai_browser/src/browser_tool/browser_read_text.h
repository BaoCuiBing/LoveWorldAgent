// browser_read_text：读取页面可见文本（辅助工具，无参数）
#pragma once // 防止头文件重复包含
#include "browser_tool/browser_tool.h" // 工具接口
class BrowserReadTextTool : public IBrowserTool { // 读文本工具
public:
    const char* name() const override { return "browser_read_text"; } // 指令名
    void Run(const ToolRequest& req) override; // 执行读取
};
