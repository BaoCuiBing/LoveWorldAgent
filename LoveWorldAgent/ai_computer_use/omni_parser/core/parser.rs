use super::bbox::Element; // 元素结构
use super::caption::Captioner; // 图标描述
use super::detector::Detector; // 图标检测
use super::geometry::BBox; // 几何框
use super::ocr::Ocr; // 文本识别
use crate::{log_info, log_warn}; // 日志宏
use anyhow::Result; // 错误类型
use image::{DynamicImage, RgbaImage}; // 图像
use std::path::PathBuf; // 路径
pub struct OmniParserConfig { // 解析配置
    pub bbox_threshold: f32, // 检测置信阈值
    pub iou_threshold: f32, // 检测 IoU 阈值
    pub caption_batch_size: usize, // 描述批量
    pub enable_ocr: bool, // 启用 OCR
    pub enable_caption: bool, // 启用图标描述
    pub detector_onnx: String, // 检测模型文件名
}
impl Default for OmniParserConfig { fn default() -> Self { Self { bbox_threshold: 0.05, iou_threshold: 0.7, caption_batch_size: 64, enable_ocr: true, enable_caption: true, detector_onnx: "ms_vision/icon_detect.onnx".to_string() } } } // 默认配置
pub struct OmniParser { detector: Detector, ocr: Option<Ocr>, captioner: Option<Captioner>, pub config: OmniParserConfig } // 解析器
impl OmniParser {
    pub fn new(models_dir: &PathBuf, config: OmniParserConfig) -> Result<Self> { // 按模型可用性装配
        let det_path = models_dir.join(&config.detector_onnx); // 检测模型路径（ms_vision/icon_detect*.onnx）
        let detector = Detector::new(det_path.to_str().unwrap(), config.bbox_threshold, config.iou_threshold)?; // 检测器
        let ocr_dir = models_dir.join("ocr"); // OCR 模型目录
        let ocr_det = ocr_dir.join("ch_PP-OCRv4_det_infer.onnx"); // DB 检测
        let ocr_rec = ocr_dir.join("ch_PP-OCRv4_rec_infer.onnx"); // CTC 识别
        let ocr_keys = ocr_dir.join("ppocr_keys_v1.txt"); // 中英字典
        let ocr = if config.enable_ocr && ocr_det.is_file() && ocr_rec.is_file() && ocr_keys.is_file() { Some(Ocr::new(ocr_det.to_str().unwrap(), ocr_rec.to_str().unwrap(), ocr_keys.to_str().unwrap())?) } else { None }; // OCR 可选
        let vision_dir = models_dir.join("ms_vision"); // 微软视觉模型目录
        let captioner = if config.enable_caption && vision_dir.join("florence2_encoder.onnx").is_file() && vision_dir.join("florence2_decoder.onnx").is_file() && vision_dir.join("tokenizer.json").is_file() { Some(Captioner::new(&vision_dir)?) } else { None }; // 描述可选
        if ocr.is_none() && config.enable_ocr { log_warn!("omni", "OCR 模型缺失（ocr/ch_PP-OCRv4_det/rec_infer.onnx、ppocr_keys_v1.txt），已禁用 OCR"); } // OCR 缺失告警
        if captioner.is_none() && config.enable_caption { log_warn!("omni", "caption 模型缺失（ms_vision/florence2_encoder/decoder.onnx），已禁用图标描述"); } // 描述缺失告警
        Ok(Self { detector, ocr, captioner, config }) // 返回解析器
    }
    pub fn parse(&mut self, img: &DynamicImage) -> Result<Vec<Element>> { // 解析单图
        let (w, h) = (img.width() as f32, img.height() as f32); // 原图尺寸
        let rgba = img.to_rgba8(); // RGBA
        let (ocr_texts, ocr_bboxes_norm): (Vec<String>, Vec<[f32; 4]>) = match &mut self.ocr { // OCR 结果
            Some(ocr) if self.config.enable_ocr => { let res = ocr.read_text(&rgba)?; let texts = res.iter().map(|r| r.text.clone()).collect(); let bboxes = res.iter().map(|r| [r.bbox[0] as f32 / w, r.bbox[1] as f32 / h, r.bbox[2] as f32 / w, r.bbox[3] as f32 / h]).collect(); (texts, bboxes) } // 识别并归一化
            _ => (Vec::new(), Vec::new()), // 未启用
        };
        let det_boxes = self.detector.detect(img, self.config.bbox_threshold, Some(self.config.iou_threshold))?; // 图标检测
        let mut elements: Vec<Element> = Vec::new(); // 元素集
        for b in &ocr_bboxes_norm { // 先放文本元素
            let bb = BBox::new(b[0], b[1], b[2], b[3]); // 框
            if bb.area() <= 0.0 { continue; } // 空框跳过
            elements.push(Element { kind: "text".into(), bbox: [b[0], b[1], b[2], b[3]], interactivity: false, content: None, source: Some("box_ocr_content_ocr".into()) }); // 文本元素
        }
        elements.iter_mut().zip(ocr_texts.iter()).for_each(|(e, t)| e.content = Some(t.clone())); // 回填 OCR 文本
        let icon_count_start = elements.len(); // 图标起始下标
        for b in &det_boxes { // 再放图标元素
            let bb = BBox::new(b.bbox[0], b.bbox[1], b.bbox[2], b.bbox[3]); // 框
            if bb.area() <= 0.0 { continue; } // 空框跳过
            elements.push(Element { kind: "icon".into(), bbox: b.bbox, interactivity: true, content: None, source: Some("box_yolo_content_yolo".into()) }); // 图标元素
        }
        let filtered = if ocr_bboxes_norm.is_empty() { elements[icon_count_start..].to_vec() } else { remove_bbox_overlap(elements[icon_count_start..].to_vec(), elements[..icon_count_start].to_vec(), self.config.iou_threshold) }; // 去重叠

        log_info!("omni", "元素合并：icons={} ocr={} 输出={}", elements.len() - icon_count_start, icon_count_start, filtered.len()); // 合并统计
        let mut sorted: Vec<Element> = filtered.into_iter().collect(); // 可排序副本
        sorted.sort_by_key(|e| e.content.is_none()); // 有文本的排前
        if let Some(cap) = &mut self.captioner { // 需要图标描述
            let to_caption: Vec<(usize, RgbaImage)> = sorted.iter().enumerate().filter(|(_, e)| e.content.is_none()).filter_map(|(idx, e)| { // 收集无文本图标
                let [x1, y1, x2, y2] = e.bbox; // 归一化框
                let (iw, ih) = (rgba.width() as f32, rgba.height() as f32); // 图像尺寸
                let (ix1, iy1, ix2, iy2) = ((x1 * iw).round() as i32, (y1 * ih).round() as i32, (x2 * iw).round() as i32, (y2 * ih).round() as i32); // 像素框
                if ix2 <= ix1 || iy2 <= iy1 { return None; } // 空框跳过
                let crop = image::imageops::crop_imm(&rgba, ix1 as u32, iy1 as u32, (ix2 - ix1) as u32, (iy2 - iy1) as u32).to_image(); // 裁剪
                let resized = image::imageops::resize(&crop, 64, 64, image::imageops::FilterType::Triangle); // 缩放到 64
                Some((idx, resized)) // 返回索引与图
            }).collect();
            for chunk in to_caption.chunks(self.config.caption_batch_size) { // 分批描述
                let captions = cap.caption_batch(&chunk.iter().map(|(_, im)| im).collect::<Vec<_>>())?; // 批量推理
                for ((idx, _), text) in chunk.iter().zip(captions) { if let Some(e) = sorted.get_mut(*idx) { e.content = Some(text); } } // 回填描述
            }
        } else { for e in sorted.iter_mut().filter(|e| e.content.is_none()) { e.content = Some(String::new()); } } // 无描述器填空串
        Ok(sorted) // 返回元素
    }
}
pub fn remove_bbox_overlap(xyxy_bboxes: Vec<Element>, ocr_bboxes: Vec<Element>, iou_threshold: f32) -> Vec<Element> { // 合并 OCR 标签（图标已由 NMS 去重）
    let mut icons = xyxy_bboxes; // 图标集直接沿用 NMS 结果
    for o in ocr_bboxes { // 合并 OCR 文本
        let ob = to_bbox(&o.bbox); // OCR 框
        let mut merged = false; // 是否已合并
        for icon in icons.iter_mut() { // 找重叠图标
            let ib = to_bbox(&icon.bbox); // 图标框
            if ob.iou(&ib) > iou_threshold { // 高度重叠
                if let Some(text) = o.content.clone() { icon.content = Some(match icon.content.clone() { Some(c) if !c.is_empty() => format!("{c} {text}"), _ => text }); } // 合并文本
                merged = true; break; // 完成合并
            }
        }
        if !merged { icons.push(o); } // 未重叠则保留文本元素
    }
    icons // 返回
}
fn to_bbox(b: &[f32; 4]) -> BBox { BBox::new(b[0], b[1], b[2], b[3]) } // 数组转 BBox