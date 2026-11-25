// AgentApi 实现：窗口级指令就地执行，browser_* 指令查注册表分发（支持启动期标签未就绪排队）
#include "server/agent_api.h" // 自身声明
#include <cstdio> // sscanf
#include <QTimer> // 超时看门狗
#include "include/wrapper/cef_helpers.h" // CEF_REQUIRE_UI_THREAD
#include "uis/app_window.h" // AppWindow 接口
#include "uis/tab_manager.h" // TabManager
#include "browser_tool/tool_helpers.h" // tool::Reply/ReplyError/StateOf
#include "server/cdp_client.h" // CdpClient
#include "server/pipe_client.h" // 管道响应
#include "utils/json_util.h" // ParseJsonObject
AgentApi::AgentApi(AppWindow* window) : window_(window) {} // 绑定宿主窗口
AgentApi::~AgentApi() { *alive_ = false; } // 置位析构守卫
void AgentApi::OnTabReady() {
    CEF_REQUIRE_UI_THREAD();
    tab_ready_ = true;
    wait_timer_armed_ = false;
    ++pending_gen_;
    if (pending_.empty()) return;
    std::vector<PendingCmd> queued;
    queued.swap(pending_);
    for (const PendingCmd& item : queued) {
        Execute(item.id, item.cmd, item.args_json);
    }
}
bool AgentApi::RunWindowCommand(int id, const std::string& cmd, CefRefPtr<CefDictionaryValue> args) { if (cmd == "window_show") { bool value = true; ArgGetBool(args, "value", value); window_->ShowSelf(value); tool::Reply(id, nullptr); return true; } if (cmd == "window_focus") { window_->FocusSelf(); tool::Reply(id, nullptr); return true; } if (cmd == "window_close") { window_->CloseSelf(); tool::Reply(id, nullptr); return true; } if (cmd == "window_quit") { window_->QuitSelf(); tool::Reply(id, nullptr); return true; } if (cmd == "window_rect") { std::string rect; int x = 0, y = 0, w = 0, h = 0; if (ArgGetString(args, "rect", rect) && std::sscanf(rect.c_str(), "%d,%d,%d,%d", &x, &y, &w, &h) == 4) { RECT payload = {static_cast<LONG>(x), static_cast<LONG>(y), static_cast<LONG>(x + w), static_cast<LONG>(y + h)}; window_->SetRect(payload); } tool::Reply(id, nullptr); return true; } if (cmd != "state") return false; TabManager* tabs = window_->Tabs(); CefRefPtr<CefDictionaryValue> data = tool::StateOf(tabs != nullptr ? tabs->Active() : nullptr); data->SetInt("tabs", tabs != nullptr ? tabs->count() : 0); data->SetInt("active", tabs != nullptr ? tabs->active() : -1); data->SetBool("visible", window_->IsVisible()); tool::Reply(id, data); return true; } // 窗口级指令分流；state 汇总活动标签与窗口可见性
void AgentApi::Execute(int id, const std::string& cmd, const std::string& args_json) {
    CEF_REQUIRE_UI_THREAD();
    CefRefPtr<CefDictionaryValue> args = ParseJsonObject(args_json);
    awaiting_user_ = false;
    if (RunWindowCommand(id, cmd, args)) return;
    TabManager* tabs = window_->Tabs();
    BrowserTab* tab = tabs != nullptr ? tabs->Active() : nullptr;
    const bool tab_creating = (tab != nullptr && tab->browser == nullptr && tab->handler != nullptr);
    if (!tab_ready_ || tab_creating) {
        if (pending_.size() < 64) {
            pending_.push_back({id, cmd, args_json});
            if (!wait_timer_armed_) {
                wait_timer_armed_ = true;
                const uint64_t gen = pending_gen_;
                std::shared_ptr<bool> alive = alive_;
                QTimer::singleShot(8000, [this, alive, gen]() {
                    if (!*alive) return;
                    if (gen != pending_gen_) return;
                    wait_timer_armed_ = false;
                    if (pending_.empty()) return;
                    std::vector<PendingCmd> expired;
                    expired.swap(pending_);
                    for (const PendingCmd& item : expired) {
                        tool::ReplyError(item.id, "浏览器初始化超时");
                    }
                });
            }
        } else {
            tool::ReplyError(id, "浏览器初始化排队已满");
        }
        return;
    }
    if (cmd.compare(0, 3, "ui_") == 0) { window_->Chrome()->RunPipeCommand(id, cmd, args); return; }
    if (tab == nullptr || tab->browser == nullptr || tab->cdp == nullptr) { tool::ReplyError(id, "无可用标签页"); return; }
    ToolRequest req;
    req.id = id;
    req.cmd = cmd;
    req.args = args;
    req.window = window_;
    req.tab = tab;
    req.cdp = tab->cdp;
    if (registry_.Dispatch(req)) { awaiting_user_ = (cmd == "browser_waiting_for_user_interaction"); return; }
    tool::ReplyError(id, "未知指令: " + cmd);
} // 参数在本线程解析（跨线程 CefValue 读不到）；启动未就绪时先排队等待 OnTabReady；ui_* 走外壳实现；browser_* 查注册表
