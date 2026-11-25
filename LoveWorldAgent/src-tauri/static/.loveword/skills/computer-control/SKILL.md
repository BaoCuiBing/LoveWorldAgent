---
name: computer-control
description: Windows 桌面自动化：与平台参考对齐的 23 个 computer_* 工具（UIA snapshot_id/element_token + omni_parser 内存视觉解析 OCR 与图标框、窗口与进程管理、原生菜单调用、键鼠前后后台控制），绝不返回或落盘直接截图。默认关闭，随设置-内置工具-电脑开关联动
enabled: true
---

# 电脑控制使用指引（23 工具 · omni_parser 视觉解析）

## 什么时候启用 / 什么时候不要用
- 先确认权限配置里「电脑」总开关已开启；能用 `command_run` / `file_*` / `browser_*` 完成的任务不要退回电脑控制。

## 23 个对齐工具总览
- 交互输入（10 个）：`computer_click`、`computer_double_click`、`computer_right_click`、`computer_drag`、`computer_type_text`、`computer_press_key`、`computer_hotkey`、`computer_scroll`、`computer_set_value`、`computer_move_cursor`
- 应用与窗口管理（7 个）：`computer_list_apps`、`computer_list_windows`、`computer_launch_app`、`computer_kill_app`、`computer_bring_to_front`、`computer_set_window_frame`、`computer_invoke_menu`
- 状态感知与系统探针（6 个）：`computer_get_window_state`、`computer_get_desktop_state`、`computer_debug_window_info`、`computer_get_accessibility_tree`、`computer_get_screen_size`、`computer_get_cursor_position`

## 标准闭环（UIA + omni_parser）
1. 发现目标：`computer_list_apps` / `computer_list_windows` / `computer_get_accessibility_tree` 拿到 `pid` 与 `window_id`；未运行则 `computer_launch_app`。
2. 感知窗口：`computer_get_window_state(pid, window_id)` 返回 `snapshot_id`、带 `element_token` 的 UIA 树，以及内存图像经 `omni_parser` 识别的 `omni_elements`（含 OCR 文本与图标检测框中心 `x/y` 与 `screen_x/screen_y`），**永不返回截图或写盘图片**。
3. 执行操作：优先传 `element_token`（旧快照在同窗下一次 `get_window_state` 后自动失效）；无 UIA 节点时用 `omni_elements` 坐标；无目标窗口的桌面全局点击用 `computer_get_desktop_state` + `scope:"desktop"`。
4. 窗口与菜单：需展开原生菜单用 `computer_invoke_menu`；最小化窗口恢复置前用 `computer_bring_to_front`；调整窗口坐标宽高用 `computer_set_window_frame`。
5. 复核与容错：操作后再次 `computer_get_window_state` 验证；后台点击无效再改 `delivery_mode:"foreground"`；UAC/管理员窗口受 UIPI 拦截时中止并提示用户。

## 模块结构
工具契约与实现位于 `ai_computer_use/src/server/{agent_api,apps,actions,state_tools,snapshots}.rs`，主应用注册见 `src-tauri/src/agent/tools_details/computer_use_tools.rs`。
