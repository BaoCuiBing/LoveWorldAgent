use super::ort_util::{build_session, extract_f32, owned_f32}; // ORT 工具
use anyhow::Result; // 错误类型
use image::RgbaImage; // 图像
use ort::session::Session; // 会话
use std::collections::VecDeque; // 连通域队列
pub struct Ocr { detector: Session, recognizer: Session, charset: Vec<char>, det_input: String, rec_input: String } // PP-OCRv4 DB 检测 + CTC 识别
pub struct OcrResult { pub text: String, pub bbox: [i32; 4] } // 文本与像素框
const DET_LIMIT: u32 = 1280; // 检测长边上限
const DB_THRESH: f32 = 0.4; // 概率二值化阈值
const DB_BOX_THRESH: f32 = 0.6; // 框平均概率阈值
const REC_HEIGHT: u32 = 48; // 识别输入高
const REC_MAX_W: u32 = 320; // 识别最大宽
impl Ocr {
    pub fn new(det_path: &str, rec_path: &str, charset_path: &str) -> Result<Self> { // 加载 DB/CTC 双模型与字典
        let detector = build_session(det_path, "ocr_det")?; // DB 检测会话
        let recognizer = build_session(rec_path, "ocr_rec")?; // CTC 识别会话
        let charset_raw = std::fs::read_to_string(charset_path).map_err(|e| anyhow::anyhow!("读取字符字典失败 {charset_path}: {e}"))?; // 读字典
        let mut charset: Vec<char> = charset_raw.lines().map(|l| l.chars().next().unwrap_or(' ')).collect(); // 每行首字符
        let det_input = detector.inputs().first().map(|i| i.name().to_string()).unwrap_or_else(|| "x".to_string()); // 检测输入名
        let rec_input = recognizer.inputs().first().map(|i| i.name().to_string()).unwrap_or_else(|| "x".to_string()); // 识别输入名
        let rec_cls = recognizer.outputs().first().and_then(|o| match o.dtype() { ort::value::ValueType::Tensor { shape, .. } => shape.last().copied(), _ => None }).unwrap_or(0) as usize; // 模型类别数
        if rec_cls == charset.len() + 2 { charset.push(' '); } // PaddleOCR use_space_char 追加空格
        crate::log_info!("omni", "PP-OCR 字典={} 模型类别={} det输入={} rec输入={}", charset.len(), rec_cls, det_input, rec_input); // 记录模型信息
        Ok(Self { detector, recognizer, charset, det_input, rec_input }) // 返回实例
    }
    pub fn read_text(&mut self, img: &RgbaImage) -> Result<Vec<OcrResult>> { // DB 检测并逐框识别
        let (nw, nh) = det_resize(img); // 检测尺寸
        let resized = image::imageops::resize(img, nw, nh, image::imageops::FilterType::Triangle); // 缩放图像
        let (data, shape) = db_preprocess(&resized); // DB 归一化
        let outputs = self.detector.run(ort::inputs![self.det_input.as_str() => owned_f32(data, shape)?])?; // 检测推理
        let name = outputs.keys().next().map(|k| k.to_string()).ok_or_else(|| anyhow::anyhow!("DB 输出为空"))?; // 输出名
        let out = outputs.get(&name).ok_or_else(|| anyhow::anyhow!("DB 输出缺失"))?; // 输出张量
        let (oshape, prob) = extract_f32(out)?; // 概率图
        drop(outputs); // 释放检测输出（解除 detector 借用）
        let (hh, ww) = (oshape[oshape.len() - 2], oshape[oshape.len() - 1]); // 输出尺寸
        if prob.len() < hh * ww { anyhow::bail!("DB 输出尺寸不符: {} < {}x{}", prob.len(), ww, hh); } // 尺寸校验
        let boxes = db_boxes(&prob, ww, hh, img.width(), img.height()); // 文本框
        let mut crops: Vec<(RgbaImage, [i32; 4])> = Vec::new(); // 待识别文本块
        for b in boxes { // 逐框裁剪
            let crop = image::imageops::crop_imm(img, b[0] as u32, b[1] as u32, (b[2] - b[0]) as u32, (b[3] - b[1]) as u32).to_image(); // 裁剪
            if crop.width() >= 4 && crop.height() >= 4 { crops.push((crop, b)); } // 过滤过小
        }
        let refs: Vec<&RgbaImage> = crops.iter().map(|(c, _)| c).collect(); // 借用视图
        let texts = self.recognize_batch(&refs)?; // 批量识别
        let mut results = Vec::new(); // 结果集
        for ((_, b), (text, conf)) in crops.iter().zip(texts) { if !text.trim().is_empty() && conf >= 0.3 { results.push(OcrResult { text: text.trim().to_string(), bbox: *b }); } } // 过滤低置信
        Ok(results) // 返回结果
    }
    fn recognize_batch(&mut self, imgs: &[&RgbaImage]) -> Result<Vec<(String, f32)>> { // 批量 CTC 识别
        let mut out = Vec::with_capacity(imgs.len()); // 结果
        let cls = self.charset.len() + 1; // blank + 字典
        for chunk in imgs.chunks(16) { // 16 个一批
            let (data, shape) = rec_batch_preprocess(chunk); // 批预处理
            let outputs = self.recognizer.run(ort::inputs![self.rec_input.as_str() => owned_f32(data, shape)?])?; // 批量推理
            let name = outputs.keys().next().map(|k| k.to_string()).ok_or_else(|| anyhow::anyhow!("CTC 输出为空"))?; // 输出名
            let t = outputs.get(&name).ok_or_else(|| anyhow::anyhow!("CTC 输出缺失"))?; // 输出张量
            let (_, pred) = extract_f32(t)?; // logits
            drop(outputs); // 释放输出
            if pred.len() < cls * chunk.len() { for _ in 0..chunk.len() { out.push((String::new(), 0.0)); } continue; } // 输出不足
            let t_len = pred.len() / (cls * chunk.len()); // 每样本时间步
            for bi in 0..chunk.len() { // 逐样本解码
                let mut prev = -1i32; // 上一步类别
                let mut text = String::new(); // 文本
                let mut conf_sum = 0.0; // 置信度和
                let mut conf_n = 0usize; // 字符数
                for t in 0..t_len { // 逐时间步
                    let mut best = (-1i32, f32::MIN); // 当前最优
                    for c in 0..cls { let v = pred[(bi * t_len + t) * cls + c]; if v > best.1 { best = (c as i32, v); } } // argmax
                    if best.0 != prev && best.0 > 0 { if let Some(ch) = self.charset.get(best.0 as usize - 1) { text.push(*ch); conf_sum += best.1; conf_n += 1; } } // CTC 去重
                    prev = best.0; // 更新
                }
                let conf = if conf_n > 0 { conf_sum / conf_n as f32 } else { 0.0 }; // 平均置信度
                out.push((text, conf)); // 收集
            }
        }
        Ok(out) // 返回
    }
}
fn rec_batch_preprocess(imgs: &[&RgbaImage]) -> (Vec<f32>, Vec<usize>) { // 固定 320x48 批预处理
    let b = imgs.len(); // 批大小
    let h = REC_HEIGHT as usize; // 高
    let w = REC_MAX_W as usize; // 宽
    let hw = h * w; // 单通道像素数
    let mut data = vec![0f32; b * 3 * hw]; // 批缓冲
    for (bi, img) in imgs.iter().copied().enumerate() { // 逐样本
        let ratio = img.width() as f32 / img.height() as f32; // 宽高比
        let tw = (ratio * h as f32).round().max(4.0).min(w as f32) as u32; // 目标宽
        let scaled = image::imageops::resize(img, tw, REC_HEIGHT, image::imageops::FilterType::Triangle); // 缩放
        let rgb = image::DynamicImage::ImageRgba8(scaled).to_rgb8(); // RGB
        for (x, y, p) in rgb.enumerate_pixels() { // 逐像素归一化
            let base = bi * 3 * hw + y as usize * w + x as usize; // 通道内偏移
            data[base] = (p[0] as f32 / 255.0 - 0.5) / 0.5; // R
            data[bi * 3 * hw + hw + y as usize * w + x as usize] = (p[1] as f32 / 255.0 - 0.5) / 0.5; // G
            data[bi * 3 * hw + 2 * hw + y as usize * w + x as usize] = (p[2] as f32 / 255.0 - 0.5) / 0.5; // B
        }
    }
    (data, vec![b, 3, h, w]) // 返回批数据与形状
}
fn det_resize(img: &RgbaImage) -> (u32, u32) { // 检测缩放（32 对齐、长边限制）
    let (w, h) = img.dimensions(); // 原尺寸
    let ratio = (DET_LIMIT as f32 / w.max(h) as f32).min(1.0); // 缩放比
    let nw = ((w as f32 * ratio).round() as u32).max(32).div_ceil(32) * 32; // 宽对齐
    let nh = ((h as f32 * ratio).round() as u32).max(32).div_ceil(32) * 32; // 高对齐
    (nw, nh) // 返回尺寸
}
fn db_preprocess(img: &RgbaImage) -> (Vec<f32>, Vec<usize>) { // DB 归一化（ImageNet 均值）
    let (iw, ih) = (img.width() as usize, img.height() as usize); // 尺寸
    let rgb = image::DynamicImage::ImageRgba8(img.clone()).to_rgb8(); // RGB
    let hw = iw * ih; // 单通道像素数
    let mut data = vec![0f32; 3 * hw]; // 三通道缓冲
    for (x, y, p) in rgb.enumerate_pixels() { // 逐像素写入
        let base = y as usize * iw + x as usize; // 通道内偏移
        data[base] = (p[0] as f32 / 255.0 - 0.485) / 0.229; // R
        data[hw + base] = (p[1] as f32 / 255.0 - 0.456) / 0.224; // G
        data[2 * hw + base] = (p[2] as f32 / 255.0 - 0.406) / 0.225; // B
    }
    (data, vec![1, 3, ih, iw]) // 返回数据与形状
}
fn db_boxes(prob: &[f32], ww: usize, hh: usize, ow: u32, oh: u32) -> Vec<[i32; 4]> { // DB 概率图转文本框
    let sx = ow as f32 / ww as f32; // 输出图→原图横向倍率
    let sy = oh as f32 / hh as f32; // 输出图→原图纵向倍率
    let mut mask = vec![0u8; hh * ww]; // 二值掩码
    for i in 0..hh * ww { if prob[i] > DB_THRESH { mask[i] = 1; } } // 阈值化
    let labels = connected_components(&mask, ww, hh); // 8 邻域标记
    let mut sum = vec![0f32; labels.max_label + 1]; // 概率和
    let mut cnt = vec![0usize; labels.max_label + 1]; // 像素数
    for i in 0..hh * ww { let l = labels.map[i]; if l > 0 { sum[l as usize] += prob[i]; cnt[l as usize] += 1; } } // 统计
    let mut boxes = Vec::new(); // 文本框
    for k in 1..=labels.max_label { // 逐连通域
        let st = &labels.stats[k]; // 统计
        if st.area < 8 || st.width < 2 || st.height < 2 || cnt[k] == 0 { continue; } // 过滤小噪点
        if (sum[k] / cnt[k] as f32) < DB_BOX_THRESH { continue; } // 过滤弱文本
        let pad = (st.width.min(st.height) as f32 * 0.3).max(2.0); // 外扩
        let x1 = ((st.left as f32 - pad) * sx).max(0.0); // 左
        let y1 = ((st.top as f32 - pad) * sy).max(0.0); // 上
        let x2 = (((st.left + st.width) as f32 + pad) * sx).min(ow as f32); // 右
        let y2 = (((st.top + st.height) as f32 + pad) * sy).min(oh as f32); // 下
        if x2 - x1 < 4.0 || y2 - y1 < 4.0 { continue; } // 过小跳过
        boxes.push([x1 as i32, y1 as i32, x2 as i32, y2 as i32]); // 收集框
    }
    boxes.sort_by_key(|b| (((b[1] + b[3]) / 2) / 8, b[0])); // 按行再按列排序

    boxes // 返回
}
struct Stat { left: usize, top: usize, width: usize, height: usize, area: usize } // 连通域统计
struct LabelResult { map: Vec<i32>, max_label: usize, stats: Vec<Stat> } // 标记结果
fn connected_components(bin: &[u8], w: usize, h: usize) -> LabelResult { // 8 邻域 BFS 标记
    let mut map = vec![-1i32; w * h]; // 标记图
    let mut stats = vec![Stat { left: 0, top: 0, width: 0, height: 0, area: 0 }]; // 下标 0 占位
    let mut cur = 0usize; // 当前标记
    let mut queue = VecDeque::new(); // BFS 队列
    for start in 0..w * h { // 扫描种子
        if bin[start] == 1 && map[start] == -1 { // 未标记前景
            cur += 1; // 新标记
            queue.push_back(start); // 入队
            map[start] = cur as i32; // 标记
            let (mut left, mut right) = (start % w, start % w); // 左右界
            let (mut top, mut bottom) = (start / w, start / w); // 上下界
            let mut area = 0usize; // 面积
            while let Some(p) = queue.pop_front() { // BFS
                area += 1; // 计数
                let x = p % w; // 列
                let y = p / w; // 行
                left = left.min(x); // 更新左
                right = right.max(x); // 更新右
                top = top.min(y); // 更新上
                bottom = bottom.max(y); // 更新下
                for (dx, dy) in [(-1i32, -1i32), (0, -1), (1, -1), (-1, 0), (1, 0), (-1, 1), (0, 1), (1, 1)] { // 8 邻域
                    let nx = x as i32 + dx; // 邻列
                    let ny = y as i32 + dy; // 邻行
                    if nx >= 0 && ny >= 0 && (nx as usize) < w && (ny as usize) < h { let n = ny as usize * w + nx as usize; if bin[n] == 1 && map[n] == -1 { map[n] = cur as i32; queue.push_back(n); } } // 未标记则入队
                }
            }
            stats.push(Stat { left, top, width: right - left + 1, height: bottom - top + 1, area }); // 记录统计
        }
    }
    LabelResult { map, max_label: cur, stats } // 返回
}