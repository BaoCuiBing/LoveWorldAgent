// 工具注册表：持有全部 browser_* 工具实例，按指令名分发；新增工具只需加一对 cpp/h 并在 Build 列表里补一行
#pragma once // 防止头文件重复包含
#include <memory> // unique_ptr
#include <string> // 标准字符串
#include <vector> // 工具数组
#include "browser_tool/browser_tool.h" // 工具接口
class ToolRegistry { // 工具注册表
public:
    ToolRegistry(); // 构造时注册全部工具
    bool Dispatch(const ToolRequest& req) const; // 命中则执行并返回 true；未命中返回 false 交给调用方继续处理
    std::vector<std::string> names() const; // 全部工具名
private:
    std::vector<std::unique_ptr<IBrowserTool>> tools_; // 工具实例表
};
