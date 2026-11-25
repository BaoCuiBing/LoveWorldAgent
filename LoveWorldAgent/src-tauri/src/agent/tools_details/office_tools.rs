use std::path::Path; // 本地绝对路径校验
use serde_json::{json, Value}; // JSON 工具：32 个 word_/ppt_/excel_ 统一走 utils/office_option.rs 命名管道
use crate::agent::tools::{blocking, params, ToolDef}; // 工具定义与同步包装
use crate::agent::tools_details::common::remember_tool_meta; // 旁路元数据（标示失败态）
use crate::utils::{office_option, sidecar_pipe}; // 管道客户端与截图剥离助手

const OFFICE_PATH_KEYS: [&str; 7] = ["path", "targetPath", "templatePath", "file", "imagePath", "themePath", "mediaPath"]; // Office 工具中所有接收文件路径的参数名
fn is_valid_local_absolute_path(raw: &str) -> bool { // 仅允许本机盘符开头的绝对路径，禁止相对路径、UNC 与设备路径
    let trimmed = raw.trim();
    if trimmed.is_empty() || trimmed.starts_with(r"\\") { return false; }
    Path::new(trimmed).is_absolute()
}
fn validate_office_paths(value: &Value) -> Option<String> { // 拦截相对路径并附带当前项目根提示便于模型立即纠偏
    let root_hint = crate::config::project_path()
        .map(|p| format!("；当前项目根目录为 {}", crate::config::strip_verbatim(p).to_string_lossy()))
        .unwrap_or_default();
    for key in OFFICE_PATH_KEYS {
        let Some(raw) = value.get(key).and_then(Value::as_str) else { continue; };
        let trimmed = raw.trim();
        if trimmed.is_empty() { continue; }
        if !is_valid_local_absolute_path(trimmed) {
            return Some(format!("参数 {key} 必须传本机盘符开头的完整绝对路径（禁止相对路径或 UNC/设备路径），收到：{trimmed}{root_hint}"));
        }
    }
    None
}
fn annotate_path_schemas(schema: &mut Value) { // 统一为 32 个工具 schema 中的路径字段补充绝对路径约束说明
    let Some(props) = schema.get_mut("properties").and_then(Value::as_object_mut) else { return; };
    for key in OFFICE_PATH_KEYS {
        if let Some(prop) = props.get_mut(key).and_then(Value::as_object_mut) {
            prop.entry("description".to_string()).or_insert_with(|| json!("必须传本机盘符开头的绝对路径（如 E:\\dir\\file），禁止传相对路径"));
        }
    }
}

