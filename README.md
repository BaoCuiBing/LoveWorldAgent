# LoveWorldAgent（爱满天下）

Windows 桌面端 AI 编程与多端自动化智能体应用，基于 **Tauri v2 + Rust (axum) + goose GDK + Vue 3 (shadcn-vue)** 构建，并通过 Windows 命名管道联动 **内置浏览器 (`ai_browser`)、Office 办公 (`ai_office_option`)、Android 移动端 (`ai_mobile`) 与桌面操控 (`ai_computer_use`)** 四个独立 Sidecar。

---

## 1. 总体架构

```text
Tauri v2 主窗口（WebView2，标题「爱满天下」，1600×1000）
  ├─ 开发态：加载 Vite http://127.0.0.1:5173/（/api 与 WS 代理至 8866）
  └─ 冻结/生产态：同源加载 axum http://127.0.0.1:8866/（托管 webdist）
        │
        ▼
Rust 主后端（src-tauri，axum 0.8 + tokio + rusqlite）
  ├─ HTTP / SSE / WS：129 条 /api/* 路由 + 静态 SPA 回退
  ├─ 对话内核（goose =0.1.0-alpha.13）：状态机 + 8 组权限审批 + 三层上下文压缩
  ├─ 持久化（SQLite static/data/lwa.db）：sessions / messages / turns / usage / context / facts
  └─ Windows 命名管道（sidecar_pipe.rs，PeekNamedPipe 非死锁 + UTF-8/GBK 自适应解码）
        ├─ \\.\pipe\loveworld_browser_panel  ──▶ ai_browser (browser_panel.exe)
        ├─ \\.\pipe\loveworld_office_option  ──▶ ai_office_option (office_option.exe)
        ├─ \\.\pipe\loveworld_mobile         ──▶ ai_mobile (mobile_use.exe)
        └─ \\.\pipe\loveworld_computer_use   ──▶ ai_computer_use (computer_use.exe)
```

---

## 2. 核心子项目代码细节

### 2.1 `LoveWorldAgent/frontend`（Vue 3 前端）

- **技术栈与构建**：Vue 3.5 + Vite 6 + Tailwind CSS v4 + shadcn-vue（`reka-ui` + `class-variance-authority` + `lucide-vue-next`）+ `monaco-editor` + `@xterm/xterm` + `markdown-it` / `mermaid` / `highlight.js` + `docx-preview` / `xlsx` / `pptx-preview` / `pdfjs-dist`；构建输出至 `../webdist`（`base: './'`）。
- **页面与三栏布局**：
  - `src/router/index.js` 采用 Hash 路由（`/` → `views/Chat.vue`），`App.vue` 全局挂载启动健康轮询遮罩 `InitState.vue`、退出清理弹窗 `QuitState.vue`、确认对话框 `ConfirmHost.vue` 与通知 `Toaster.vue`。
  - `views/Chat.vue`：左栏会话管理（`views/chats/SessionList.vue`）、中栏消息流与输入框（`MessageListFather.vue` / `TurnItem.vue` / `SelfMessageSee.vue` / `AiMessageSee.vue` / `InputMessage.vue` / `InsertContext.vue`）、右栏四合一工作台（`views/RightTools.vue`）。
  - `AiMessageSee.vue` 与 `utils/chatEventReducer.js` 消费后端 SSE 直出的 goose `AgentEvent`（`message` / `usage` / `messageUsage` / `mcpNotification` / `historyReplaced`）；上下文压缩轮次（`isCompaction`）底部仅保留「重试」「删除」两个动作按钮。
- **右栏面板（`src/views/right_panel/`）**：
  - `AgencyList.vue`：任务待办清单 + 联动选中模型上下文上限的实时 Token 进度条 + 手动上下文压缩入口。
  - `BuildPanel.vue`：手风琴集成 `BroweringPanel.vue`、`ComputerUsePanel.vue`、`MobilePanel.vue`、`OfficeOptionPanel.vue`，并统一基于 `SidecarPanelBase.vue` 展示工具执行步骤与自动截图缩略图（截图不送入大模型）。
  - `FileCode.vue`：懒加载文件树（`FileCodeTree.vue`）+ 代码/Office 预览器（`FileCodeViewer.vue` → `CodeEditor.vue` / `OfficeView.vue`）。
  - `TerminalPanel.vue`：多标签 `xterm.js` 终端，单标签独立 `/api/terminal/ws` WebSocket 通道与本地标签持久化。
- **消息卡片分流（`src/components/md_show/PartNode.vue`）**：
  - 按分片类型与工具前缀路由到 `MdShow.vue`、`CommandCall.vue`、`FileEdit.vue`、`DiffChangesCard.vue`、`RuleCall.vue`、`SkillCall.vue`、`McpCall.vue`、`BuildToolCall.vue`（覆盖 `browser_*`/`computer_*`/`word_*`/`ppt_*`/`excel_*`/`mobile_*`）、`SubAgents.vue`、`UpdateList.vue`、`ConfirmationCard.vue`、`QuestionCard.vue`、`CompactionCard.vue`、`UnsupportedCard.vue`。
