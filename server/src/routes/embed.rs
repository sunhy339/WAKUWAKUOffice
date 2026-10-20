use axum::body::Body;
use axum::http::header;
use axum::http::{StatusCode, Uri};
use axum::response::{IntoResponse, Response};
use rust_embed::RustEmbed;

#[derive(RustEmbed)]
#[folder = "../frontend/dist/"]
struct WebAsset;

/// SPA fallback handler：匹配所有非 /api 路由
pub async fn fallback_handler(uri: Uri) -> Response {
    let path = uri.path().trim_start_matches('/');

    // 尝试精确匹配静态文件
    if !path.is_empty() {
        if let Some(asset) = WebAsset::get(path) {
            return serve_asset(path, asset);
        }
    }

    // SPA fallback → index.html
    if let Some(index) = WebAsset::get("index.html") {
        return serve_asset("index.html", index);
    }

    // 前端未构建时返回 API 信息
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            serde_json::json!({
                "name": "WaKuOffice",
                "version": "0.2.0",
                "message": "前端未构建。请先运行 npm run build 构建 frontend/dist/，或访问 /api/health",
                "docs": "/api/health",
            })
            .to_string(),
        ))
        .unwrap()
}

fn serve_asset(path: &str, asset: rust_embed::EmbeddedFile) -> Response {
    let mime = mime_guess::from_path(path).first_or_octet_stream();
    // index.html 必须及时获取最新的资源清单；带 hash 的构建产物可以长期缓存。
    // 如果入口页也缓存，发布新版本后旧入口可能继续引用已不存在的动态 chunk，
    // 页面会在路由切换或懒加载时出现空白。
    let cache_control = if path == "index.html" {
        "no-cache, no-store, must-revalidate"
    } else {
        "public, max-age=31536000, immutable"
    };
    let body = Body::from(asset.data.into_owned());
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, mime.as_ref())
        .header(header::CACHE_CONTROL, cache_control)
        .body(body)
        .unwrap()
}
