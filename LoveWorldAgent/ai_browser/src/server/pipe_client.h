// 命名管道客户端：主程序 ↔ sidecar 的 JSON 行协议 v2（req / res / ev 三类帧）。
// 入站：{"type":"req","id":N,"cmd":"browser_click","args":{...}}；兼容旧帧 navigate / setRect / show（视为 id=0 的一次性指令）
// 出站：{"type":"res","id":N,"ok":true,"data":{...}} / {"type":"res","id":N,"ok":false,"error":"..."} / {"type":"ev","event":"title","data":{...}}
// 长文本（截图 base64、页面快照）按 {"chunks":K,"chunk":i,"data":"片段"} 分片，接收端按 id 顺序拼接；JSON 解析/序列化统一走 json_util。
#pragma once // 防止头文件重复包含
#include <windows.h> // HANDLE/HWND
// 命名管道线程 → UI 线程的内部消息（与 Qt 重写的 sidecar 共用，原定义在 app_window.h）
#ifndef LWA_APP_COMMANDS // 防重定义
#define LWA_APP_COMMANDS // 消息宏守卫
#define WM_APP_COMMAND (WM_APP + 4) // 指令消息：lParam 携带 CmdRequest*
#define WM_APP_ENSURE_TAB (WM_APP + 5) // 补建一个空标签
#define WM_APP_DEFER (WM_APP + 6) // 在 UI 线程执行回调：lParam 携带 std::function*
#endif
#include <atomic> // 原子布尔
#include <condition_variable> // 出站唤醒
#include <memory> // unique_ptr
#include <mutex> // 互斥
#include <string> // 标准字符串
#include <thread> // 读写线程
#include <vector> // 队列容器
#include "include/cef_values.h" // CEF 值类型
struct CmdRequest { int id = 0; std::string cmd; std::string args_json; }; // 管道线程解析出的请求；CefValue 受创建线程限制，故只传 args 的 JSON 文本，由 UI 线程自己解析
class PipeClient { // 命名管道客户端
public:
    static void Start(const std::wstring& pipe_name); // 启动读写线程（幂等）
    static void Stop(); // 停止并回收线程
    static void SetTarget(HWND target); // 设定指令投递目标（面板窗口），并冲入连接前排队的请求
    static bool Alive(); // 实例是否存在且运行中
    static void Respond(int id, CefRefPtr<CefDictionaryValue> data); // 响应成功（仅 CEF UI 线程可调）
    static void RespondError(int id, const std::string& error); // 响应失败
    static void RespondText(int id, const std::string& text); // 长文本响应：超限自动分片
    static void PostEvent(const std::string& event, CefRefPtr<CefDictionaryValue> data); // 状态上报（title / address / loading / tabs / dialog / page）
private:
    explicit PipeClient(std::wstring pipe_name); // 私有构造：经 Start 创建
    ~PipeClient(); // 析构关闭句柄
    void ReadLoop(); // 读线程：连接-轮询读-按行分发
    void WriteLoop(); // 写线程：出站队列落管道
    bool OpenOnce(); // 连接一次管道（成功后复制写句柄）
    void CloseHandles(); // 关闭读写句柄
    void HandleLine(const std::string& line); // 单行帧解析：req/navigate/setRect/show → CmdRequest
    void Write(const std::string& line); // 入出站队列，由写线程落管道（避免 UI 线程被背压卡住）
    void Flush(const std::string& line); // 写线程真正写管道
    void DispatchQueued(HWND target); // 设定目标并冲入排队指令（调用方需持有 life_lock_）
    static void PostRequest(HWND target, std::unique_ptr<CmdRequest> req); // PostMessageW 转交 UI 线程，接收方释放
    static PipeClient* instance_; // 单例
    static std::mutex life_lock_; // 生命周期锁
    const std::wstring pipe_name_; // 管道名（不含 \\.\pipe\ 前缀）
    HANDLE pipe_ = INVALID_HANDLE_VALUE; // 只给读线程用
    HANDLE write_ = INVALID_HANDLE_VALUE; // 同一实例的副本句柄，只给写线程用（避免读写互锁）
    std::thread reader_; // 读线程
    std::thread writer_; // 写线程
    std::atomic<bool> running_{false}; // 运行标记
    std::mutex io_lock_; // 读写句柄/目标保护
    std::mutex write_lock_; // 写串行化
    HWND target_ = nullptr; // 指令投递目标窗口
    std::vector<std::unique_ptr<CmdRequest>> pending_; // 目标未就绪时的排队指令
    std::mutex out_lock_; // 出站队列锁
    std::condition_variable out_cv_; // 出站唤醒
    std::vector<std::string> outbox_; // 出站队列
};