- **九页设置弹窗（`src/views/SetingDialog.vue`）**：
  - 规则（`RelusEdit`）、技能（`SkillEdit`）、对话（`ChatSseEdit`）、模型（`ModelEdit`）、系统（`SystemEdit`）、权限（`PermissionEdit`）、智能体（`AgentEdit`）、内置工具（`BuiltToolsEdit` 六标签：浏览器/电脑/Word/PPT/Excel/移动端）、MCP（`McpConfigEdit`）。

### 2.2 `LoveWorldAgent/ai_office_option`（Office 自动化 Sidecar）

- **定位与入口**：独立 Rust crate（包名 `office_option`，edition 2021，MSRV 1.88），不参与 `src-tauri` workspace；入口 `src/main.rs` 启用逐显示器 DPI 感知（`DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2`），仅支持 `pipe --pipe=loveworld_office_option` 模式。
- **COM 与并发模型**：
  - 基于 `windows 0.62` 的 COM `IDispatch` 晚绑定（`src/com/{app,constants,dispatch,document}.rs`），自动适配 Microsoft Office 与 WPS Office（`Word.Application`/`KWPS.Application`、`PowerPoint.Application`/`KWPP.Application`、`Excel.Application`/`KET.Application`）。
  - `src/common/sta.rs` 为 Word / PPT / Excel 三族分别维护独立 STA（Single-Threaded Apartment）工作线程队列，三族并发操作互不阻塞。
- **32 个内置工具**：
  - **Word（12 个，`src/tools/` + `src/word/`）**：`word_document`、`word_read`、`word_write`、`word_table`、`word_format`、`word_structure`、`word_find`、`word_edit`、`word_media`、`word_variable`、`word_batch`、`word_where_am_i`。
  - **PPT（10 个，`src/ppt/`）**：`ppt_presentation`、`ppt_read`、`ppt_slide`、`ppt_write`、`ppt_shape`、`ppt_table`、`ppt_format`、`ppt_chart`、`ppt_media`、`ppt_where_am_i`。
  - **Excel（10 个，`src/excel/`）**：`excel_workbook`、`excel_read`、`excel_write`、`excel_sheet`、`excel_format`、`excel_formula`、`excel_table`、`excel_chart`、`excel_sort_filter`、`excel_where_am_i`。
- **路径安全、备份与自动截图**：
  - 允许访问本机合法路径（仅拦截 UNC 网络路径、设备路径与 `..` 目录穿越）；保存前自动轮转 `.bak` 备份（`backup.keep` 默认 5 份）。
  - 配置落 `.loveword/agent_office.json`（支持三族独立 `auto_screenshot.{word,ppt,excel}` 开关），`src/common/screen.rs` 对可见窗口截图落 `static/data/office_screen/`，主进程剥离截图字段后仅向前端面板提供展示。

### 2.3 `LoveWorldAgent/ai_mobile`（Android 移动端 Sidecar）

- **定位与入口**：独立 Rust crate（包名 `mobile_use`，edition 2021，MSRV 1.88），入口 `src/main.rs` 走 `pipe --pipe=loveworld_mobile` 单模式，通过 `adb` CLI 驱动 Android 真机或模拟器。
- **核心模块（`src/mobile/`）**：`adb.rs`、`devices.rs`、`apps.rs`、`screen.rs`、`ui.rs`、`input.rs`、`coords.rs`、`logs.rs`、`recording.rs`。
- **28 个内置工具**：
  - **设备与信息**：`mobile_list_available_devices`、`mobile_get_screen_size`、`mobile_get_orientation`、`mobile_set_orientation`、`mobile_set_location`、`mobile_clipboard`。
  - **应用管理**：`mobile_list_apps`、`mobile_get_foreground_app`、`mobile_launch_app`、`mobile_terminate_app`、`mobile_install_app`、`mobile_uninstall_app`、`mobile_open_url`。
  - **UI 与输入交互**：`mobile_list_elements_on_screen`（`uiautomator` 元素树 `@eN` + 边界框）、`mobile_click_on_screen_at_coordinates`、`mobile_double_tap_on_screen`、`mobile_long_press_on_screen_at_coordinates`、`mobile_swipe_on_screen`、`mobile_press_button`、`mobile_type_keys`、`mobile_batch_commands`（含截图尺寸与设备分辨率自适应坐标缩放）。
  - **截图、录屏与诊断**：`mobile_take_screenshot`、`mobile_save_screenshot`、`mobile_start_screen_recording`、`mobile_stop_screen_recording`、`mobile_get_device_logs`、`mobile_list_crashes`、`mobile_get_crash`。
- **配置**：落 `.loveword/agent_mobile.json`（`adbPath`、`defaultDevice`、`screenshotMaxSize` 限制 320~4096，默认 1280）。

