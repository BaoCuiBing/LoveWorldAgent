use super::ort_util::{build_session, extract_f32, owned_f32}; // ORT 工具
use crate::log_info; // 日志宏
use anyhow::Result; // 错误类型
use image::{DynamicImage, GenericImageView}; // 图像
use ort::session::Session; // 会话
pub struct Detector { session: Session, input_name: String, conf_threshold: f32, iou_threshold: f32, net: Option<(u32, u32)> } // 图标检测器
pub struct DetBox { pub bbox: [f32; 4], pub conf: f32 } // 检测结果
struct PreDims { pad_left: f32, pad_top: f32, scale: f32, ow: f32, oh: f32 } // letterbox 参数
impl PreDims {
    fn to_norm(&self, x1: f32, y1: f32, x2: f32, y2: f32) -> [f32; 4] { // 输入坐标转原图归一化
        [(((x1 - self.pad_left) / self.scale) / self.ow).clamp(0.0, 1.0), (((y1 - self.pad_top) / self.scale) / self.oh).clamp(0.0, 1.0), (((x2 - self.pad_left) / self.scale) / self.ow).clamp(0.0, 1.0), (((y2 - self.pad_top) / self.scale) / self.oh).clamp(0.0, 1.0)] // x1/y1/x2/y2
    }
}
impl Detector {
    pub fn new(model_path: &str, conf_threshold: f32, iou_threshold: f32) -> Result<Self> { // 构建检测会话
        let session = build_session(model_path, "icon_detect")?; // DirectML 会话
        let first = session.inputs().first(); // 首个输入
        let input_name = first.map(|i| i.name().to_string()).unwrap_or_else(|| "images".to_string()); // 输入名
        let net = first.and_then(|i| match i.dtype() { ort::value::ValueType::Tensor { shape, .. } => { let h = shape.get(2).copied().filter(|v| *v > 0).map(|v| v as u32); let w = shape.get(3).copied().filter(|v| *v > 0).map(|v| v as u32); w.zip(h) } _ => None }); // 固定输入尺寸
        if let Some((w, h)) = net { log_info!("omni", "icon_detect 固定输入 {w}x{h}，使用 letterbox 预处理"); } // 记录固定尺寸
        Ok(Self { session, input_name, conf_threshold, iou_threshold, net }) // 返回检测器
    }
    pub fn detect(&mut self, img: &DynamicImage, bbox_threshold: f32, iou_threshold: Option<f32>) -> Result<Vec<DetBox>> { // 执行检测+NMS
        let conf = if bbox_threshold > 0.0 { bbox_threshold } else { self.conf_threshold }; // 置信度阈值
        let iou = iou_threshold.unwrap_or(self.iou_threshold); // IoU 阈值
        let (data, shape, dims) = match self.net { Some((w, h)) => preprocess_inner(img, w, h)?, None => preprocess(img)? }; // 预处理
        let tensor = owned_f32(data, shape)?; // 输入张量
        let outputs = self.session.run(ort::inputs![self.input_name.as_str() => tensor]).map_err(|e| anyhow::anyhow!("检测推理失败: {e}"))?; // 推理
        let out = outputs.get("output0").or_else(|| outputs.get("output")).ok_or_else(|| anyhow::anyhow!("检测模型输出缺失"))?; // 取输出
        let (oshape, pred) = extract_f32(out)?; // 提取预测
        let (chan, n) = (oshape[1], oshape[2]); // 通道数/候选数
        let mut max_conf = f32::MIN; // 最高置信度
        let mut above = 0usize; // 超阈值计数
        for i in 0..n { let c = pred[idx(&[0, 4, i], &oshape)]; max_conf = max_conf.max(c); if c >= conf { above += 1; } } // 统计置信度
        log_info!("omni", "icon_detect 输出 shape={oshape:?} conf_max={max_conf:.3} 阈值{conf:.3}命中={above}/{n}"); // 检测统计
        let mut candidates: Vec<DetBox> = Vec::new(); // 候选框
        for i in 0..n { // 逐候选转换
            let conf_val = pred[idx(&[0, 4, i], &oshape)]; // 当前置信度
            if conf_val < conf { continue; } // 低于阈值跳过
            let (xc, yc, bw, bh) = (pred[idx(&[0, 0, i], &oshape)], pred[idx(&[0, 1, i], &oshape)], pred[idx(&[0, 2, i], &oshape)], pred[idx(&[0, 3, i], &oshape)]); // 中心框
            let _ = chan; // 预留通道
            candidates.push(DetBox { bbox: dims.to_norm(xc - bw / 2.0, yc - bh / 2.0, xc + bw / 2.0, yc + bh / 2.0), conf: conf_val }); // 转归一化框
        }
        let num_candidates = candidates.len(); // 候选数
        let kept = nms(candidates, iou); // 非极大值抑制
        log_info!("omni", "NMS 后 {}/{}，top1={:?}", kept.len(), num_candidates, kept.first().map(|b| (b.bbox, b.conf))); // NMS 统计
        Ok(kept) // 返回结果
    }
}
pub fn idx(ix: &[usize], shape: &[usize]) -> usize { // 行主序线性索引
    let mut off = 0usize; // 偏移
    for (k, &i) in ix.iter().enumerate() { off = off * shape[k] + i; } // 逐维累加
    off // 返回偏移
}
fn preprocess(img: &DynamicImage) -> Result<(Vec<f32>, Vec<usize>, PreDims)> { // 动态尺寸预处理
    let (ow, oh) = img.dimensions(); // 原图尺寸
    let net_w = (((ow + 31) / 32) * 32).clamp(64, 1920); // 宽对齐 32
    let net_h = (((oh + 31) / 32) * 32).clamp(64, 1920); // 高对齐 32
    preprocess_inner(img, net_w, net_h) // 进入 letterbox
}
fn preprocess_inner(img: &DynamicImage, net_w: u32, net_h: u32) -> Result<(Vec<f32>, Vec<usize>, PreDims)> { // letterbox 预处理
    let (ow, oh) = img.dimensions(); // 原图尺寸
    let scale = (net_w as f32 / ow as f32).min(net_h as f32 / oh as f32); // 等比缩放
    let new_w = ((ow as f32 * scale).round() as u32).max(1); // 缩放后宽
    let new_h = ((oh as f32 * scale).round() as u32).max(1); // 缩放后高
    let resized = img.resize_exact(new_w, new_h, image::imageops::FilterType::Triangle).to_rgb8(); // 缩放图像
    let hw = (net_h * net_w) as usize; // 单通道像素数
    let mut data = vec![0f32; 3 * hw]; // 三通道缓冲
    let pad_left = ((net_w - new_w) / 2) as usize; // 左侧填充
    let pad_top = ((net_h - new_h) / 2) as usize; // 顶部填充
    for (x, y, p) in resized.enumerate_pixels() { // 逐像素写入
        let (xi, yi) = (x as usize + pad_left, y as usize + pad_top); // 目标坐标
        let base = yi * net_w as usize + xi; // 通道内偏移
        data[base] = p[0] as f32 / 255.0; // R
        data[hw + base] = p[1] as f32 / 255.0; // G
        data[2 * hw + base] = p[2] as f32 / 255.0; // B
    }
    Ok((data, vec![1, 3, net_h as usize, net_w as usize], PreDims { pad_left: pad_left as f32, pad_top: pad_top as f32, scale, ow: ow as f32, oh: oh as f32 })) // 返回数据/形状/反算参数
}
fn nms(mut boxes: Vec<DetBox>, iou_threshold: f32) -> Vec<DetBox> { // 非极大值抑制
    boxes.sort_by(|a, b| b.conf.partial_cmp(&a.conf).unwrap_or(std::cmp::Ordering::Equal)); // 置信度降序
    let mut keep: Vec<DetBox> = Vec::new(); // 保留框
    'outer: for b in boxes { // 逐候选
        for k in &keep { // 与已保留框比较
            let ix = (b.bbox[2].min(k.bbox[2]) - b.bbox[0].max(k.bbox[0])).max(0.0); // 交集宽
            let iy = (b.bbox[3].min(k.bbox[3]) - b.bbox[1].max(k.bbox[1])).max(0.0); // 交集高
            let inter = ix * iy; // 交集面积
            let area_b = (b.bbox[2] - b.bbox[0]) * (b.bbox[3] - b.bbox[1]); // 当前面积
            let area_k = (k.bbox[2] - k.bbox[0]) * (k.bbox[3] - k.bbox[1]); // 已留面积
            let iou = inter / (area_b + area_k - inter + 1e-9); // IoU
            if iou > iou_threshold { continue 'outer; } // 抑制
        }
        keep.push(b); // 保留
    }
    keep // 返回
}