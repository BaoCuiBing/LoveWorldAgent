// 指令入口：把管道下发的 req 指令分流——window_* / state 就地执行，browser_* 交给工具注册表。只在 CEF UI 线程调用（由 AppWindow 的 WM_APP_COMMAND 转交）。
#pragma once // 防止头文件重复包含
#include <cstdint> // uint64_t
#include <memory> // shared_ptr
#include <string> // 标准字符串
#include <vector> // 排队队列
#include "browser_tool/tool_registry.h" // 工具注册表
#include "include/cef_values.h" // CEF 值类型
class AppWindow; // 前置声明
class AgentApi { // 指令分流器
public:
    explicit AgentApi(AppWindow* window); // 绑定窗口
    ~AgentApi(); // 析构标记失效
    void Execute(int id, const std::string& cmd, const std::string& args_json); // id==0 表示旧帧式一次性指令，只执行不回响应；args_json 在本（UI）线程解析
    void OnTabReady(); // 首个标签浏览器就绪后冲入初始化期间排队的指令
    bool awaiting_user() const { return awaiting_user_; } // 交还用户操作标记：置位后由人工操作，下一条指令即视为收回
    const ToolRegistry& registry() const { return registry_; } // 工具注册表只读访问
private:
    struct PendingCmd { int id = 0; std::string cmd; std::string args_json; };
    bool RunWindowCommand(int id, const std::string& cmd, CefRefPtr<CefDictionaryValue> args); // 窗口级指令；返回 false 表示不是窗口指令，继续走工具分发
    AppWindow* window_ = nullptr; // 宿主窗口
    ToolRegistry registry_; // browser_* 工具注册表
    bool awaiting_user_ = false; // 是否等待用户交互
    bool tab_ready_ = false; // 首个标签是否已就绪
    bool wait_timer_armed_ = false; // 是否已挂载初始化超时看门狗
    uint64_t pending_gen_ = 0; // 排队批次代次（区分新建标签超时与历史批次）
    std::vector<PendingCmd> pending_; // 启动竞态期间排队的标签指令
    std::shared_ptr<bool> alive_ = std::make_shared<bool>(true); // 生命周期守卫
};