### 2.4 `LoveWorldAgent/ai_browser`（Qt6 + CEF 内置浏览器 Sidecar）

- **定位与入口**：C++17 + Qt6 Widgets + CEF 111.2.7 独立进程 `browser_panel.exe`（`CMakeLists.txt` 构建，静态 `/MT` 运行库），主应用按需拉起独立顶层窗口并通过命名管道 `loveworld_browser_panel` 通信。
- **源码分层（`src/`）**：
  - `main.cpp`：DPI 感知 → `AppUserModelID`（`com.loveworld.agent` 与主窗口任务栏分组）→ `CefExecuteProcess` 子进程分流 → Qt `QApplication` + CEF 10ms `QTimer` 消息泵 → 命名管道客户端。
  - `server/`：`pipe_client.cpp`、`agent_api.cpp`（窗口指令就地分派、`tab_ready_` 启动排队队列、`ui_*` 转外壳、`browser_*` 查注册表分发）、`client_app.cpp`、`app_handler.cpp`、`cdp_client.cpp`。
  - `uis/`：`qt_app_window.cpp`、`qt_theme.cpp`、`tab_manager.cpp`、`ui_chrome.cpp`、`cursor_overlay.cpp`（模拟鼠标箭头与点击涟漪叠加层）。
  - `components/`：自定义 Qt 控件集合（`bcb_*` 按钮、输入框、列表、菜单、标签页、弹窗、开关、树等）。
  - `browser_tool/`：21 个主应用暴露的 `browser_*` DOM/CDP/交互工具（`navigate`、`navigate_back`、`tabs`、`snapshot`、`read_text`、`take_screenshot`、`click`、`hover`、`type`、`select_option`、`press_key`、`get_attribute`、`scroll`、`wait_for`、`waiting_for_user_interaction`、`evaluate`、`handle_dialog`、`console_messages`、`network_requests` 等）以及 `ui_*` / `window_*` 外壳指令。
- **缓存与设置**：CEF 缓存与根缓存统一由主应用注入至 `static/additional/cache`，设置落 `.loveword/agent_browser.json`；日志写 `static/logs/browser_panel.log`。

---

## 3. 目录与文档索引

```text
e:\BiSaiCodeProjects\
├── LoveWorldAgent\
│   ├── frontend\          # Vue 3 + shadcn-vue 前端源码
│   ├── src-tauri\         # Tauri v2 + Rust/axum 主后端与静态配置 static/
│   ├── ai_browser\        # C++17 + Qt6 + CEF 111 内置浏览器 sidecar
│   ├── ai_office_option\  # Rust COM Office 自动化 sidecar（Word/PPT/Excel）
│   ├── ai_mobile\         # Rust adb Android 移动端 sidecar
│   ├── ai_computer_use\   # Rust UIA + OmniParser 桌面自动化 sidecar
│   └── webdist\           # 前端构建输出目录
├── mds\                   # 项目设计文档与接口速查
│   ├── README.md          # 本文档
│   ├── 项目设计.md        # 总体设计基线与文档索引
│   ├── 路由接口文档.md    # 全部 /api/* 路由接口速查
│   └── projects_mds\      # 项目概述、项目结构、技术栈、开发环境搭建、启动与运行、核心模块开发、注意事项
└── release\               # 绿色便携分发目录（loveworldagent.exe + webdist + static + browser）
```

---

## 4. 本地构建与便携分发

### 4.1 构建各组件

```powershell
# 1. 前端构建（输出至 LoveWorldAgent/webdist）
cd e:\BiSaiCodeProjects\LoveWorldAgent\frontend
npm install
npm run build

# 2. 编译 Rust Sidecar（生成 exe 并同步到 src-tauri/static/）
powershell -ExecutionPolicy Bypass -File e:\BiSaiCodeProjects\LoveWorldAgent\ai_office_option\build.ps1
powershell -ExecutionPolicy Bypass -File e:\BiSaiCodeProjects\LoveWorldAgent\ai_mobile\build.ps1
powershell -ExecutionPolicy Bypass -File e:\BiSaiCodeProjects\LoveWorldAgent\ai_computer_use\build.ps1

# 3. 编译主程序 Release
cd e:\BiSaiCodeProjects\LoveWorldAgent\src-tauri
cargo build --release
```

### 4.2 绿色便携目录结构（`release/`）

`src-tauri/src/config.rs` 当检测到 `exe` 同级目录同时存在 `webdist/` 与 `static/` 时自动启用冻结态路径（使用同级目录，不回退源码树）：

```text
release\
├── loveworldagent.exe     # 主程序
├── webdist\               # 前端构建产物
├── browser\               # ai_browser/out/Release 全部 CEF + Qt6 运行时与 browser_panel.exe
└── static\                # 随包静态配置与 Sidecar 播种源（.loveword/、config.json、imgs/、computer_use/、office_option/、mobile/）
```

更多细节请参阅 [项目设计.md](./项目设计.md) 与 [路由接口文档.md](./路由接口文档.md)。
