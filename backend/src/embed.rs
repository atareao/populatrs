use std::path::PathBuf;

use axum::{
    body::Body,
    http::StatusCode,
    response::{IntoResponse, Response},
};

/// Possible locations for the `dist` directory (frontend build output).
/// Checked in order — first match wins.
fn dist_dir() -> PathBuf {
    let candidates = [
        "dist",             // Docker / project root
        "frontend/dist",    // Running from project root
        "../frontend/dist", // Running from backend/
    ];
    for candidate in &candidates {
        let path = PathBuf::from(candidate);
        if path.join("index.html").exists() {
            return path;
        }
    }
    // Fallback: use the default location
    PathBuf::from("dist")
}

/// Serve static files from the frontend build directory.
/// Falls back to `index.html` for SPA routing.
pub async fn serve_embedded(path: &str) -> Response {
    let base = dist_dir();
    let clean_path = path.trim_start_matches('/');

    let file_path = if clean_path.is_empty() || clean_path.starts_with("api/") {
        "index.html"
    } else {
        clean_path
    };

    let full_path = base.join(file_path);

    match tokio::fs::read(&full_path).await {
        Ok(content) => {
            let ext = file_path.rsplit('.').next().unwrap_or("");
            let mime = match ext {
                "html" => "text/html; charset=utf-8",
                "css" => "text/css",
                "js" => "application/javascript",
                "json" => "application/json",
                "png" => "image/png",
                "svg" => "image/svg+xml",
                "ico" => "image/x-icon",
                "woff2" => "font/woff2",
                "woff" => "font/woff",
                "ttf" => "font/ttf",
                _ => "application/octet-stream",
            };
            Response::builder()
                .status(StatusCode::OK)
                .header("content-type", mime)
                .header("cache-control", "public, max-age=3600")
                .body(Body::from(content))
                .unwrap_or_else(|_| (StatusCode::NOT_FOUND, "Not Found").into_response())
        }
        Err(_) => {
            // Final SPA fallback — serve index.html
            let fallback = base.join("index.html");
            match tokio::fs::read(&fallback).await {
                Ok(content) => Response::builder()
                    .status(StatusCode::OK)
                    .header("content-type", "text/html; charset=utf-8")
                    .body(Body::from(content))
                    .unwrap_or_else(|_| (StatusCode::NOT_FOUND, "Not Found").into_response()),
                Err(_) => (
                    StatusCode::NOT_FOUND,
                    "Frontend not built. Run `cd frontend && pnpm install && pnpm build` first.",
                )
                    .into_response(),
            }
        }
    }
}
