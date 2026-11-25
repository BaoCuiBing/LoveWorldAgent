// input_helper 实现：在 CEF UI 线程通过 CefBrowserHost 派发真实 Chromium 鼠标/键盘事件
#include "browser_tool/input_helper.h"
#include <windows.h>
#include <cctype>
#include "include/wrapper/cef_helpers.h"
namespace input {
namespace {
std::string Lower(std::string s) {
    for (char& ch : s) ch = static_cast<char>(std::tolower(static_cast<unsigned char>(ch)));
    return s;
}
}  // namespace

uint32_t ParseCefModifiers(CefRefPtr<CefListValue> modifiers) {
    uint32_t flags = 0;
    if (modifiers == nullptr) return flags;
    for (size_t i = 0; i < modifiers->GetSize(); ++i) {
        const std::string m = Lower(modifiers->GetString(i).ToString());
        if (m == "ctrl" || m == "control") flags |= EVENTFLAG_CONTROL_DOWN;
        else if (m == "alt") flags |= EVENTFLAG_ALT_DOWN;
        else if (m == "shift") flags |= EVENTFLAG_SHIFT_DOWN;
        else if (m == "meta" || m == "command" || m == "cmd") flags |= EVENTFLAG_COMMAND_DOWN;
    }
    return flags;
}

cef_mouse_button_type_t ParseMouseButton(const std::string& raw) {
    const std::string low = Lower(raw);
    if (low == "right") return MBT_RIGHT;
    if (low == "middle") return MBT_MIDDLE;
    return MBT_LEFT;
}

void SendNativeClick(CefRefPtr<CefBrowser> browser, int x, int y, cef_mouse_button_type_t button, int click_count, uint32_t cef_modifiers) {
    CEF_REQUIRE_UI_THREAD();
    if (browser == nullptr) return;
    CefRefPtr<CefBrowserHost> host = browser->GetHost();
    if (host == nullptr) return;
    host->SetFocus(true);
    CefMouseEvent ev;
    ev.x = x;
    ev.y = y;
    ev.modifiers = cef_modifiers;
    host->SendMouseMoveEvent(ev, false);
    host->SendMouseClickEvent(ev, button, false, 1);
    host->SendMouseClickEvent(ev, button, true, 1);
    if (click_count >= 2) {
        host->SendMouseClickEvent(ev, button, false, 2);
        host->SendMouseClickEvent(ev, button, true, 2);
    }
}

void SendNativeMove(CefRefPtr<CefBrowser> browser, int x, int y, uint32_t cef_modifiers) {
    CEF_REQUIRE_UI_THREAD();
    if (browser == nullptr) return;
    CefRefPtr<CefBrowserHost> host = browser->GetHost();
    if (host == nullptr) return;
    CefMouseEvent ev;
    ev.x = x;
    ev.y = y;
    ev.modifiers = cef_modifiers;
    host->SendMouseMoveEvent(ev, false);
}

void SendNativeKey(CefRefPtr<CefBrowser> browser, const js::KeyInfo& info, uint32_t cef_modifiers, bool send_char) {
    CEF_REQUIRE_UI_THREAD();
    if (browser == nullptr) return;
    CefRefPtr<CefBrowserHost> host = browser->GetHost();
    if (host == nullptr) return;
    host->SetFocus(true);
    const UINT scan = ::MapVirtualKeyW(static_cast<UINT>(info.vk), MAPVK_VK_TO_VSC);
    CefKeyEvent down = {};
    down.type = KEYEVENT_RAWKEYDOWN;
    down.windows_key_code = info.vk;
    down.native_key_code = static_cast<int>((scan << 16) | 1);
    down.modifiers = cef_modifiers;
    host->SendKeyEvent(down);
    const bool has_ctrl_or_alt = (cef_modifiers & (EVENTFLAG_CONTROL_DOWN | EVENTFLAG_ALT_DOWN)) != 0;
    if (send_char && info.ch != 0 && !has_ctrl_or_alt) {
        CefKeyEvent ch = {};
        ch.type = KEYEVENT_CHAR;
        ch.windows_key_code = static_cast<int>(info.ch);
        ch.native_key_code = down.native_key_code;
        ch.character = info.ch;
        ch.unmodified_character = info.ch;
        ch.modifiers = cef_modifiers;
        host->SendKeyEvent(ch);
    }
    CefKeyEvent up = down;
    up.type = KEYEVENT_KEYUP;
    up.native_key_code |= static_cast<int>(0xC0000000);
    host->SendKeyEvent(up);
}
}  // namespace input
