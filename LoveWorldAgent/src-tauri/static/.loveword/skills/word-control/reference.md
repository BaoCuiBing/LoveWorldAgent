---
name: word-control-reference
description: word-control 技能的 12 工具参数速查（与 spec §7 一致）
---

# Word 工具参数速查

所有工具除 `word_document status/quit` 外均以 `path`（完整文件路径）定位文档。索引 1 起，offset/limit 从 0 起。返回 `{ok:true,data,warnings?}` 或 `{ok:false,error:{code,message,hint,recoverable}}`。

## locator（语义定位，可省略，默认 cursor）
`by`: heading/paragraph/table/bookmark/cursor/end；`match`: 匹配文本；`matchMode`: exact/contains/startsWith/regex；`occurrence`: 第几个（1 起）；`offsetDirection`: before/after；`offsetCount`: 偏移段落数；`name`: 书签名（by=bookmark 必填）；`level`: 标题级别 1-9。

## word_document
`action`: status/create/open/save/save_as/export_pdf/close/quit/info/structure。
- status：`path?`，不创建 Word，<100ms
- create：`path`（必不存在）+ title/author/templatePath/orientation/topMargin/bottomMargin/leftMargin/rightMargin（cm）/baseStyleProfile
- open：`path` + forceNewWindow
- save：`path` → path/backupPath/bytes/savedAt
- save_as：`path` + targetPath + format(docx/doc/pdf/rtf/txt/html/mht/xml/odt/dotx/dotm/docm)
- export_pdf：`path` + targetPath? + openAfter
- close：`path` + saveChanges（默认 false）
- quit：force?（仅结束受控实例）
- info：`path` → 字数/页数/段落/字符/节
- structure：`path` + maxLevel → headings/tables

## word_read
`target`: text/paragraph/table/comments/bookmarks/lists/sections/styles/cursor/locate。
- text：`offset`(默认0)/`limit`(默认200000) → text/chars/total/truncated/nextOffset
- paragraph：`index` 或 `locator`
- table：`index`(默认1) → rows/cols/data
- locate：`locator` → found/paragraphIndex/headingContext

## word_write
`path` + (`markdown` 或 `file` 二选一) + `position`(locator，默认 cursor) + `mode`(insert/replace_selection) + `save`。返回 blocks/chars/paragraphStart/paragraphEnd/position/saved?。

## word_table（17 action）
insert(rows/columns/data/autoFitBehavior)、edit_cell(row/column/text)、edit_cells(data)、add_row、delete_row(rowIndex)、add_column、delete_column(column)、set_borders、set_shading、merge、set_column_width、set_row_height、set_cell_font、set_cell_alignment、apply_style、to_text、from_text。表格序号用 `tableIndex`（默认1），部分支持 `locator`。

## word_format（8 target）
font、paragraph、style、page_setup、properties、body_indent、track_changes、track_apply。缩进/页边距单位 cm，字号/间距单位 pt。关键属性失败→FORMAT_APPLY_FAILED，可选失败→PARTIAL_APPLY 警告。

## word_structure（10 action）
hyperlink、footnote、comment、bookmark、toc、page_region、page_numbers、watermark、columns、section_break。

## word_find（8 action）
find、replace、goto、goto_paragraph、select_all、select_text(start+length)、select_current、select_at。replace 修改文档，其余只改光标/选区。

## word_edit（5 action）
delete(unit=selection/word/paragraph/range)、backspace(count)、clipboard(op=copy/cut/paste)、undo(count)、redo(count)。

## word_media（3 action）
image(imagePath/width/height)、chart(type=column/bar/line/pie/area + data 二维)、textbox(text/width/height/orientation/positionLeft/positionTop)。尺寸单位 points。

## word_variable
`path` + variables({key:value}，1-100) + scope(document/selection) + preserveFormatting。替换 {{key}} 与 {key}。

## word_batch
`path` + ops([{action:"insert",locator,text}]，1-50)。单条失败不中断，返回 results/partial。

## word_where_am_i
`path`。返回 state/doc/cursor/selection/wordRunning/instances，只读 <100ms。

## 错误码
INVALID_ARGUMENT / PATH_DENIED / FILE_NOT_FOUND / FILE_EXISTS / NO_DOCUMENT / DOC_CHANGED / DIALOG_BLOCKED / WORD_GONE / COM_BUSY / COM_MEMBER_NOT_FOUND / WORD_ERROR / FORMAT_APPLY_FAILED / BACKUP_FAILED / CHART_DATA_FAILED / DUPLICATE_INSTANCE_UNSAVED / INTERNAL_PANIC / TIMEOUT。

## 警告码
UNSAVED_CHANGES_DISCARDED / PARTIAL_APPLY / SLOW_OPERATION / STYLE_FALLBACK。
