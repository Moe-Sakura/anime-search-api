mod bangumi;
mod core;
mod engine;
mod http_client;
mod rules;
mod types;
mod updater;

use axum::{
    body::Body,
    extract::{Multipart, Path, Request},
    http::{header, HeaderMap, Method, StatusCode},
    response::{Html, IntoResponse, Response},
    routing::{any, get, post},
    Json, Router,
};
use futures::StreamExt;
use serde_json::json;
use std::net::SocketAddr;
use tower_http::cors::{Any, CorsLayer};
use tracing::{info, Level};
use tracing_subscriber::FmtSubscriber;

use crate::core::search_stream_with_rules_options;
use crate::rules::get_builtin_rules;

#[tokio::main]
async fn main() {
    // 初始化日志
    let _subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .with_target(false)
        .with_thread_ids(false)
        .with_file(false)
        .with_line_number(false)
        .init();

    // CORS 配置
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods([Method::GET, Method::POST, Method::OPTIONS])
        .allow_headers([header::CONTENT_TYPE]);

    // 检查启动时是否自动更新规则
    if std::env::var("AUTO_UPDATE").unwrap_or_default() == "1" {
        info!("📡 正在检查规则更新...");
        let result = updater::update_rules().await;
        info!(
            "📦 更新完成: {} 新增, {} 更新, {} 失败",
            result.added, result.updated, result.failed
        );
    }

    // 路由
    let app = Router::new()
        // 核心路由
        .route("/", get(index_handler))
        .route("/api", post(search_handler))
        .route("/info", get(api_info_handler))
        .route("/rules", get(rules_handler))
        .route("/update", get(update_handler))
        .route("/health", get(health_handler))
        // Bangumi API 通用代理 (透传到 api.bgm.tv，自动添加 CORS)
        .route("/bgm/{*path}", any(bangumi_proxy_handler))
        .layer(cors);

    // 启动服务器
    let port = std::env::var("PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(3000);
    let addr = SocketAddr::from(([0, 0, 0, 0], port));

    info!("🚀 动漫聚搜 API 启动在 http://{}", addr);
    info!("📚 已加载 {} 个规则", get_builtin_rules().len());

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

/// GET / - 最小前端页面
async fn index_handler() -> Html<&'static str> {
    Html(INDEX_HTML)
}

/// GET /api - API 信息
async fn api_info_handler() -> impl IntoResponse {
    Json(json!({
        "name": "AnimeSearch API",
        "version": "0.3.0",
        "description": "在线动漫聚合搜索后端",
        "endpoints": {
            "core": {
                "GET /": "搜索页面",
                "POST /api": "搜索动漫 (FormData: anime=关键词, rules=规则名1,规则名2)",
                "GET /rules": "获取所有规则列表",
                "GET /update": "从 KazumiRules 更新规则",
                "GET /health": "健康检查"
            },
            "bangumi_proxy": {
                "ANY /bgm/*": "Bangumi API 通用代理 (透传到 api.bgm.tv，自动添加 CORS)",
                "example": "GET /bgm/v0/subjects/328609 → https://api.bgm.tv/v0/subjects/328609"
            }
        },
        "auth": {
            "note": "Bangumi API 需要认证的端点请在请求头添加 Authorization: Bearer <token>",
            "get_token": "https://next.bgm.tv/demo/access-token"
        }
    }))
}

/// POST / - 动漫搜索处理器 (SSE 流式响应)
async fn search_handler(mut multipart: Multipart) -> Response {
    // 解析 FormData
    let mut keyword: Option<String> = None;
    let mut rule_names: Option<String> = None;
    let mut fetch_episodes = false;

    while let Ok(Some(field)) = multipart.next_field().await {
        match field.name() {
            Some("anime") => {
                if let Ok(text) = field.text().await {
                    keyword = Some(text.trim().to_string());
                }
            }
            Some("rules") => {
                if let Ok(text) = field.text().await {
                    rule_names = Some(text.trim().to_string());
                }
            }
            Some("episodes") => {
                if let Ok(text) = field.text().await {
                    fetch_episodes = text.trim() == "1" || text.trim().to_lowercase() == "true";
                }
            }
            _ => {}
        }
    }

    let keyword = match keyword {
        Some(k) if !k.is_empty() => k,
        _ => {
            return (
                StatusCode::BAD_REQUEST,
                [(header::CONTENT_TYPE, "application/json")],
                Json(json!({"error": "Anime name is required"})),
            )
                .into_response();
        }
    };

    // 筛选规则
    let all_rules = get_builtin_rules();
    let selected_rules: Vec<_> = match rule_names {
        Some(names) if !names.is_empty() => {
            let name_list: Vec<&str> = names.split(',').map(|s| s.trim()).collect();
            all_rules
                .into_iter()
                .filter(|r| name_list.contains(&r.name.as_str()))
                .collect()
        }
        _ => {
            // 如果没有指定规则，返回错误
            return (
                StatusCode::BAD_REQUEST,
                [(header::CONTENT_TYPE, "application/json")],
                Json(json!({"error": "Rules are required. Use 'rules' field to specify rule names (comma separated)"})),
            )
                .into_response();
        }
    };

    if selected_rules.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            [(header::CONTENT_TYPE, "application/json")],
            Json(json!({"error": "No matching rules found"})),
        )
            .into_response();
    }

    info!(
        "🔍 搜索: {} (规则: {}, 获取集数: {})",
        keyword,
        selected_rules
            .iter()
            .map(|r| r.name.as_str())
            .collect::<Vec<_>>()
            .join(", "),
        fetch_episodes
    );

    // 创建 SSE 流
    let stream = search_stream_with_rules_options(keyword, selected_rules, fetch_episodes);

    // 将流转换为字节流
    let body = Body::from_stream(stream.map(|s| Ok::<_, std::convert::Infallible>(s)));

    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "text/event-stream; charset=utf-8")
        .header(header::CACHE_CONTROL, "no-cache")
        .header(header::CONNECTION, "keep-alive")
        .header(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
        .body(body)
        .unwrap()
}

