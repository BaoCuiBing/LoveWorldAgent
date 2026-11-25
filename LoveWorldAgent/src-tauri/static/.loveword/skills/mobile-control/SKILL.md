---
name: mobile-control
description: Android 移动端自动化：28 个 mobile_* 工具经 adb 操作真机/模拟器——设备与应用列表、前台应用、启动/停止/安装/卸载、屏幕尺寸与截图、uiautomator 元素树（@eN 引用 + bounds）、坐标点击/双击/长按/滑动与坐标缩放、按键与网址、文本输入、方向与模拟位置、剪贴板、logcat 与崩溃、录屏、批量命令。默认启用，随设置-内置工具-移动端开关联动
enabled: true
---



# 移动端操作（Android + adb）

## 何时使用

- 需要在 Android 真机或模拟器上操作 App、查看界面元素或截图
- 需要在设备上启动/停止/安装/卸载应用、读取日志或崩溃
- 需要录制设备屏幕或批量执行一组操作

## 关键约束

1. 设备必须已开启 USB 调试并授权；`mobile_list_available_devices` 只返回 `state=device` 的在线设备。
2. 多设备时必须显式指定 `device`（或用设置中的默认设备）；否则返回 `DEVICE_REQUIRED`。
3. 交互前先 `mobile_list_elements_on_screen` 取 `@eN` 引用；点击类工具优先传 `ref`，页面变化后旧引用失效。
4. 截图尺寸与设备物理分辨率可能不一致，坐标按比例换算；`mobile_get_screen_size` 可读真实分辨率。
5. 文本输入仅保证 ASCII；非 ASCII 走剪贴板广播 best-effort，失败返回 `UNSUPPORTED_INPUT`。
6. `mobile_set_location` 仅模拟器可用（`adb emu geo fix`）；真机返回 `UNSUPPORTED_INPUT`。
7. 截图与录屏落盘到 `static/data/mobile_screen`；录屏默认 180 秒上限。
8. adb 未安装或路径错误返回 `ADB_NOT_FOUND`；请到「设置 → 内置工具 → 移动端」填写 adbPath。

## 工具清单

| 工具 | 用途 |
| --- | --- |
| `mobile_list_available_devices` | 在线设备 serial/model |
| `mobile_list_apps` | 已安装应用包名（可选系统应用） |
| `mobile_get_foreground_app` | 当前前台组件与包名 |
| `mobile_launch_app` / `mobile_terminate_app` | 启动 / 停止应用 |
| `mobile_install_app` / `mobile_uninstall_app` | 安装 APK / 卸载应用 |
| `mobile_get_screen_size` | 物理分辨率与密度 |
| `mobile_click_on_screen_at_coordinates` | 点击坐标或 `ref` |
| `mobile_double_tap_on_screen` | 双击 |
| `mobile_long_press_on_screen_at_coordinates` | 长按 |
| `mobile_list_elements_on_screen` | uiautomator 元素树 |
| `mobile_press_button` | home/back/enter 等按键 |
| `mobile_open_url` | 系统浏览器打开网址 |
| `mobile_swipe_on_screen` | 方向或起止点滑动 |
| `mobile_type_keys` | 文本输入（可选回车） |
| `mobile_take_screenshot` / `mobile_save_screenshot` | 截图 base64 / 落盘 |
| `mobile_get_orientation` / `mobile_set_orientation` | 方向读取 / 设置 |
| `mobile_set_location` | 模拟器经纬度 |
| `mobile_clipboard` | 剪贴板读写 |
| `mobile_get_device_logs` | logcat 最近行 |
| `mobile_start_screen_recording` / `mobile_stop_screen_recording` | 录屏 |
| `mobile_list_crashes` / `mobile_get_crash` | 崩溃摘要 / 详情 |
| `mobile_batch_commands` | 批量执行（≤20 条） |

## 错误码

`DEVICE_NOT_FOUND`、`DEVICE_REQUIRED`、`ADB_NOT_FOUND`、`ADB_TIMEOUT`、`APP_NOT_FOUND`、`UNSUPPORTED_INPUT`、`INVALID_ARGUMENT`。
