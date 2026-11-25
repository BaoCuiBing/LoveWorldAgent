use axum::routing::get; // HTTP 服务：axum 路由器 + 全放行 CORS + webdist 静态服务（SPA 回退）+ 健康检查
use axum::{Json, Router}; // JSON 响应与路由器
use serde_json::json; // JSON 构造
use tower_http::cors::{Any, CorsLayer}; // CORS 中间件
use tower_http::services::{ServeDir, ServeFile}; // 静态文件服务
use crate::config; // 路径与 HTTP 配置
use crate::state; // 全局状态
use crate::utils::logger; // 日志输出
pub fn router() -> Router { // 组装 HTTP 路由器
    let index = config::WEB_DIST_DIR.join("index.html"); // SPA 入口
    let static_files = ServeDir::new((*config::WEB_DIST_DIR).clone()).fallback(ServeFile::new(index)); // 静态目录，未命中回退 index
    Router::new() // 创建路由器
        .route("/api/health", get(health)) // 健康检查
        .merge(crate::api::router()) // 合并桌面能力路由
        .merge(crate::rouer::register()) // 合并业务路由
        .fallback_service(static_files) // 其余走静态服务
        .layer(CorsLayer::new().allow_origin(Any).allow_methods(Any).allow_headers(Any)) // 全放行 CORS
        .with_state(state::shared()) // 注入全局状态
}
async fn health() -> impl axum::response::IntoResponse { // 健康检查响应
    Json(json!({"code": 200, "status": "healthy", "service": "LoveWorldAgent"})) // 返回健康状态
}
pub fn start_server(host: &str, port: u16) -> anyhow::Result<()> { // 在独立线程启动 HTTP 服务
    let addr = format!("{host}:{port}"); // 监听地址
    std::thread::Builder::new().name("http-server".to_string()).spawn(move || { // 后台线程
        let runtime = match tokio::runtime::Builder::new_multi_thread().enable_all().build() { // 创建 Tokio 运行时
            Ok(runtime) => runtime, // 成功
            Err(err) => { // 失败
                logger::error("server", &format!("HTTP 运行时创建失败: {err}")); // 记录
                return; // 退出
            }
        };
        runtime.block_on(async move { // 运行时内启动服务
            let listener = match tokio::net::TcpListener::bind(&addr).await { // 绑定端口
                Ok(listener) => listener, // 成功
                Err(err) => { // 失败
                    logger::error("server", &format!("HTTP 服务绑定失败 {addr}: {err}")); // 记录
                    return; // 退出
                }
            };
            logger::info("server", &format!("HTTP 服务已启动: http://{addr}/api/health")); // 记录
            if let Err(err) = axum::serve(listener, router()).await { logger::error("server", &format!("HTTP 服务异常退出: {err}")); } // 运行服务
        });
    })?;
    Ok(()) // 线程启动成功
}
