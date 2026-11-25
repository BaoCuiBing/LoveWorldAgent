// ClientApp 实现：上下文就绪时创建 Qt 面板窗口并登记管道目标
#include "server/client_app.h" // 自身声明
#include "include/wrapper/cef_helpers.h" // CEF_REQUIRE_UI_THREAD
#include "uis/app_window.h" // AppWindow 接口
#include "uis/qt_app_window.h" // Qt 实现
#include "server/pipe_client.h" // SetTarget
HWND ClientApp::panel_hwnd_ = nullptr; // 静态句柄初始化
AppOptions g_options; // 全局启动参数
ClientApp::ClientApp() = default; // 使用默认构造
void ClientApp::OnContextInitialized() { CEF_REQUIRE_UI_THREAD(); if (panel_hwnd_ != nullptr && ::IsWindow(panel_hwnd_)) return; AppWindow* panel = QtAppWindow::Create(); if (panel == nullptr) return; panel_hwnd_ = panel->hwnd(); PipeClient::SetTarget(panel_hwnd_); } // 幂等创建面板：已存在直接返回；否则创建并设置管道投递目标
