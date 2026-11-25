---
name: excel-control
description: Excel 工作簿读写：10 个 excel_* 工具覆盖生命周期、区域/单元格、工作表、格式、公式、表格、图表、排序筛选。默认启用，随设置-内置工具-Excel 开关联动
enabled: true
---





# Excel 操作（10 个 excel_* 工具）

## 何时使用

- 需要创建、编辑、读取本地 Excel 工作簿（.xlsx）或导出 PDF
- 需要批量写入区域、设置格式、写入公式、创建表格/图表、排序筛选

## 关键约束

1. **所有工具必须传本机盘符开头的完整绝对路径 path**（如 `E:\demo_project\a.xlsx`，**禁止传相对路径**；`excel_workbook status/quit` 除外），不使用临时 id，不支持网络 UNC 与设备路径。
2. `create` 必须传 path 且目标不存在，创建后立即保存落盘；目标已存在返回 FILE_EXISTS。
3. 复杂任务先用 `excel_workbook status` 判断 Office 与运行态，再用 `excel_read used_range` 确认数据范围。
4. 工作表用 `sheet` 名称定位（缺省活动工作表）；区域用 A1 地址（如 `A1:C10`）。
5. 写入类工具在只读工作簿上返回 READ_ONLY；无 Office 时返回 APP_NOT_INSTALLED。
6. 区域越大逐格读写越慢，建议单次不超过 1000 格。

## 工具速查

- `excel_workbook`：status/create/open/save/save_as/export_pdf/close/quit/info
- `excel_read`：range/sheet/used_range/cells/table
- `excel_write`：range/cells/markdown/formula
- `excel_sheet`：add/delete/rename/copy/move/select
- `excel_format`：font/fill/border/number_format/column_width/row_height/merge
- `excel_formula`：set/get/recalculate
- `excel_table`：create/delete/add_row/delete_row/filter
- `excel_chart`：insert/update_data/set_title/set_type
- `excel_sort_filter`：sort/filter/autofilter/clear
- `excel_where_am_i`：当前工作簿、工作表与选区状态
