use super::parser::OmniParser; // 解析器
use anyhow::Result; // 错误类型
use base64::Engine; // base64 解码
use crate::log_info; // 日志宏
use serde::{Deserialize, Serialize}; // 序列化派生
use serde_json::Value; // JSON 值
use std::io::{BufRead, Write}; // 行读写
use std::path::PathBuf; // 路径
#[derive(Deserialize)] // 解析请求
struct ParseRequest { // 请求结构
    id: u64, // 请求 id
    image: String, // base64 或本地路径
    #[serde(default)] // 可缺省
    bbox_threshold: Option<f32>, // 预留
    #[serde(default)] // 可缺省
    iou_threshold: Option<f32>, // 预留
}
#[derive(Serialize)] // 解析响应
struct ParseResponse { // 响应结构
    id: u64, // 请求 id
    #[serde(skip_serializing_if = "Option::is_none")] // 空值不输出
    bboxes: Option<Value>, // 元素列表
    #[serde(skip_serializing_if = "Option::is_none")] // 空值不输出
    error: Option<String>, // 错误信息
    ms: u128, // 耗时毫秒
}
pub fn run_stdio_server(models_dir: PathBuf) -> Result<()> { // stdio JSON 服务循环
    let mut parser = OmniParser::new(&models_dir, Default::default())?; // 构建解析器
    log_info!("omni", "omni serve 就绪，models={}", models_dir.display()); // 就绪记录
    let stdin = std::io::stdin(); // 标准输入
    let mut line = String::new(); // 行缓冲
    loop { // 逐行处理
        line.clear(); // 清空缓冲
        let n = stdin.lock().read_line(&mut line)?; // 读一行
        if n == 0 { break; } // EOF 退出
        let trimmed = line.trim(); // 去空白
        if trimmed.is_empty() { continue; } // 空行跳过
        let resp = handle(&mut parser, trimmed); // 处理请求
        let out = serde_json::to_string(&resp)?; // 序列化
        println!("{out}"); // 输出一行
        std::io::stdout().lock().flush()?; // 立即刷新
    }
    Ok(()) // 正常结束
}
fn handle(parser: &mut OmniParser, raw: &str) -> ParseResponse { // 单请求处理
    let req: ParseRequest = match serde_json::from_str(raw) { Ok(r) => r, Err(e) => { return ParseResponse { id: 0, bboxes: None, error: Some(format!("bad request: {e}")), ms: 0 }; } }; // 解析失败返回错误响应
    let start = std::time::Instant::now(); // 计时
    match parse_inner(parser, &req) { // 执行解析
        Ok(bboxes) => ParseResponse { id: req.id, bboxes: Some(bboxes), error: None, ms: start.elapsed().as_millis() }, // 成功响应
        Err(e) => ParseResponse { id: req.id, bboxes: None, error: Some(format!("{e:#}")), ms: start.elapsed().as_millis() }, // 失败响应
    }
}
fn parse_inner(parser: &mut OmniParser, req: &ParseRequest) -> Result<Value> { // 解析执行
    let bytes = match base64::engine::general_purpose::STANDARD.decode(&req.image) { // 先按 base64
        Ok(b) => b, // base64 输入
        Err(_) => std::fs::read(PathBuf::from(&req.image))?, // 回退本地路径
    };
    let img = image::load_from_memory(&bytes)?; // 解码图片
    let _ = req.bbox_threshold; // 预留
    let _ = req.iou_threshold; // 预留
    let elements = parser.parse(&img)?; // 解析
    Ok(serde_json::json!(elements)) // 返回元素
}