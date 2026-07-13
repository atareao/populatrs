use axum::{
    body::Body,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use rust_embed::RustEmbed;

#[derive(RustEmbed)]
#[folder = "../frontend/dist"]
struct Assets;

/// Serve an embedded file, falling back to index.html for SPA routing.
pub fn serve_embedded(path: &str) -> Response {
    let clean_path = path.trim_start_matches('/');

    // Try exact path first
    if let Some(file) = Assets::get(clean_path) {
        let mime = mime_guess::from_path(clean_path).first_or_octet_stream();
        return Response::builder()
            .header("content-type", mime.as_ref())
            .header("cache-control", "public, max-age=3600")
            .body(Body::from(file.data))
            .unwrap_or_else(|_| (StatusCode::NOT_FOUND, "Not Found").into_response());
    }

    // SPA routing: serve index.html for non-file paths
    let index_path = if clean_path.is_empty() || !clean_path.contains('.') {
        "index.html"
    } else {
        clean_path
    };

    if let Some(file) = Assets::get(index_path) {
        let mime = mime_guess::from_path(index_path).first_or_octet_stream();
        return Response::builder()
            .header("content-type", mime.as_ref())
            .body(Body::from(file.data))
            .unwrap_or_else(|_| (StatusCode::NOT_FOUND, "Not Found").into_response());
    }

    // Final SPA fallback
    if let Some(file) = Assets::get("index.html") {
        return Response::builder()
            .header("content-type", "text/html; charset=utf-8")
            .body(Body::from(file.data))
            .unwrap_or_else(|_| (StatusCode::NOT_FOUND, "Not Found").into_response());
    }

    (
        StatusCode::NOT_FOUND,
        "Frontend not built. Run `cd frontend && npm install && npm run build` first.",
    )
        .into_response()
}