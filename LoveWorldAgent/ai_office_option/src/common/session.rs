use serde_json::{Map, Value}; // JSON 类型
use std::time::Duration; // 超时
use crate::common::sta::{StaCell, StaHandle}; // 通用 STA 队列

static WORD_STA: StaCell = StaCell::new(); // Word 独立 STA（与 PPT/Excel 互不阻塞）

pub fn submit(cmd: &str, args: &Map<String, Value>, timeout: Duration) -> Value { // 提交 Word 工具到共享 STA
    WORD_STA.handle("word-sta", crate::com::dispatch::dispatch_command).submit(cmd, args, timeout) // 串行执行
}
pub fn handle() -> &'static StaHandle { WORD_STA.handle("word-sta", crate::com::dispatch::dispatch_command) } // 供 shutdown/诊断使用