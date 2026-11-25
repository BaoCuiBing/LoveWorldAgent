// browser_handle_dialog：处理弹窗（可选 accept/promptText，默认 accept=true）
#pragma once // 防止头文件重复包含
#include "browser_tool/browser_tool.h" // 工具接口
class BrowserHandleDialogTool : public IBrowserTool { // 弹窗应答工具
public:
    const char* name() const override { return "browser_handle_dialog"; } // 指令名
    void Run(const ToolRequest& req) override; // 执行应答
};
