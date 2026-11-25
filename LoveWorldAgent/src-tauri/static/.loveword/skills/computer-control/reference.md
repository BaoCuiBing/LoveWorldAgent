# computer-control 参考

## 运行形态

- sidecar：`ai_computer_use`（包名 `computer_use.exe`，内置工具），开发态产物 `ai_computer_use/target/{debug,release}/`，打包态 `app_dir/computer_use/`
- 接入方式：主应用创建命名管道 `loveworld_computer_use`，首次调用任一 `computer_*` 时拉起 `pipe --pipe=loveworld_computer_use`，进程复用，应用退出时整树结束
- 协议：`{"type":"req","id":N,"cmd":"computer_click","args":{...}}` → `{"type":"res","id":N,"ok":true,"data":{...}}`；大响应按 `{"chunks":K,"chunk":i,"data":"片段"}` 分片，主应用按 id 拼接
- 日志：`static/logs/computer_use.log`（与主应用 app.log 同格式，达 `logging.max_lines` 归档 `old_N_`）；INFO 记录启动 pid/DPI、每次请求 id/cmd、工具耗时、断线与重连
- 坐标：进程声明 Per-Monitor V2 DPI 感知；click/scroll/drag 与 vision 的对外坐标统一为**窗口相对物理像素**（原点=窗口外框左上角），可用 `coordinate_space{width,height}` 归一化；grounding 的 x/y 为 0~1000 归一化

## 工具契约

| 工具 | 必填 | 说明 |
| --- | --- | --- |
| computer_list_apps | 无 | 运行进程 + 开始菜单 lnk + UWP/AUMID（`Get-StartApps`，缓存 60s）；返回 `app_id/name/pid?/path/kind` |
| computer_launch_app | app | 先按 app_id、再按名称/lnk/exe 解析；同 app 已有窗口则激活复用；返回 `app_id/pid?/windowId?/activated?` |
| computer_grounding | app 或 pid 二选一、windowId、target、routing | `routing.fallback_reason` 五选一，除 `visual_only` 外必须带 `ax_target{name,role}`，`ax_action_failed` 还需 `failed_element_id`；返回 `AX_TARGET_AVAILABLE`(element_id) 或 `found/not_found`(normalized_1000 辅助) |
| computer_get_app_state | pid 或 app | `windowId` 可选；返回树（默认 diff，深度 25）+ `foreign_child_windows` + `focused/warning` + `window{dpi,scale,workArea}`；`vision:true` 追加窗口相对 `point/text`（上限 500）；`disableScreenshot` 仅为兼容占位；不返回截图 |
| computer_click | element_id 或 x+y+windowId | 坐标一律窗口相对像素，`coordinate_space{width,height}` 可归一化；带 element_id 可传 x/y 校准并校验范围，否则点元素中心；`foreground:true` 强制前台，默认后台优先且前台回退前激活目标；越界拒绝 |
| computer_scroll | element_id 或 x+y+windowId | 带 element_id 优先 ScrollPattern，失败用滚轮；`deltaX/deltaY` 精细滚动（正上/正右），`pages/direction` 方向滚动；`foreground:true` 强制前台 |
| computer_drag | fromX/fromY/toX/toY + element_id 或 windowId | 窗口相对坐标，20 步平滑、300ms，越界拒绝；`foreground:true` 强制前台 |
| computer_type_text | text | 焦点输入（对齐内置）：element_id 仅聚焦提示，不再后台写值；默认 10ms/键，slowly=80ms；默认输入后恢复原焦点，foreground=true 保持激活 |
| computer_press_key | key | `modifiers` 小写 ctrl/alt/shift（cmd→ctrl）；Win 键被屏蔽；带 element_id 且未指定 foreground 时先后台投递；`foreground:true` 强制前台 |
| computer_perform_action | element_id、action | 8 类：invoke/toggle/select/expand/collapse/set_value/set_focus/scroll_into_view；除 invoke 外动作后校验状态，未生效报错 |
| computer_set_value | element_id、value | ValuePattern 写入后回读校验；只读、不支持或回读不一致时报错（建议改用 click+type_text） |
| computer_get_value | element_id | 读取 ValuePattern 当前值；不支持时改用树内 `val=` |
| computer_select_text | element_id、text | TextPattern 查找 + prefix/suffix 消歧；`text/cursor_before/cursor_after`；不支持 TextPattern 时回退 ValuePattern+键盘 |

