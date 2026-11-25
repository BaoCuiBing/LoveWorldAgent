use super::detector::idx; // 线性索引
use super::ort_util::{build_session, extract_f32, owned_i64}; // ORT 工具
use anyhow::Result; // 错误类型
use image::RgbaImage; // 图像
use ort::session::Session; // 会话
use std::path::PathBuf; // 路径
use tokenizers::Tokenizer; // 分词器
pub struct Captioner { encoder: Session, decoder: Session, tokenizer: Tokenizer, task_token_id: u32, decoder_start_token_id: i64, eos_token_id: i64, max_new_tokens: usize } // Florence2 图标描述器
impl Captioner {
    pub fn new(models_dir: &PathBuf) -> Result<Self> { // 加载编码/解码模型与分词器
        let encoder = build_session(models_dir.join("florence2_encoder.onnx").to_str().unwrap(), "florence2_encoder")?; // 编码器
        let decoder = build_session(models_dir.join("florence2_decoder.onnx").to_str().unwrap(), "florence2_decoder")?; // 解码器
        let tokenizer = Tokenizer::from_file(models_dir.join("tokenizer.json")).map_err(|e| anyhow::anyhow!("加载 tokenizer 失败: {e}"))?; // 分词器
        let task_token_id = tokenizer.token_to_id("<CAPTION>").ok_or_else(|| anyhow::anyhow!("tokenizer 缺少 <CAPTION> token"))?; // 任务 token
        Ok(Self { encoder, decoder, tokenizer, task_token_id, decoder_start_token_id: 2, eos_token_id: 2, max_new_tokens: 20 }) // 固定解码参数
    }
    pub fn caption_batch(&mut self, images: &[&RgbaImage]) -> Result<Vec<String>> { // 批量描述图标
        let mut results = Vec::with_capacity(images.len()); // 结果集
        for img in images { // 逐图推理
            let (pix, pix_shape) = preprocess_pixels(img); // 预处理像素
            let input_ids: Vec<i64> = vec![self.task_token_id as i64]; // 任务输入
            let attention: Vec<i64> = vec![1]; // 注意力掩码
            let enc = self.encoder.run(ort::inputs!["input_ids" => owned_i64(input_ids.clone(), vec![1, input_ids.len()])?, "attention_mask" => owned_i64(attention.clone(), vec![1, attention.len()])?, "pixel_values" => super::ort_util::owned_f32(pix, pix_shape)?])?; // 编码
            let enc_out = enc.get("encoder_hidden_states").ok_or_else(|| anyhow::anyhow!("encoder 输出缺失"))?; // 取编码输出
            let (hidden_shape, hidden_data) = extract_f32(enc_out)?; // 提取张量
            let mut seq: Vec<i64> = vec![self.decoder_start_token_id]; // 解码序列
            for _ in 0..self.max_new_tokens { // 自回归解码
                let out = self.decoder.run(ort::inputs!["decoder_input_ids" => owned_i64(seq.clone(), vec![1, seq.len()])?, "encoder_hidden_states" => super::ort_util::owned_f32(hidden_data.clone(), hidden_shape.clone())?])?; // 解码一步
                let name = out.keys().next().map(|k| k.to_string()).ok_or_else(|| anyhow::anyhow!("decoder 输出缺失"))?; // 输出名
                let value = out.get(&name).ok_or_else(|| anyhow::anyhow!("decoder 输出缺失"))?; // 输出张量
                let (lshape, ldata) = extract_f32(value)?; // 提取 logits
                let l = lshape[1]; // 序列长度
                let v = lshape[2]; // 词表大小
                let mut best = (0usize, f32::MIN); // argmax
                for k in 0..v { let val = ldata[idx(&[0, l - 1, k], &lshape)]; if val > best.1 { best = (k, val); } } // 找最大 logit
                let tok = best.0 as i64; // 选中 token
                if tok == self.eos_token_id { break; } // 遇到结束符
                seq.push(tok); // 追加 token
            }
            let text_ids: Vec<u32> = seq.iter().filter(|&&t| t != self.decoder_start_token_id && t != self.eos_token_id).map(|&t| t as u32).collect(); // 去掉起止符
            let text = self.tokenizer.decode(&text_ids, true).unwrap_or_default().trim().to_string(); // 解码文本
            results.push(text); // 收集结果
        }
        Ok(results) // 返回批量结果
    }
}
fn preprocess_pixels(img: &RgbaImage) -> (Vec<f32>, Vec<usize>) { // 384x384 归一化
    let resized = image::imageops::resize(img, 384, 384, image::imageops::FilterType::Triangle); // 缩放
    let rgb = image::DynamicImage::ImageRgba8(resized).to_rgb8(); // 转 RGB
    let hw = 384 * 384; // 单通道像素数
    let mut data = vec![0f32; 3 * hw]; // 三通道缓冲
    for (x, y, p) in rgb.enumerate_pixels() { // 逐像素写入
        let base = y as usize * 384 + x as usize; // 通道内偏移
        data[base] = (p[0] as f32 / 255.0 - 0.485) / 0.229; // R 归一化
        data[hw + base] = (p[1] as f32 / 255.0 - 0.456) / 0.224; // G 归一化
        data[2 * hw + base] = (p[2] as f32 / 255.0 - 0.406) / 0.225; // B 归一化
    }
    (data, vec![1, 3, 384, 384]) // 返回数据与形状
}