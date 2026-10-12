//! Serves the viewer SPA embedded in the binary.
//!
//! Debug builds read from disk instead of embedding, so rebuilding the viewer takes effect without
//! a restart.

use axum::{
    http::{StatusCode, header},
    response::{IntoResponse, Response},
};
use rust_embed::RustEmbed;

#[derive(RustEmbed)]
#[folder = "$CONCEPTORY_VIEWER_DIST"]
#[allow_missing = true]
struct Viewer;

/// Page returned when the viewer build output is not embedded
const MISSING: &str = "<!doctype html><meta charset=\"utf-8\"><title>conceptory</title><p>viewer \
                       の成果物が埋め込まれていません。viewer をビルドしてから conceptory \
                       をビルドし直してください。</p>";

/// Returns the static file at `path`.
///
/// Screens are switched by the query on `/`, so only `/` returns `index.html`. Returning
/// `index.html` for a missing file would hand HTML to whatever tried to load a script and obscure
/// why it failed, so this returns 404 instead.
pub fn serve(path: &str) -> Response {
    let path = match path.trim_start_matches('/') {
        "" => "index.html",
        path => path,
    };
    let Some(file) = Viewer::get(path) else {
        return if path == "index.html" {
            (
                StatusCode::SERVICE_UNAVAILABLE,
                [(header::CONTENT_TYPE, "text/html; charset=utf-8")],
                MISSING,
            )
                .into_response()
        } else {
            StatusCode::NOT_FOUND.into_response()
        };
    };
    // Files emitted with hashed names never change, so let the browser cache them
    let cache = if path.starts_with("assets/") {
        "public, max-age=31536000, immutable"
    } else {
        "no-store"
    };
    (
        [
            (header::CONTENT_TYPE, file.metadata.mimetype().to_owned()),
            (header::CACHE_CONTROL, cache.to_owned()),
        ],
        file.data.into_owned(),
    )
        .into_response()
}
