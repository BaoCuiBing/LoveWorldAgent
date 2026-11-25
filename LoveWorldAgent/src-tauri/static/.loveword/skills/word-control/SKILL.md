---
name: word-control
description: Word 文档读写：12 个 word_* 工具以完整 path 定位文档，创建/打开/保存/导出 PDF、Markdown 写入与分页读取、表格 17 种操作、字体段落格式、页眉页脚目录水印、查找替换、图片图表文本框、变量与批量。默认启用，随设置-内置工具-Word 开关联动
enabled: true
---





# Word 操作（12 个 word_* 工具）

## 何时使用

- 需要创建、编辑、读取本地 Word 文档（.docx/.doc/.rtf 等）
- 需要把 Markdown 内容写入 Word、生成表格、设置格式或导出 PDF

## 关键约束

1. **所有工具必须传本机盘符开头的完整绝对路径 path**（如 `E:\demo_project\a.docx`，**禁止传相对路径**；`word_document status/quit` 除外），不使用 docId 或临时 id，不支持网络 UNC 与设备路径。
2. `create` 必须传 path 且目标不存在，创建后立即保存落盘；目标已存在返回 FILE_EXISTS。
3. 复杂任务先用 `word_document status` 与 `word_where_am_i` 确认状态与光标，再开始读写。
4. 写入长文分 3~8 次 `word_write` 追加，单次 ≤100k 字符；写入前确认 path 指向目标文档，返回 DOC_CHANGED 时先重新打开。
5. `word_read target=text` 默认读 200k 字符，用 offset/limit 分页，依据 total/nextOffset 判断是否继续。
6. 保存用 `word_document save`（保存前自动生成 .bak 备份并保留 5 份）；关闭用 `close`，未保存修改会给出 UNSAVED_CHANGES_DISCARDED 警告。
7. 写/格式/关闭类操作按权限逐项审批；Word 组关闭=硬拦截，组开启但条目关闭=需用户批准。


## 工具清单

| 工具 | 用途 |
| --- | --- |
| `word_document` | status/create/open/save/save_as/export_pdf/close/quit/info/structure |
| `word_read` | text/paragraph/table/comments/bookmarks/lists/sections/styles/cursor/locate |
| `word_write` | Markdown 或文件写入，支持 position 定位与追加 |
| `word_table` | 表格 17 种操作（insert/edit_cell/add_row/merge/borders 等） |
| `word_format` | font/paragraph/style/page_setup/properties/body_indent/track_changes/track_apply |
| `word_structure` | hyperlink/footnote/comment/bookmark/toc/page_region/page_numbers/watermark/columns/section_break |
| `word_find` | find/replace/goto/goto_paragraph/select_all/select_text/select_current/select_at |
| `word_edit` | delete/backspace/clipboard/undo/redo |
| `word_media` | image/chart/textbox |
| `word_variable` | 替换 {{key}} 与 {key} 变量占位符 |
| `word_batch` | 批量插入（1~50 条，单条失败不中断） |
| `word_where_am_i` | 只读查询状态、文档与光标（<100ms） |

各 action/target 的参数详见 `reference.md`。
