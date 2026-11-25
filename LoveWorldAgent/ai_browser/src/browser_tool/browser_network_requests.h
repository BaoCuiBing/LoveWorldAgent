// browser_network_requests：读取网络请求（可选 includeStatic/filter）
#pragma once // 防止头文件重复包含
#include "browser_tool/browser_tool.h" // 工具接口
class BrowserNetworkRequestsTool : public IBrowserTool { // 网络请求记录工具
public:
    const char* name() const override { return "browser_network_requests"; } // 指令名
    void Run(const ToolRequest& req) override; // 读取记录
};