## 视觉识别（vision）

- 触发：`computer_get_app_state {vision:true}`。
- 模型：`ms_vision/icon_detect.onnx`（v2 图标检测）+ `ocr/ch_PP-OCRv4_det/rec_infer.onnx` + `ppocr_keys_v1.txt`（中英数字 OCR）。
- GPU：CUDA EP，4GB arena 上限、HEURISTIC、workspace=0、SameAsRequested；实测峰值约 1.9GB。
- 返回：`vision:[{point:[x,y],text}]`，point 为元素中心的**窗口相对坐标**（原点=窗口外框左上角，默认窗口物理像素；传 `coordinate_space:{width:1000,height:1000}` 则为归一化）；已跳过空文本；上限 500 项，超出置 `vision_truncated`。
- 失败：`vision:[]` + `vision_error`，不阻塞 UIA 树。
- 用法：`point` 直接作为 `computer_click/scroll` 的 `x/y` 并传目标 `windowId`（get_app_state 传了 `coordinate_space` 时 click 传同一个）。
- 适用：UIA 树稀疏、自定义控件（酷狗/QQ/微信）、`grounding:not_found`、需要 OCR 读取图片文本。

## 常见失败

| 现象 | 原因 | 处理 |
| --- | --- | --- |
| element_id 不存在或过期 | 窗口结构变化或 sidecar 重启 | 重新 computer_get_app_state，使用最新树里的 ID |
| 元素离屏/无矩形 | 窗口最小化、被遮挡或未激活 | 先 computer_launch_app 激活或 computer_get_app_state 复核 |
| AX_TARGET_AVAILABLE | UIA 有可用目标，未给视觉坐标 | 直接用返回的 element_id 执行 click/perform_action |
| not_found | 目标不在当前 AX 树 | 重新观察或滚动后再 grounding；仍无则开 `vision:true` |
| 坐标不在窗口范围 | 传了旧版绝对屏幕坐标，或超出 coordinate_space | 改用窗口相对像素（原点=窗口外框左上角）；归一化坐标需同时传 `coordinate_space{width,height}` |
| set_value 报“写入后回读不一致” | 自绘/RichEdit/Electron 拒绝 ValuePattern | 改用 `click`（element_id）+ `type_text`；或 `get_value` 回读确认 |
| get_value 报不支持 ValuePattern | 控件只有 TextPattern/RangeValue | 改用 `get_app_state` 树内 `val=` 或 `select_text` 读取 |
| 点击已关注/关注无变化 | 确认弹窗延迟 1~2s 才出现 | 点击后等待再 `get_app_state(vision=true)`，查「确定/取消」坐标并点确定 |
| 右键菜单项点不中 | 点击前激活主窗口导致菜单关闭 | 已修：同 pid 前台跳过激活；仍异常时用 UIA `menu` 节点或键盘选择 |
| 双击结果跳到 MV 浮层 | 客户端自开 `kugou_ui_float`/`kugou_mv_win` | 主窗口 `focused=false` 属正常；用页面「返回」或左上角收起箭头退出 |
| 侧栏「我的收藏/我喜欢」不跳转 | hover 信息卡拦截单击 | 右键出「歌单菜单」→「播放」；或改用页面内入口 |
| `— □ ×` 点击变成最小化 | vision 把组合按钮识别为一段文本 | 先截图放大；窗口控制优先用 UIA `button "最小化/最大化/关闭"` |
| set_value 返回 `0x80004001` | 酷狗等 webview 元素拒绝 SetValue | 进度条改用 `computer_drag` 或坐标点击；弹幕框先悬浮视频区域，再用 `vision:true` 找输入框 |
| element_id 点击返回成功但无变化 | 后台投递被 webview 忽略（无回执） | 加 `foreground:true` 强制前台激活后点击；或用坐标点击并 vision 复核 |
| 点击无反应 | 窗口未激活或被遮挡 | 先激活窗口，再取新状态复核 |
| 截图全黑 | 窗口受 DRM/远程桌面保护 | 改用 UI 树文本或 `vision`（同样受限） |
| vision 报模型缺失 | `models/{ocr,ms_vision}` 或 CUDA DLL 缺失 | 重新执行 `build.ps1`，检查 `cuda_runtime` |
| vision 显存不足 | 其他程序占用 GPU 或 arena 超限 | 关闭占显存程序；确认 CUDA EP 4GB 上限生效 |
| 操作管理员窗口失败 | UIPI 限制 | 提示用户以管理员身份运行应用 |
| launch_app 报 `program not found` | 名称解析失败（如 `powerpnt`） | 换 `PowerPoint`/`excel` 等显示名；仍失败用 Shell 直启 exe+文件参数 |
| Office 启动后无文档窗口 | launch_app 只到启动页/开始页 | 主窗内 Ctrl+O→「浏览」→`#32770` 文件名框 type 完整路径→Enter |
| 弹窗内容 vision 读不到 | 弹窗与主窗不同屏；vision 只截目标窗 rect | `get_app_state` 直接以弹窗 `windowId` 为目标 |
| 对话框确定/颜色/树项后台 invoke 无效 | Office 自绘控件拒绝后台投递（关窗不生效） | 用 `click element_id` 返回的 `point` 前台坐标点击；`disableDiff` 树看 `selected` 复核 |
| 排序结果首行不动 | 选区未含标题行，首行被当表头 | 选区从标题行起（如 A3:F8），勾「数据包含标题」 |
| 排序报「所有排序条件必须指定一个列或行」 | 「列」下拉为空 | 展开列下拉（显示为首行数据值）选中再确定 |
| Excel 插入的是迷你图/瀑布图 | 「柱形」标签属迷你图组；柱形图按钮下拉是其他图表 | 选区后 Alt+F1 直接插默认簇状柱形图 |
| Shift+点击扩展选区无效 | click 工具无 modifiers 参数 | 名称框输 `A1:G1` 范围回车，或键盘 Shift+方向键 |
| PPT 新建幻灯片按钮点击无效 | ribbon 按钮命中失败 | 右键缩略图→「新建幻灯片」；版式用 Alt→H→L 键序列 |
| 保存复核不可靠 | UI 无法确认写入 | 离线 zipfile 读 docx/xlsx/pptx XML 断言（transition/hlinkClick/公式/文本） |
| Word 选标题带出表格 | `Ctrl+Shift+↓` 按段扩选，标题后紧邻表格时被包含 | 改 `Home`+`Shift+End` 只选当前行 |
| Word 字号输入落到「字形」 | 点「字号」wrapper 未移动焦点，焦点仍在字形 RichEdit | 点字号框的 RichEdit 子元素，确认 diff 里 `(focused)` 在字号框再输入数字磅值（15=小三） |
| Excel 排序「排序依据/要应用的格式」打不开 | combo 拒绝 set_value/箭头/Alt+Down | `perform_action expand` 出 `REListBox20W`/`#32768` 后用前台坐标点选；第二条条件先 invoke「添加条件」 |
| Excel「打开」按钮点不中 | UIA rect 偏移到文件名下拉区域 | 文件名框 `type_text` 完整路径后直接 Enter |
| 酷狗双击歌曲行无反应 | 行点击只选中，不跳详情 | 右键歌曲行→「查看评论(N)」进歌曲评论页 |
| 评论点赞无反馈 | 点赞按钮是文字下方统计行最左的拇指图标 | 点计数左侧约 20px 处，回读计数 +1 确认已赞 |