fn locator_schema() -> Value { // locator 参数 schema（Word 语义定位）
    json!({"type": "object", "properties": {
        "by": {"type": "string", "enum": ["heading", "paragraph", "table", "bookmark", "cursor", "end"], "description": "定位类型（默认 cursor）"},
        "match": {"type": "string", "description": "匹配文本（heading/paragraph）"},
        "matchMode": {"type": "string", "enum": ["exact", "contains", "startsWith", "regex"], "description": "匹配方式（默认 exact）"},
        "occurrence": {"type": "integer", "description": "第几个匹配（1 起，默认 1）"},
        "offsetDirection": {"type": "string", "enum": ["before", "after"], "description": "偏移方向"},
        "offsetCount": {"type": "integer", "description": "偏移段落数"},
        "name": {"type": "string", "description": "书签名称（by=bookmark 必填）"},
        "level": {"type": "integer", "description": "标题级别 1-9"}
    }, "additionalProperties": false})
}
fn word_defs() -> Vec<(&'static str, &'static str, Value, f64)> { // 12 个 Word 工具
    vec![
        ("word_document", "Word 文档生命周期：status/create/open/save/save_as/export_pdf/close/quit/info/structure", json!({"type":"object","properties":{"action":{"type":"string","enum":["status","create","open","save","save_as","export_pdf","close","quit","info","structure"]},"path":{"type":"string"},"targetPath":{"type":"string"},"format":{"type":"string"},"saveChanges":{"type":"boolean"},"title":{"type":"string"},"author":{"type":"string"},"templatePath":{"type":"string"},"orientation":{"type":"string","enum":["portrait","landscape"]},"topMargin":{"type":"number"},"bottomMargin":{"type":"number"},"leftMargin":{"type":"number"},"rightMargin":{"type":"number"},"baseStyleProfile":{"oneOf":[{"type":"string"},{"type":"object"}]},"forceNewWindow":{"type":"boolean"},"openAfter":{"type":"boolean"},"force":{"type":"boolean"},"maxLevel":{"type":"integer"}},"required":["action"],"additionalProperties":false}), 180.0),
        ("word_read", "读取 Word 内容：text/paragraph/table/comments/bookmarks/lists/sections/styles/cursor/locate", json!({"type":"object","properties":{"path":{"type":"string"},"target":{"type":"string","enum":["text","paragraph","table","comments","bookmarks","lists","sections","styles","cursor","locate"]},"offset":{"type":"integer"},"limit":{"type":"integer"},"index":{"type":"integer"},"locator":locator_schema()},"required":["path"],"additionalProperties":false}), 15.0),
        ("word_write", "向 Word 写入 Markdown 或插入文件内容，支持定位与追加", json!({"type":"object","properties":{"path":{"type":"string"},"markdown":{"type":"string"},"file":{"type":"string"},"position":locator_schema(),"mode":{"type":"string","enum":["insert","replace_selection"]},"save":{"type":"boolean"}},"required":["path"],"additionalProperties":false}), 60.0),
        ("word_table", "Word 表格 17 种操作：insert/edit_cell/edit_cells/add_row/delete_row 等", json!({"type":"object","properties":{"path":{"type":"string"},"action":{"type":"string"},"tableIndex":{"type":"integer"},"rows":{"type":"integer"},"columns":{"type":"integer"},"row":{"type":"integer"},"column":{"type":"integer"},"rowIndex":{"type":"integer"},"text":{"type":"string"},"data":{"type":"array"},"width":{"type":"number"},"height":{"type":"number"},"color":{"type":"string"},"styleName":{"type":"string"},"separator":{"type":"string"},"autoFitBehavior":{"type":"string"},"inside":{"type":"object"},"outside":{"type":"object"},"target":{"type":"string"},"alignment":{"type":"string"},"locator":locator_schema()},"required":["path","action"],"additionalProperties":false}), 60.0),
        ("word_format", "Word 格式：font/paragraph/style/page_setup/properties/body_indent/track_changes/track_apply", json!({"type":"object","properties":{"path":{"type":"string"},"target":{"type":"string","enum":["font","paragraph","style","page_setup","properties","body_indent","track_changes","track_apply"]},"name":{"type":"string"},"size":{"type":"number"},"bold":{"type":"boolean"},"italic":{"type":"boolean"},"underline":{"type":"string"},"color":{"type":"string"},"strikethrough":{"type":"boolean"},"alignment":{"type":"string"},"leftIndent":{"type":"number"},"rightIndent":{"type":"number"},"firstLineIndent":{"type":"number"},"spaceBefore":{"type":"number"},"spaceAfter":{"type":"number"},"lineSpacing":{"type":"number"},"lineSpacingRule":{"type":"string"},"styleName":{"type":"string"},"orientation":{"type":"string"},"indent":{"type":"number"},"enable":{"type":"boolean"},"action":{"type":"string"},"locator":locator_schema()},"required":["path","target"],"additionalProperties":false}), 60.0),
        ("word_structure", "Word 结构：hyperlink/footnote/comment/bookmark/toc/page_region/page_numbers/watermark/columns/section_break", json!({"type":"object","properties":{"path":{"type":"string"},"action":{"type":"string"},"text":{"type":"string"},"address":{"type":"string"},"name":{"type":"string"},"levels":{"type":"integer"},"target":{"type":"string"},"sectionIndex":{"type":"integer"},"color":{"type":"string"},"fontSize":{"type":"integer"},"count":{"type":"integer"},"spacing":{"type":"number"},"remove":{"type":"boolean"},"locator":locator_schema()},"required":["path","action"],"additionalProperties":false}), 60.0),
        ("word_find", "Word 查找与选区：find/replace/goto/goto_paragraph/select_all/select_text/select_current/select_at", json!({"type":"object","properties":{"path":{"type":"string"},"action":{"type":"string"},"findText":{"type":"string"},"replaceWith":{"type":"string"},"matchCase":{"type":"boolean"},"matchWholeWord":{"type":"boolean"},"direction":{"type":"string"},"wrap":{"type":"boolean"},"replaceAll":{"type":"boolean"},"what":{"type":"string"},"which":{"type":"string"},"name":{"type":"string"},"index":{"type":"integer"},"start":{"type":"integer"},"length":{"type":"integer"},"scope":{"type":"string"},"locator":locator_schema()},"required":["path","action"],"additionalProperties":false}), 15.0),
        ("word_edit", "Word 编辑：delete/backspace/clipboard/undo/redo", json!({"type":"object","properties":{"path":{"type":"string"},"action":{"type":"string"},"unit":{"type":"string"},"range":{"type":"object"},"count":{"type":"integer"},"op":{"type":"string"}},"required":["path","action"],"additionalProperties":false}), 15.0),
        ("word_media", "Word 媒体：image/chart/textbox", json!({"type":"object","properties":{"path":{"type":"string"},"action":{"type":"string"},"imagePath":{"type":"string"},"type":{"type":"string"},"data":{"type":"array"},"title":{"type":"string"},"width":{"type":"number"},"height":{"type":"number"},"text":{"type":"string"},"orientation":{"type":"string"},"positionLeft":{"type":"number"},"positionTop":{"type":"number"},"locator":locator_schema()},"required":["path","action"],"additionalProperties":false}), 60.0),
        ("word_variable", "替换 Word 文档变量占位符 {{key}} 与 {key}", json!({"type":"object","properties":{"path":{"type":"string"},"variables":{"type":"object"},"scope":{"type":"string","enum":["document","selection"]},"preserveFormatting":{"type":"boolean"}},"required":["path","variables"],"additionalProperties":false}), 60.0),
        ("word_batch", "批量执行 Word 插入操作（1~50 条，单条失败不中断）", json!({"type":"object","properties":{"path":{"type":"string"},"ops":{"type":"array","items":{"type":"object","properties":{"action":{"type":"string"},"locator":locator_schema(),"text":{"type":"string"}},"required":["action","text"]}}},"required":["path","ops"],"additionalProperties":false}), 60.0),
        ("word_where_am_i", "只读查询 Word 当前状态、文档与光标位置（<100ms，不创建 Word）", json!({"type":"object","properties":{"path":{"type":"string"}},"required":["path"],"additionalProperties":false}), 15.0),
    ]
}
fn ppt_defs() -> Vec<(&'static str, &'static str, Value, f64)> { // 10 个 PPT 工具
    vec![
        ("ppt_presentation", "PowerPoint 生命周期：status/create/open/save/save_as/export_pdf/close/quit/info", json!({"type":"object","properties":{"action":{"type":"string","enum":["status","create","open","save","save_as","export_pdf","close","quit","info"]},"path":{"type":"string"},"targetPath":{"type":"string"},"saveChanges":{"type":"boolean"},"force":{"type":"boolean"}},"required":["action"],"additionalProperties":false}), 180.0),
        ("ppt_read", "读取演示文稿：slides/shapes/text/notes/tables/charts", json!({"type":"object","properties":{"path":{"type":"string"},"target":{"type":"string","enum":["slides","shapes","text","notes","tables","charts"]},"slideIndex":{"type":"integer"}},"required":["path","target"],"additionalProperties":false}), 15.0),
        ("ppt_slide", "幻灯片操作：add/delete/duplicate/move/select/layout", json!({"type":"object","properties":{"path":{"type":"string"},"action":{"type":"string","enum":["add","delete","duplicate","move","select","layout"]},"slideIndex":{"type":"integer"},"layout":{"type":"integer"},"toIndex":{"type":"integer"}},"required":["path","action"],"additionalProperties":false}), 60.0),
        ("ppt_write", "写入演示文稿：title/text/bullets/markdown/notes", json!({"type":"object","properties":{"path":{"type":"string"},"action":{"type":"string","enum":["title","text","bullets","markdown","notes"]},"slideIndex":{"type":"integer"},"text":{"type":"string"},"markdown":{"type":"string"},"items":{"type":"array","items":{"type":"string"}}},"required":["path","action"],"additionalProperties":false}), 60.0),
        ("ppt_shape", "形状操作：add_textbox/add_picture/set_position/set_size/set_fill/set_line/delete", json!({"type":"object","properties":{"path":{"type":"string"},"action":{"type":"string","enum":["add_textbox","add_picture","set_position","set_size","set_fill","set_line","delete"]},"slideIndex":{"type":"integer"},"shapeIndex":{"type":"integer"},"shapeName":{"type":"string"},"text":{"type":"string"},"imagePath":{"type":"string"},"left":{"type":"number"},"top":{"type":"number"},"width":{"type":"number"},"height":{"type":"number"},"color":{"type":"string"},"visible":{"type":"boolean"}},"required":["path","action"],"additionalProperties":false}), 60.0),
        ("ppt_table", "表格操作：insert/edit_cell/add_row/delete_row/merge", json!({"type":"object","properties":{"path":{"type":"string"},"action":{"type":"string","enum":["insert","edit_cell","add_row","delete_row","merge"]},"slideIndex":{"type":"integer"},"shapeIndex":{"type":"integer"},"rows":{"type":"integer"},"columns":{"type":"integer"},"row":{"type":"integer"},"column":{"type":"integer"},"rowIndex":{"type":"integer"},"text":{"type":"string"},"data":{"type":"array"},"rowStart":{"type":"integer"},"colStart":{"type":"integer"},"rowEnd":{"type":"integer"},"colEnd":{"type":"integer"}},"required":["path","action"],"additionalProperties":false}), 60.0),
        ("ppt_format", "格式操作：font/paragraph/background/theme/alignment", json!({"type":"object","properties":{"path":{"type":"string"},"target":{"type":"string","enum":["font","paragraph","background","theme","alignment"]},"slideIndex":{"type":"integer"},"shapeIndex":{"type":"integer"},"shapeName":{"type":"string"},"name":{"type":"string"},"size":{"type":"number"},"bold":{"type":"boolean"},"italic":{"type":"boolean"},"underline":{"type":"boolean"},"color":{"type":"string"},"alignment":{"type":"string"},"spaceBefore":{"type":"number"},"spaceAfter":{"type":"number"},"lineSpacing":{"type":"number"},"themePath":{"type":"string"}},"required":["path","target"],"additionalProperties":false}), 60.0),
        ("ppt_chart", "图表操作：insert/update_data/set_title/set_type", json!({"type":"object","properties":{"path":{"type":"string"},"action":{"type":"string","enum":["insert","update_data","set_title","set_type"]},"slideIndex":{"type":"integer"},"shapeIndex":{"type":"integer"},"type":{"type":"string"},"data":{"type":"array"},"title":{"type":"string"},"left":{"type":"number"},"top":{"type":"number"},"width":{"type":"number"},"height":{"type":"number"}},"required":["path","action"],"additionalProperties":false}), 90.0),
        ("ppt_media", "媒体操作：image/audio/video", json!({"type":"object","properties":{"path":{"type":"string"},"action":{"type":"string","enum":["image","audio","video"]},"slideIndex":{"type":"integer"},"imagePath":{"type":"string"},"mediaPath":{"type":"string"},"left":{"type":"number"},"top":{"type":"number"},"width":{"type":"number"},"height":{"type":"number"}},"required":["path","action"],"additionalProperties":false}), 60.0),
        ("ppt_where_am_i", "只读查询当前演示文稿、幻灯片与形状状态", json!({"type":"object","properties":{"path":{"type":"string"}},"required":[],"additionalProperties":false}), 15.0),
    ]
}
fn excel_defs() -> Vec<(&'static str, &'static str, Value, f64)> { // 10 个 Excel 工具
    vec![
        ("excel_workbook", "Excel 生命周期：status/create/open/save/save_as/export_pdf/close/quit/info", json!({"type":"object","properties":{"action":{"type":"string","enum":["status","create","open","save","save_as","export_pdf","close","quit","info"]},"path":{"type":"string"},"targetPath":{"type":"string"},"saveChanges":{"type":"boolean"},"force":{"type":"boolean"}},"required":["action"],"additionalProperties":false}), 180.0),
        ("excel_read", "读取工作簿：range/sheet/used_range/cells/table", json!({"type":"object","properties":{"path":{"type":"string"},"target":{"type":"string","enum":["range","sheet","used_range","cells","table"]},"sheet":{"type":"string"},"range":{"type":"string"},"address":{"type":"string"},"row":{"type":"integer"},"column":{"type":"integer"}},"required":["path","target"],"additionalProperties":false}), 15.0),
        ("excel_write", "写入工作簿：range/cells/markdown/formula", json!({"type":"object","properties":{"path":{"type":"string"},"action":{"type":"string","enum":["range","cells","markdown","formula"]},"sheet":{"type":"string"},"range":{"type":"string"},"row":{"type":"integer"},"column":{"type":"integer"},"data":{"type":"array"},"value":{},"markdown":{"type":"string"},"formula":{"type":"string"}},"required":["path","action"],"additionalProperties":false}), 60.0),
        ("excel_sheet", "工作表操作：add/delete/rename/copy/move/select", json!({"type":"object","properties":{"path":{"type":"string"},"action":{"type":"string","enum":["add","delete","rename","copy","move","select"]},"sheet":{"type":"string"},"name":{"type":"string"},"after":{"type":"integer"},"before":{"type":"integer"}},"required":["path","action"],"additionalProperties":false}), 60.0),
        ("excel_format", "格式操作：font/fill/border/number_format/column_width/row_height/merge", json!({"type":"object","properties":{"path":{"type":"string"},"target":{"type":"string","enum":["font","fill","border","number_format","column_width","row_height","merge"]},"sheet":{"type":"string"},"range":{"type":"string"},"name":{"type":"string"},"size":{"type":"number"},"bold":{"type":"boolean"},"italic":{"type":"boolean"},"color":{"type":"string"},"numberFormat":{"type":"string"},"width":{"type":"number"},"height":{"type":"number"},"borderIndex":{"type":"integer"},"lineStyle":{"type":"integer"},"weight":{"type":"integer"}},"required":["path","target"],"additionalProperties":false}), 60.0),
        ("excel_formula", "公式操作：set/get/recalculate", json!({"type":"object","properties":{"path":{"type":"string"},"action":{"type":"string","enum":["set","get","recalculate"]},"sheet":{"type":"string"},"range":{"type":"string"},"cell":{"type":"string"},"row":{"type":"integer"},"column":{"type":"integer"},"formula":{"type":"string"}},"required":["path","action"],"additionalProperties":false}), 60.0),
        ("excel_table", "表格操作：create/delete/add_row/delete_row/filter", json!({"type":"object","properties":{"path":{"type":"string"},"action":{"type":"string","enum":["create","delete","add_row","delete_row","filter"]},"sheet":{"type":"string"},"name":{"type":"string"},"tableIndex":{"type":"integer"},"range":{"type":"string"},"rowIndex":{"type":"integer"},"field":{"type":"integer"},"criteria":{"type":"string"},"data":{"type":"array"}},"required":["path","action"],"additionalProperties":false}), 60.0),
        ("excel_chart", "图表操作：insert/update_data/set_title/set_type", json!({"type":"object","properties":{"path":{"type":"string"},"action":{"type":"string","enum":["insert","update_data","set_title","set_type"]},"sheet":{"type":"string"},"chartIndex":{"type":"integer"},"chartName":{"type":"string"},"range":{"type":"string"},"type":{"type":"string"},"title":{"type":"string"},"left":{"type":"number"},"top":{"type":"number"},"width":{"type":"number"},"height":{"type":"number"}},"required":["path","action"],"additionalProperties":false}), 90.0),
        ("excel_sort_filter", "排序筛选：sort/filter/autofilter/clear", json!({"type":"object","properties":{"path":{"type":"string"},"action":{"type":"string","enum":["sort","filter","autofilter","clear"]},"sheet":{"type":"string"},"range":{"type":"string"},"key":{"type":"string"},"order":{"type":"string"},"field":{"type":"integer"},"criteria":{"type":"string"}},"required":["path","action"],"additionalProperties":false}), 60.0),
        ("excel_where_am_i", "只读查询当前工作簿、工作表与选区状态", json!({"type":"object","properties":{"path":{"type":"string"}},"required":[],"additionalProperties":false}), 15.0),
    ]
}
pub fn build_office_tools() -> Vec<ToolDef> { // 构造 32 个工具，按前缀归入 word/ppt/excel 审批组
    let _ = params; // 兼容导入（本模块用内联 schema）
    let mut tools = Vec::new(); // 结果
    for (group, defs) in [("word", word_defs()), ("ppt", ppt_defs()), ("excel", excel_defs())] { // 逐族
        for (name, description, mut parameters, timeout) in defs { // 逐工具
            annotate_path_schemas(&mut parameters);
            let tool_name = name.to_string(); // 克隆工具名
            tools.push(ToolDef::new(name, description, parameters, blocking(move |args: String| { // 同步处理器
                let value: Value = serde_json::from_str(&args).unwrap_or_else(|_| json!({})); // 解析参数
                if let Some(msg) = validate_office_paths(&value) {
                    remember_tool_meta(&tool_name, json!({"ok": false, "error": msg}));
                    return json!({"ok": false, "error": {"code": "INVALID_PATH", "message": msg}}).to_string();
                }
                let result = office_option::call(&tool_name, &value, timeout); // 调用 sidecar
                if result.get("ok").and_then(Value::as_bool).unwrap_or(false) { // 成功
                    sidecar_pipe::strip_screenshot_fields(result.get("data").cloned().unwrap_or(Value::Null)).to_string() // 剥离任何截图字段再返回模型
                } else { // 失败
                    let error = result.get("error").cloned().unwrap_or_else(|| json!("Office 工具调用失败")); // 错误
                    remember_tool_meta(&tool_name, json!({"ok": false, "error": error}));
                    json!({"ok": false, "error": error}).to_string() // 返回错误
                }
            })).with_group(group)) // 归组
        }
    }
    tools // 返回
}
pub fn build_word_tools() -> Vec<ToolDef> { build_office_tools() } // 兼容旧调用
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exposes_thirty_two_tools_and_strips_screenshots_from_llm_output() {
        assert_eq!(build_office_tools().len(), 32);
        let dirty = json!({"ok": true, "screenshot": "C:\\screen\\w.png", "nested": {"image_base64": "AAA", "saved": true}});
        let clean = sidecar_pipe::strip_screenshot_fields(dirty);
        assert!(clean.get("screenshot").is_none());
        assert!(clean["nested"].get("image_base64").is_none());
        assert_eq!(clean["nested"]["saved"], json!(true));
    }
    #[test]
    fn rejects_relative_and_unc_office_paths() {
        assert!(validate_office_paths(&json!({"path": "office_titles/a.xlsx"})).is_some());
        assert!(validate_office_paths(&json!({"path": "/tmp/a.xlsx"})).is_some());
        assert!(validate_office_paths(&json!({"path": r"\\server\share\a.docx"})).is_some());
        assert!(validate_office_paths(&json!({"targetPath": "out.pdf"})).is_some());
        assert!(validate_office_paths(&json!({"path": r"E:\demo_project\a.xlsx"})).is_none());
    }
}