/// 获取规则列表
async fn rules_handler() -> impl IntoResponse {
    let rules = get_builtin_rules();
    let rule_info: Vec<_> = rules
        .iter()
        .map(|r| {
            json!({
                "name": r.name,
                "version": r.version,
                "baseUrl": r.base_url,
                "color": r.color,
                "tags": r.tags,
                "magic": r.magic
            })
        })
        .collect();

    Json(rule_info)
}

/// 健康检查
async fn health_handler() -> impl IntoResponse {
    Json(json!({
        "status": "ok",
        "timestamp": chrono::Utc::now().to_rfc3339()
    }))
}

/// GET /update - 从 KazumiRules 更新规则
async fn update_handler() -> impl IntoResponse {
    info!("📡 手动触发规则更新...");
    let result = updater::update_rules().await;
    Json(json!({
        "success": true,
        "total": result.total,
        "added": result.added,
        "updated": result.updated,
        "failed": result.failed,
        "details": result.details
    }))
}

// ============================================================================
// Bangumi API 通用代理
// ============================================================================

const BANGUMI_API_BASE: &str = "https://api.bgm.tv";
const BANGUMI_USER_AGENT: &str = "kirito/anime-search (https://github.com/AdingApkgg/anime-search-api)";

/// 通用 Bangumi API 代理
/// 将 /bgm/* 的请求透传到 api.bgm.tv/*，自动添加 CORS 头
async fn bangumi_proxy_handler(
    Path(path): Path<String>,
    headers: HeaderMap,
    req: Request,
) -> Response {
    use http_client::HTTP_CLIENT;
    
    // 构建目标 URL
    let query = req.uri().query().map(|q| format!("?{}", q)).unwrap_or_default();
    let target_url = format!("{}/{}{}", BANGUMI_API_BASE, path, query);
    
    // 构建请求
    let method = req.method().clone();
    let mut request_builder = HTTP_CLIENT.request(method.clone(), &target_url)
        .header("User-Agent", BANGUMI_USER_AGENT);
    
    // 转发 Authorization 头
    if let Some(auth) = headers.get("Authorization") {
        if let Ok(auth_str) = auth.to_str() {
            request_builder = request_builder.header("Authorization", auth_str);
    }
}

    // 转发 Content-Type 头
    if let Some(ct) = headers.get("Content-Type") {
        if let Ok(ct_str) = ct.to_str() {
            request_builder = request_builder.header("Content-Type", ct_str);
    }
}

    // 如果有 body，转发 body
    let body_bytes = match axum::body::to_bytes(req.into_body(), 10 * 1024 * 1024).await {
        Ok(bytes) => bytes,
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({"error": format!("Failed to read request body: {}", e)})),
            ).into_response();
        }
    };

    if !body_bytes.is_empty() {
        request_builder = request_builder.body(body_bytes.to_vec());
    }
    
    // 发送请求
    let response = match request_builder.send().await {
        Ok(resp) => resp,
        Err(e) => {
            return (
                StatusCode::BAD_GATEWAY,
                Json(json!({"error": format!("Proxy request failed: {}", e)})),
            ).into_response();
        }
    };

    // 构建响应
    let status = StatusCode::from_u16(response.status().as_u16()).unwrap_or(StatusCode::OK);
    let content_type = response
        .headers()
        .get("Content-Type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("application/json")
        .to_string();
    
    let response_body = match response.bytes().await {
        Ok(bytes) => bytes,
        Err(e) => {
            return (
                StatusCode::BAD_GATEWAY,
                Json(json!({"error": format!("Failed to read response: {}", e)})),
            ).into_response();
    }
    };
    
    Response::builder()
        .status(status)
        .header(header::CONTENT_TYPE, content_type)
        .header(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
        .header(header::ACCESS_CONTROL_ALLOW_METHODS, "GET, POST, PUT, PATCH, DELETE, OPTIONS")
        .header(header::ACCESS_CONTROL_ALLOW_HEADERS, "Content-Type, Authorization")
        .body(Body::from(response_body.to_vec()))
        .unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())
}

/// 最小前端 HTML
/// 内嵌前端 HTML (编译时从 static/index.html 读取)
const INDEX_HTML: &str = include_str!("../static/index.html");
