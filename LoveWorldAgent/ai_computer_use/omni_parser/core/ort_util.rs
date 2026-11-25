use anyhow::{anyhow, Result}; // 错误类型
use crate::{log_info, log_warn}; // 日志宏
use ort::ep::{cuda::ConvAlgorithmSearch, ArenaExtendStrategy}; // EP 配置枚举
use ort::{execution_providers::CUDAExecutionProvider, session::{builder::GraphOptimizationLevel, Session}, value::{Tensor, TensorValueType}}; // ORT 组件
pub fn build_session(path: &str, tag: &str) -> Result<Session> { // 统一构建 CUDA 优先会话
    match commit_cuda(path, tag) { // 先尝试 CUDA
        Ok(session) => { log_info!("omni", "{tag} 使用 CUDA EP（4GB 上限、省显存）"); Ok(session) } // CUDA 生效
        Err(err) => { log_warn!("omni", "{tag} CUDA 不可用（{err}），回退 CPU"); commit_cpu(path, tag) } // 回退 CPU
    }
}
fn commit_cuda(path: &str, tag: &str) -> Result<Session> { // CUDA EP 会话
    let builder = Session::builder().map_err(|e| anyhow!("{tag} builder 失败: {e}"))?; // 新建 builder
    let cuda = CUDAExecutionProvider::default().with_memory_limit(4 * 1024 * 1024 * 1024).with_arena_extend_strategy(ArenaExtendStrategy::SameAsRequested).with_conv_algorithm_search(ConvAlgorithmSearch::Heuristic).with_conv_max_workspace(false).build(); // 4GB 上限 + 按需扩展 + 启发式 + 小 workspace
    let builder = builder.with_execution_providers([cuda]).map_err(|e| anyhow!("{tag} CUDA EP 注册失败: {e}"))?; // 注册 CUDA
    builder.with_optimization_level(GraphOptimizationLevel::Level3).map_err(|e| anyhow!("{tag} 优化级别设置失败: {e}"))?.commit_from_file(path).map_err(|e| anyhow!("{tag} CUDA 加载失败 {path}: {e}")) // 优化并加载
}
fn commit_cpu(path: &str, tag: &str) -> Result<Session> { // CPU 回退会话
    let builder = Session::builder().map_err(|e| anyhow!("{tag} builder 失败: {e}"))?; // 新建 builder
    builder.with_optimization_level(GraphOptimizationLevel::Level3).map_err(|e| anyhow!("{tag} 优化级别设置失败: {e}"))?.commit_from_file(path).map_err(|e| anyhow!("{tag} 加载失败 {path}: {e}")) // 优化并加载
}
pub fn owned_f32(data: Vec<f32>, shape: Vec<usize>) -> Result<Tensor<f32>> { Tensor::from_array((shape, data)).map_err(|e| anyhow!("tensor 构建失败: {e}")) } // f32 张量
pub fn owned_i64(data: Vec<i64>, shape: Vec<usize>) -> Result<Tensor<i64>> { Tensor::from_array((shape, data)).map_err(|e| anyhow!("tensor 构建失败: {e}")) } // i64 张量
pub fn extract_f32(v: &ort::value::DynValue) -> Result<(Vec<usize>, Vec<f32>)> { // 提取 f32 输出
    let tensor = v.downcast_ref::<TensorValueType<f32>>().map_err(|e| anyhow!("输出张量类型不符: {e}"))?; // 类型校验
    let arr = tensor.try_extract_array::<f32>().map_err(|e| anyhow!("输出张量提取失败: {e}"))?; // 取数组
    let shape: Vec<usize> = arr.shape().to_vec(); // 形状
    let data: Vec<f32> = arr.iter().copied().collect(); // 数据
    Ok((shape, data)) // 返回形状与数据
}