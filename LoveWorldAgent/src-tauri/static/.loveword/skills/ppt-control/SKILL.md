---
name: ppt-control
description: PowerPoint 演示文稿读写：10 个 ppt_* 工具覆盖生命周期、幻灯片、文本、形状、表格、格式、图表与媒体。默认启用，随设置-内置工具-PPT 开关联动
enabled: true
---





# PPT 操作（10 个 ppt_* 工具）

## 何时使用

- 需要创建、编辑、读取本地 PowerPoint 演示文稿（.pptx）或导出 PDF
- 需要批量新增幻灯片、写入标题/正文、插入表格、图表、图片与音视频

## 关键约束

1. **所有工具必须传本机盘符开头的完整绝对路径 path**（如 `E:\demo_project\a.pptx`，**禁止传相对路径**；`ppt_presentation status/quit` 除外），不使用临时 id，不支持网络 UNC 与设备路径。
2. `create` 必须传 path 且目标不存在，创建后立即保存落盘；目标已存在返回 FILE_EXISTS。
3. 复杂任务先用 `ppt_presentation status` 判断 Office 与运行态，再用 `ppt_read slides` 确认幻灯片数量。
4. 幻灯片索引从 1 开始；形状用 `shapeIndex`（从 1 开始）或 `shapeName` 定位。
5. 写入类工具在只读演示文稿上返回 READ_ONLY；无 Office 时返回 APP_NOT_INSTALLED。
6. 大文件导出/保存可能较慢，超时后先查询状态，不要盲目重复调用。

## 工具速查

- `ppt_presentation`：status/create/open/save/save_as/export_pdf/close/quit/info
- `ppt_read`：slides/shapes/text/notes/tables/charts
- `ppt_slide`：add/delete/duplicate/move/select/layout
- `ppt_write`：title/text/bullets/markdown/notes
- `ppt_shape`：add_textbox/add_picture/set_position/set_size/set_fill/set_line/delete
- `ppt_table`：insert/edit_cell/add_row/delete_row/merge
- `ppt_format`：font/paragraph/background/theme/alignment
- `ppt_chart`：insert/update_data/set_title/set_type
- `ppt_media`：image/audio/video
- `ppt_where_am_i`：当前演示、幻灯片与光标状态
