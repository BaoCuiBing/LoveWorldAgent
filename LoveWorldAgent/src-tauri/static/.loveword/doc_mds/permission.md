# 工具权限（Permission）使用指南

工具权限决定智能体能「做什么」：三种模式控制整体审批策略，五个权限组分别覆盖 MCP 工具、PowerShell 命令、内置浏览器工具、电脑控制与 Word 操作。配置保存在 `.loveword/agent_permission.json`（整个文件即权限对象），在设置页「权限」修改后下一轮对话即生效。

## 三种权限模式

| 模式 | 值 | 行为 |
| --- | --- | --- |
| 手动审批 | `manual` | 每一步工具调用都弹出确认条，允许/拒绝后才执行 |
| 自动审批 | `auto` | 仅命中组内灰条目、或含输出重定向的命令需要批准，其余自动放行（默认） |
| 完全访问 | `full` | 全部工具直接执行，不再弹审批（高风险，仅建议在隔离环境使用） |

默认模式为 `auto`。

## 五个权限组

| 键 | 名称 | 内容 | 默认状态 |
| --- | --- | --- | --- |
| `mcp` | MCP工具 | 占位，暂无工具（与「无 MCP」边界一致） | 空组 |
| `powershell` | PowerShell | 19 条高风险命令：`format C:`、`diskpart`、`del /f /s /q`、`rm -rf *`、`git push --force`、`IEX DownloadString`、`Add-MpPreference -ExclusionPath`、`taskkill`、`reg delete` 等 | 全灰（需批准） |
| `browser` | 内置浏览器 | 21 个 `browser_*` 工具注册名（`browser_open`、`browser_snapshot`、`browser_click` … `browser_state`） | 全蓝（自动放行） |
| `computer` | 电脑控制 | 12 个 `computer_*` 工具（列表/启动/定位/状态树/点击/滚动/拖动/输入/按键/语义动作/设值/选区） | 全灰（需批准） |
| `word` | Word操作 | 12 个 `word_*` 工具（文档/读取/写入/表格/格式/结构/查找/编辑/媒体/变量/批量/位置），以完整 path 定位文档，随内置工具 Word 开关联动 | 全蓝（自动放行） |

设置页中蓝色徽标 = 自动放行，灰色 = 需你批准；点徽标逐条切换，整组开关批量置开/关。「自定义白名单」的条目过长时以省略号显示，鼠标悬浮可看全文。

## 本轮行为变更（重要）

- `auto` 下 **PowerShell 未列出的命令默认放行**：旧版 `readonly` 组（56 条只读命令）已移除，`dir`、`git status`、`node --check` 等不再逐条弹审批。
- **输出重定向仍强制审批**：剔除 `2>&1` 这类纯 stderr 合并后仍有 `>` 的命令会写文件/改配置，即使未命中任何组也要批准。
- **内置浏览器有真实门禁**：`build_browser_tools()` 给每个工具挂上 `needs_approval`，按 `browser` 组的蓝/灰逐条判定；`confirm` 确认条会显示工具专属理由，而不是命令白名单文案。
- `manual` 仍每一步都中断；`full` 不检查任何组。`always`（永久允许）会把命令或工具名写进 `custom`，之后免审批。

## 配置示例

`.loveword/agent_permission.json`：

```json
{
  "mode": "auto",
  "groups": {
    "mcp": { "label": "MCP工具", "enabled": true, "commands": {} },
    "powershell": { "label": "PowerShell", "enabled": true, "commands": { "taskkill": false } },
    "browser": { "label": "内置浏览器", "enabled": true, "commands": { "browser_click": true } },
    "computer": { "label": "电脑控制", "enabled": true, "commands": {} }
  },
  "custom": []
}
```

- 组定义以文件里的 `groups` 为唯一事实源；文件缺失时后端用内置四组骨架兜底。
- `custom` 为命令或工具名数组，「永久允许」时自动追加，命中后即使标灰也免审批。
- 权限文件用「临时文件 + `os.replace`」原子写，失败不影响原配置。

## API 读写

```
GET  /api/permission              # 返回 mode/groups/custom + groups_meta（4 组定义）
PUT  /api/permission              # body: {mode, groups, custom}
POST /api/permission/check        # body: {command} -> {allowed, reason}
POST /api/permission/allow        # body: {command} -> {ok}，追加 custom
```

- `PUT` 做归一化：`mode` 只接受 `manual/auto/full`；`groups` 按文件组定义展开逐条布尔；`custom` 仅保留非空字符串。
- `/check` 的判定与工具执行完全一致，命令卡「在终端查看」会先调它。

## 与对话区审批的联动

- 命中需批准项时 SSE 下发 `confirm` + `paused`，确认条出现在对应工具卡下方；允许后 `POST /api/chat/stream {resume}` 续流。
- 未决审批在内存中保留 6 小时，超时或后端重启后确认条标记「审批已失效」并禁用按钮。
- 权限页与底部审批 chip 共用同一数据源，改完即时同步。

## FAQ

**Q：为什么 `dir` 不再弹审批？**
A：四组重构后 `auto` 模式采用「未列默认放行」，只有 PowerShell 组里标灰的高风险命令和含输出重定向的命令才需要批准。

**Q：怎么让某个浏览器工具每次都问我？**
A：在「自动审批」→「内置浏览器」组里把该工具徽标点成灰色即可；反向操作可重新放行。

**Q：改了权限要重启吗？**
A：不用。接口直接写权限文件，`permission.py` 每次判定都重新读盘，前端重新拉取即可。

**Q：`full` 模式安全吗？**
A：不安全。它会跳过全部审批，仅在可信、隔离的测试环境使用。
