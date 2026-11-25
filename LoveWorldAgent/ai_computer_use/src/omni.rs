//! omni_parser 模块桥接：源码位于 ../omni_parser/core/，模型位于 omni_parser/models/
use anyhow::{bail, Result}; // 错误类型
use clap::Subcommand; // 子命令派生
use crate::log_info; // 日志宏
use serde::{Deserialize, Serialize}; // 序列化结构
use std::path::{Path, PathBuf}; // 路径处理
use std::sync::{Mutex, OnceLock}; // 常驻视觉解析器
#[path = "../omni_parser/core/lib.rs"] // 桥接 core 源码目录
pub mod omni_parser; // omni_parser 模块（OmniParser/run_stdio_server）
#[derive(Debug, Clone, Serialize, Deserialize)] // 单个视觉解析元素（同时符合 omni_parser 契约与窗口/屏幕像素坐标）
pub struct ParsedVisualCenter { pub x: i32, pub y: i32 } // 中心点结构体
#[derive(Debug, Clone, Serialize, Deserialize)] // 单个视觉解析元素
pub struct ParsedVisualElement { // 视觉元素结构
    pub index: usize, // V0..Vn 序号
    #[serde(rename = "type")] // 序列化字段名为 type（text / icon）
    pub kind: String, // text / icon
    pub content: String, // OCR 文本或图标描述（无 captioner 时为空串但仍保留 icon 框）
    pub interactivity: bool, // 是否可交互
    pub center: ParsedVisualCenter, // 局部中心点 {x, y}
    pub x: i32, // 图像/窗口局部中心 X（像素）
    pub y: i32, // 图像/窗口局部中心 Y（像素）
    pub screen_x: i32, // 屏幕绝对中心 X（像素）
    pub screen_y: i32, // 屏幕绝对中心 Y（像素）
    pub bbox: [i32; 4], // 图像/窗口局部像素框 [x1, y1, x2, y2]
    pub bbox_screen: [i32; 4], // 屏幕绝对像素框 [x1, y1, x2, y2]
}
#[derive(Debug, Clone, Serialize)] // 视觉解析汇总
pub struct VisionParseOutput { // 视觉解析返回结构
    pub elements: Vec<ParsedVisualElement>, // 完整元素列表（含无文本图标框）
    pub text_count: usize, // OCR 文本数
    pub icon_count: usize, // 检测到图标框数量
    pub ms: u128, // 推理耗时（毫秒）
    pub summary: String, // 供模型直接阅读的紧凑文本清单
}
#[derive(Subcommand)] // omni 子命令集
pub enum OmniCommand { // omni 的二级子命令
    Serve { // stdio JSON 服务（每行一个请求）
        #[arg(long)] // 模型目录
        models_dir: Option<PathBuf>, // 缺省自动探测
    },
    Parse { // 单图解析
        #[arg(long)] // 输入图片
        image: PathBuf, // png/jpeg
        #[arg(long)] // 模型目录
        models_dir: Option<PathBuf>, // 缺省自动探测
        #[arg(long, default_value = "v2")] // 检测模型版本
        model: String, // v2 / v3 / 自定义文件名
    },
}
fn find_upward(start: &Path, rel: &str) -> Option<PathBuf> { // 向上最多 8 级探测相对路径
    let mut dir = Some(start.to_path_buf()); // 从起点逐级向上
    for _ in 0..8 { // 上限 8 级
        let d = dir?; // 当前目录
        let candidate = d.join(rel); // 候选路径
        if candidate.exists() { return Some(candidate); } // 命中返回
        dir = d.parent().map(|p| p.to_path_buf()); // 上一级
    }
    None // 未命中
}
pub fn resolve_models_dir(custom: Option<PathBuf>) -> Result<PathBuf> { // 解析模型目录（兼容随包态与开发态）
    if let Some(d) = custom { return Ok(if d.is_absolute() { d } else { std::env::current_dir()?.join(d) }); } // 显式指定优先
    if let Ok(exe) = std::env::current_exe() { // exe 旁 models（随包态）
        if let Some(parent) = exe.parent() { // exe 所在目录
            let p = parent.join("models"); // 随包模型目录
            if p.join("ms_vision").join("icon_detect.onnx").exists() { return Ok(p); } // 命中
        }
    }
    let mut starts: Vec<PathBuf> = Vec::new(); // 向上探测起点集合
    if let Ok(cwd) = std::env::current_dir() { starts.push(cwd); } // 开发态 cwd
    if let Ok(exe) = std::env::current_exe() { if let Some(p) = exe.parent() { starts.push(p.to_path_buf()); } } // exe 目录
    for s in &starts { // 逐起点探测
        for rel in ["omni_parser/models/ms_vision/icon_detect.onnx", "models/ms_vision/icon_detect.onnx", "static/computer_use/models/ms_vision/icon_detect.onnx"] { // 源码布局 + 随包布局 + static 回退
            if let Some(hit) = find_upward(s, rel) { return Ok(hit.parent().and_then(|p| p.parent()).expect("必有父目录").to_path_buf()); } // 返回 models 根目录
        }
    }
    bail!("未找到 models/ms_vision/icon_detect.onnx，可用 --models-dir 显式指定") // 探测失败
}
fn cuda_dir() -> Option<PathBuf> { // 定位 cuda_runtime 运行库目录
    if let Ok(exe) = std::env::current_exe() { if let Some(dir) = exe.parent() { let p = dir.join("cuda_runtime"); if p.join("onnxruntime.dll").is_file() { return Some(p); } } } // 随包态：exe 旁
    let mut cur = std::env::current_exe().ok().and_then(|e| e.parent().map(|p| p.to_path_buf())); // 开发态：exe 向上
    while let Some(dir) = cur { // 逐级探测
        for rel in ["omni_parser/cuda_runtime", "static/computer_use/cuda_runtime"] { // 源码布局与 static 布局
            let p = dir.join(rel); // 拼路径
            if p.join("onnxruntime.dll").is_file() { return Some(p); } // 命中
        }
        cur = dir.parent().map(|p| p.to_path_buf()); // 上一级
    }
    if let Ok(cwd) = std::env::current_dir() { // cwd 向上探测兜底
        if let Some(hit) = find_upward(&cwd, "omni_parser/cuda_runtime/onnxruntime.dll").or_else(|| find_upward(&cwd, "static/computer_use/cuda_runtime/onnxruntime.dll")) {
            return hit.parent().map(|p| p.to_path_buf()); // 返回目录
        }
    }
    None // 未命中
}
fn ort_dll_path() -> PathBuf { // 解析 onnxruntime.dll
    if let Ok(p) = std::env::var("ORT_DYLIB_PATH") { let path = PathBuf::from(p); if path.is_file() { return path; } } // 环境变量优先
    if let Some(dir) = cuda_dir() { return dir.join("onnxruntime.dll"); } // cuda_runtime 子目录
    if let Ok(exe) = std::env::current_exe() { if let Some(dir) = exe.parent() { let p = dir.join("onnxruntime.dll"); if p.is_file() { return p; } } } // exe 旁兼容
    PathBuf::from("onnxruntime.dll") // 回退当前目录
}
fn init_ort() -> Result<()> { // 初始化 ORT 动态库（load-dynamic）
    if let Some(dir) = cuda_dir() { // 运行库子目录
        let wide = windows::core::HSTRING::from(dir.to_string_lossy().as_ref()); // 宽字符路径
        let _ = unsafe { windows::Win32::System::LibraryLoader::SetDllDirectoryW(windows::core::PCWSTR(wide.as_ptr())) }; // 加入 DLL 搜索
    }
    let dll = ort_dll_path(); // 解析路径
    let committed = ort::init_from(&dll).map_err(|e| anyhow::anyhow!("加载 {} 失败: {e}", dll.display()))?.commit(); // 加载并提交环境
    if committed { log_info!("omni", "ORT 动态库={}", dll.display()); } // 首次成功记录
    Ok(())
}
static VISION: OnceLock<Mutex<omni_parser::OmniParser>> = OnceLock::new(); // 常驻视觉解析器
pub fn parse_png_region(png: &[u8], origin_x: i32, origin_y: i32, width: u32, height: u32, max_items: usize) -> Result<VisionParseOutput> { // 在内存中对 PNG 做 omni_parser 识别，返回结构化元素（含无文本 icon 框）
    init_ort()?; // 初始化 CUDA 运行时
    if VISION.get().is_none() { // 首次构建常驻解析器
        let dir = resolve_models_dir(None)?; // 自动探测模型
        let parser = omni_parser::OmniParser::new(&dir, Default::default())?; // v2 + OCR
        let _ = VISION.set(Mutex::new(parser)); // 常驻复用
    }
    let lock = VISION.get().ok_or_else(|| anyhow::anyhow!("视觉解析器初始化失败"))?; // 取实例
    let mut parser = lock.lock().map_err(|e| anyhow::anyhow!("视觉解析器锁失败: {e}"))?; // 加锁
    let start = std::time::Instant::now(); // 计时
    let img = image::load_from_memory(png)?; // 内存中解码 PNG（不落盘）
    let iw = width.max(img.width()).max(1) as f32; // 图像宽
    let ih = height.max(img.height()).max(1) as f32; // 图像高
    let raw = parser.parse(&img)?; // 识别文本与图标框
    let mut elements = Vec::new(); // 结构化元素集合
    let mut text_count = 0usize; // 文本计数
    let mut icon_count = 0usize; // 图标计数
    let mut summary_lines = Vec::new(); // 紧凑 Markdown 文本清单
    for item in raw.into_iter().take(max_items.max(1)) { // 保留文本与图标框（即使 icon 文本为空也不丢弃）
        let text = item.content.unwrap_or_default().trim().to_string(); // 读取文本
        let is_icon = item.kind == "icon"; // 是否图标检测框
        if !is_icon && text.is_empty() { continue; } // 仅丢弃空 OCR 文本，icon 一律保留坐标框
        let x1 = (item.bbox[0].clamp(0.0, 1.0) * iw).round() as i32; // 局部左
        let y1 = (item.bbox[1].clamp(0.0, 1.0) * ih).round() as i32; // 局部上
        let x2 = (item.bbox[2].clamp(0.0, 1.0) * iw).round() as i32; // 局部右
        let y2 = (item.bbox[3].clamp(0.0, 1.0) * ih).round() as i32; // 局部下
        let cx = (x1 + x2) / 2; // 局部中心 X
        let cy = (y1 + y2) / 2; // 局部中心 Y
        let idx = elements.len(); // 顺序下标
        if is_icon { icon_count += 1; } else { text_count += 1; } // 统计分类
        if summary_lines.len() < 160 { // 摘要控制行数上限
            let w = (x2 - x1).max(0); let h = (y2 - y1).max(0); // 宽高
            let label = if text.is_empty() { "[icon]".to_string() } else { format!("{text:?}") }; // 标签文本
            summary_lines.push(format!("[V{idx}] {} {label} @({cx},{cy}) {w}x{h}", item.kind)); // 紧凑输出
        }
        elements.push(ParsedVisualElement { index: idx, kind: item.kind, content: text, interactivity: item.interactivity, center: ParsedVisualCenter { x: cx, y: cy }, x: cx, y: cy, screen_x: origin_x + cx, screen_y: origin_y + cy, bbox: [x1, y1, x2, y2], bbox_screen: [origin_x + x1, origin_y + y1, origin_x + x2, origin_y + y2] }); // 写入元素
    }
    let ms = start.elapsed().as_millis(); // 总耗时
    let summary = if summary_lines.is_empty() { "未识别到文本或图标控件".to_string() } else { summary_lines.join("\n") }; // 拼接摘要
    Ok(VisionParseOutput { elements, text_count, icon_count, ms, summary }) // 返回完整识别结果
}
pub fn run_serve(models_dir: Option<PathBuf>) -> Result<()> { // 运行 stdio JSON 服务
    init_ort()?; // 初始化 CUDA 运行时
    let dir = resolve_models_dir(models_dir)?; // 解析模型目录
    log_info!("omni", "启动 omni serve，models={}", dir.display()); // 记录
    omni_parser::run_stdio_server(dir) // 进入服务循环
}
pub fn run_parse(image: PathBuf, models_dir: Option<PathBuf>, model: String) -> Result<()> { // 单图 CLI 解析入口
    init_ort()?; // 初始化 CUDA 运行时
    let dir = resolve_models_dir(models_dir)?; // 解析模型目录
    let img = image::open(&image)?; // 读图
    let mut config = omni_parser::OmniParserConfig::default(); // 默认配置
    config.detector_onnx = match model.as_str() { "v3" => "ms_vision/icon_detect_v3.onnx".to_string(), "v2" => "ms_vision/icon_detect.onnx".to_string(), other => other.to_string() }; // 模型文件名
    let mut parser = omni_parser::OmniParser::new(&dir, config)?; // 构建解析器
    let start = std::time::Instant::now(); // 计时
    let elements = parser.parse(&img)?; // 解析
    println!("{}", serde_json::json!({"bboxes": elements, "models_dir": dir, "ms": start.elapsed().as_millis()})); // 输出 JSON 行
    Ok(())
}