use axum::{
    body::Body,
    http::StatusCode,
    response::{IntoResponse, Response},
};

/// Serve static files from the `./dist` directory (not embedded).
/// Falls back to `index.html` for SPA routing.
pub async fn serve_embedded(path: &str) -> Response {
    let clean_path = path.trim_start_matches('/');

    let file_path = if clean_path.is_empty() || clean_path.starts_with("api/") {
        "index.html"
    } else {
        clean_path
    };

    let full_path = format!("dist/{}", file_path);

    match tokio::fs::read(&full_path).await {
        Ok(content) => {
            let ext = file_path.rsplit('.').next_back().unwrap_or("");
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
            match tokio::fs::read("dist/index.html").await {
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