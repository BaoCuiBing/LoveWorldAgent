---
name: browser-automation
description: 驱动内置浏览器完成网页访问、页面交互、内容读取、控制台与网络取证和截图；当任务需要打开网页、点击或填写元素、核对页面状态、遇到登录验证码或需要取证时使用。
enabled: true
---



# 内置浏览器操作

用 21 个 `browser_*` 工具驱动独立的 CEF 浏览器窗口（标题固定「内置浏览器」）。首次调用任一交互工具会自动拉起窗口；窗口的 ✕ 只隐藏保活，再次调用即可复用。工具结果统一为 JSON 字符串。

## 标准流程

1. **打开与导航**：用 `browser_open(url)` 一步完成「启动 + 导航」；已运行时用 `browser_navigate(url)` 换页、`browser_navigate_back()` 后退、`browser_tabs(action, index, url)` 管理标签（list/new/close/select）。
2. **取快照**：用 `browser_snapshot()` 取当前页面的结构化元素与 `ref`（形如 `e12`）。任何 click/hover/type/select/scroll/get_attribute/evaluate 之前都必须先取快照。
3. **交互**：按 `ref` 调 `browser_click`、`browser_hover`、`browser_type`、`browser_select_option`、`browser_press_key`、`browser_scroll`、`browser_get_attribute`、`browser_evaluate`。页面结构变化（导航、展开、异步加载、弹窗）后旧 `ref` 立即失效，必须重新 `browser_snapshot()`，不得复用旧 ref 猜测。
4. **等待与弹窗**：用 `browser_wait_for(time/text/text_gone)` 等待加载与文本出现；JS 确认框用 `browser_handle_dialog(accept, prompt_text)`。
5. **取证**：用 `browser_read_text()` 读纯文本、`browser_take_screenshot()` 截图、`browser_console_messages()` 看控制台、`browser_network_requests()` 看请求（首次调用开启监听，需再次调用取记录）、`browser_state()` 核对运行/标题/地址/加载/标签状态。
6. **登录交还**：遇登录、验证码、人机验证等需人工判断的场景，调 `browser_waiting_for_user_interaction(reason)` 唤起窗口并把控制权交还用户；明确告知用户在窗口中完成后再继续，之后重新 `browser_snapshot()`，不要尝试绕过验证。

## 工具速查

| 分类 | 工具 |
| --- | --- |
| 导航与标签 | `browser_open`、`browser_navigate`、`browser_navigate_back`、`browser_tabs`、`browser_state` |
| 快照与读取 | `browser_snapshot`、`browser_read_text`、`browser_get_attribute`、`browser_console_messages`、`browser_network_requests` |
| 交互 | `browser_click`、`browser_hover`、`browser_type`、`browser_select_option`、`browser_press_key`、`browser_scroll`、`browser_evaluate`、`browser_handle_dialog` |
| 等待与交还 | `browser_wait_for`、`browser_waiting_for_user_interaction` |
| 取证 | `browser_take_screenshot` |

逐工具的用途、关键参数、返回值与失效处理见同目录 `reference.md`。

## 约束

- 所有 `browser_*` 工具受权限页「内置浏览器」组逐条门禁：蓝色自动放行、灰色需用户批准。审批被拒后不要重试同一工具，改用其它取证路径或向用户提问。
- 截图落在 `src-tauri/static/data/browser_screen`，文件名为 `browser_<yyyyMMdd_HHmmss_SSS>.png`，右栏「其他」页会自动登记；登记前不要声称已取证。
- `browser_state()` 只读本地状态、不拉起窗口；`browser_open()` 会启动或复用窗口。不要重复启动窗口。
- 读取页面内容与控制台日志属于只读操作；`browser_evaluate` 会在页面内执行 JS，仅在确有需要时使用并如实回报结果。
- 工具返回 `ok:false` 或超时时，先 `browser_state()` 判断窗口与管道状态，再决定是否重新导航；不要连续重试同一失败调用。
