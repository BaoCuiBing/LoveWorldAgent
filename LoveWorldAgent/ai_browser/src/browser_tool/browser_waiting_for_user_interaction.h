// browser_waiting_for_user_interaction：交还用户操作（必填 reason，可选 viewId）
#pragma once // 防止头文件重复包含
#include "browser_tool/browser_tool.h" // 工具接口
class BrowserWaitingForUserTool : public IBrowserTool { // 交还用户控制权工具
public:
    const char* name() const override { return "browser_waiting_for_user_interaction"; } // 指令名
    void Run(const ToolRequest& req) override; // 唤起窗口并上报 awaitUser
};
