# 内置浏览器工具参考

> 与 `SKILL.md` 配套：这里列出 21 个 `browser_*` 工具的用途、关键参数与返回形态；底层由 `backend/agent/tools.py` 的 `build_browser_tools()` 注册，经 `backend/utils/browser.py` 的管道 v2 转发给 CEF sidecar。

## 工具表

| 工具 | 用途 | 关键参数 | 返回要点 |
| --- | --- | --- | --- |
| `browser_open` | 启动或复用窗口并导航到网址 | `url` | `{ok, state}`，含窗口与页面状态 |
| `browser_navigate` | 当前标签导航 | `url`（http/https/file） | 页面状态 |
| `browser_navigate_back` | 后退 | — | 页面状态 |
| `browser_tabs` | 标签管理 | `action=list/new/close/select`、`index`、`url` | 标签列表或操作回执 |
| `browser_snapshot` | 取结构化页面快照 | — | 元素树与可交互 `ref`（如 `e12`） |
| `browser_read_text` | 取页面纯文本 | — | 文本内容 |
| `browser_take_screenshot` | 页面或元素截图 | `filename`、`type=png`、`full_page`、`ref` | 图片绝对路径 |
| `browser_click` | 点击元素 | `ref`、`double_click`、`button`、`modifiers` | 操作回执 |
| `browser_hover` | 悬停元素 | `ref` | 操作回执 |
| `browser_type` | 输入文本 | `ref`、`text`、`slowly`、`submit` | 操作回执 |
| `browser_select_option` | 选择下拉选项 | `ref`、`values` | 操作回执 |
| `browser_press_key` | 发送按键 | `key`（Enter/Escape/Tab/方向键等） | 操作回执 |
| `browser_get_attribute` | 读取元素属性 | `ref`、`attribute`（href/class/value） | 属性值 |
| `browser_scroll` | 滚动页面或元素 | `direction`、`amount`、`ref` | 操作回执 |
| `browser_wait_for` | 等待时间/文本出现/文本消失 | `time`、`text`、`text_gone` | 等待结果 |
| `browser_waiting_for_user_interaction` | 登录/验证码交还用户 | `reason` | 用户完成后的状态 |
| `browser_evaluate` | 在页面执行 JS 函数体 | `function`、`ref` | 执行结果 |
| `browser_handle_dialog` | 应答 JS 弹窗 | `accept`、`prompt_text` | 操作回执 |
| `browser_console_messages` | 读控制台日志 | `level`、`only_errors` | 日志列表 |
| `browser_network_requests` | 读网络请求 | `include_static`、`filter` | 请求列表（首次开启监听） |
| `browser_state` | 读本地浏览器状态 | — | running/pipe/visible/title/address/loading/tabs/dialog/awaitUser |

## 失效与恢复

1. **ref 失效**：导航、展开、异步渲染、弹窗、标签切换后旧 `ref` 全部作废；重新 `browser_snapshot()`，不要凭记忆构造 ref。
2. **窗口被关闭**：✕ 只隐藏不退出，`browser_state()` 中 `visible=false`；再次调用任一交互工具或 `browser_open()` 即可复现，无需重启应用。
3. **管道断开**：`utils/browser.py` 会自愈一次；若仍失败，用 `browser_state()` 确认 `pipe` 字段，再决定是否让用户重新打开窗口。
4. **超时**：`browser_wait_for` 页面内最长等 30 秒；工具层默认 20~35 秒。超时后先读状态，不要连续重试。
5. **登录/验证码**：一律用 `browser_waiting_for_user_interaction(reason)` 交还用户；用户完成后重新取快照，禁止尝试自动绕过验证。
6. **审批拒绝**：权限组「内置浏览器」内该工具被标灰时会中断等待批准；被拒后不重试同一工具。

## 取证产物

- 截图目录：`src-tauri/static/data/browser_screen`，命名 `browser_<yyyyMMdd_HHmmss_SSS>.png`。
- 右栏「其他」页由 `Chat.vue` 自动登记截图等工具产出；`GET /api/browser/screenshot` 仅允许读该目录内的图片。
- 步骤流：`GET /api/browser/steps` 记录每次工具调用的 seq、参数、结果与截图，供排障回放。
