// CEF UI 线程原生输入辅助（SendMouseClickEvent / SendMouseMoveEvent / SendKeyEvent / SetFocus）
// 注意：CEF 111 下 ImeCommitText 仅用于无窗模式(OSR)，本工程为窗口模式故绝不使用 ImeCommitText
#pragma once
#include <cstdint>
#include "include/cef_browser.h"
#include "include/cef_values.h"
#include "utils/js_builder.h"
namespace input {
uint32_t ParseCefModifiers(CefRefPtr<CefListValue> modifiers);
cef_mouse_button_type_t ParseMouseButton(const std::string& raw);
void SendNativeClick(CefRefPtr<CefBrowser> browser, int x, int y, cef_mouse_button_type_t button, int click_count, uint32_t cef_modifiers);
void SendNativeMove(CefRefPtr<CefBrowser> browser, int x, int y, uint32_t cef_modifiers);
void SendNativeKey(CefRefPtr<CefBrowser> browser, const js::KeyInfo& info, uint32_t cef_modifiers, bool send_char);
}
