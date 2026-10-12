//! Provides the API and static files to the viewer.
//!
//! The server is only meant to be opened locally, so it listens on `127.0.0.1` and rejects
//! requests whose Host or Origin does not point to itself. Even if an external page reaches this
//! server through DNS rebinding, it cannot read the responses because the Host does not match.

use std::{
    convert::Infallible,
    fs,
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};

use axum::{
    Json, Router,
    extract::{Query, Request, State as Extract},
    http::{HeaderMap, StatusCode, Uri, header},
    middleware::{self, Next},
    response::{
        IntoResponse, Response,
        sse::{Event as SseEvent, KeepAlive, Sse},
    },
    routing::get,
};
use serde::Deserialize;
use serde_json::json;
use tokio_stream::{Stream, StreamExt as _, wrappers::BroadcastStream};

use crate::{assets, snapshot::State};

/// Hosts and origins from which requests are accepted
#[derive(Clone)]
struct Guard {
    hosts: Vec<String>,
    origins: Vec<String>,
}

/// Builds the server routes.
pub fn router(state: Arc<State>, port: u16, allowed_origins: Vec<String>) -> Router {
    let hosts = vec![format!("127.0.0.1:{port}"), format!("localhost:{port}")];
    let mut origins: Vec<String> = hosts.iter().map(|host| format!("http://{host}")).collect();
    origins.extend(allowed_origins);
    let guard = Guard { hosts, origins };

    Router::new()
        .route("/api/overview", get(overview))
        .route("/api/document", get(document))
        .route("/api/events", get(events))
        .route("/files", get(file))
        .fallback(get(static_file))
        .with_state(state)
        .layer(middleware::from_fn_with_state(guard, check_origin))
}

/// Lets through only requests whose Host and Origin point to the server itself.
async fn check_origin(Extract(guard): Extract<Guard>, request: Request, next: Next) -> Response {
    let headers = request.headers();
    let host_allowed = header_value(headers, header::HOST)
        .is_some_and(|host| guard.hosts.iter().any(|allowed| allowed == host));
    let origin_allowed = header_value(headers, header::ORIGIN)
        .is_none_or(|origin| guard.origins.iter().any(|allowed| allowed == origin));
    if host_allowed && origin_allowed {
        next.run(request).await
    } else {
        StatusCode::FORBIDDEN.into_response()
    }
}

fn header_value(headers: &HeaderMap, name: header::HeaderName) -> Option<&str> {
    headers.get(name).and_then(|value| value.to_str().ok())
}

/// Returns the document list, graph, terms and diagnostics.
async fn overview(Extract(state): Extract<Arc<State>>) -> Response {
    let snapshot = state.snapshot();
    Json(json!({
        "serverId": state.server_id,
        "revision": snapshot.revision,
        "overview": snapshot.view.overview,
    }))
    .into_response()
}

#[derive(Deserialize)]
struct DocumentQuery {
    id: String,
}

/// Returns the display data for one document. Identifiers may contain `.` as in `./docs/a.md`,
/// which the browser would resolve if they were in the URL path, so they come in the query.
async fn document(
    Extract(state): Extract<Arc<State>>,
    Query(query): Query<DocumentQuery>,
) -> Response {
    let snapshot = state.snapshot();
    snapshot.view.details.get(&query.id).map_or_else(
        || StatusCode::NOT_FOUND.into_response(),
        |detail| Json(detail).into_response(),
    )
}

/// Sends change notifications as Server-Sent Events. Right after connecting, sends the server ID
/// and revision.
async fn events(
    Extract(state): Extract<Arc<State>>,
) -> Sse<impl Stream<Item = Result<SseEvent, Infallible>>> {
    let hello = SseEvent::default()
        .event("hello")
        .json_data(json!({
            "serverId": state.server_id,
            "revision": state.snapshot().revision,
        }))
        .unwrap_or_default();
    let updates = BroadcastStream::new(state.subscribe()).map(|received| {
        // If notifications piled up and some were dropped, make the viewer refetch everything
        let event = received.map_or_else(
            |_| SseEvent::default().event("resync").data("{}"),
            |event| {
                SseEvent::default()
                    .event("update")
                    .json_data(event)
                    .unwrap_or_default()
            },
        );
        Ok(event)
    });
    Sse::new(tokio_stream::once(Ok(hello)).chain(updates))
        .keep_alive(KeepAlive::new().interval(Duration::from_secs(15)))
}

#[derive(Deserialize)]
struct FileQuery {
    path: String,
}

/// Returns files such as images referenced by documents. Limited to files under the loaded OKF
/// bundles.
async fn file(Extract(state): Extract<Arc<State>>, Query(query): Query<FileQuery>) -> Response {
    let Some(target) = resolve_file(state.roots(), Path::new(&query.path)) else {
        return StatusCode::NOT_FOUND.into_response();
    };
    fs::read(&target).map_or_else(
        |_| StatusCode::NOT_FOUND.into_response(),
        |data| {
            (
                [
                    (header::CONTENT_TYPE, content_type(&target)),
                    // Prevent embedded scripts from running
                    (header::CONTENT_SECURITY_POLICY, "sandbox"),
                    (header::X_CONTENT_TYPE_OPTIONS, "nosniff"),
                ],
                data,
            )
                .into_response()
        },
    )
}

/// Resolves a display path to the absolute path of an existing file under an OKF bundle.
fn resolve_file(roots: &[PathBuf], path: &Path) -> Option<PathBuf> {
    roots.iter().find_map(|root| {
        let rel = path.strip_prefix(root).ok()?;
        let root = root.canonicalize().ok()?;
        let target = root.join(rel).canonicalize().ok()?;
        (target.starts_with(&root) && target.is_file()).then_some(target)
    })
}

fn content_type(path: &Path) -> &'static str {
    match path
        .extension()
        .and_then(|extension| extension.to_str())
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("png") => "image/png",
        Some("jpg" | "jpeg") => "image/jpeg",
        Some("gif") => "image/gif",
        Some("svg") => "image/svg+xml",
        Some("webp") => "image/webp",
        Some("avif") => "image/avif",
        Some("pdf") => "application/pdf",
        Some("txt" | "md") => "text/plain; charset=utf-8",
        _ => "application/octet-stream",
    }
}

async fn static_file(uri: Uri) -> Response {
    assets::serve(uri.path())
}

#[cfg(test)]
#[allow(
    clippy::missing_panics_doc,
    reason = "tests report failures by panicking"
)]
mod tests {
    use axum::{body::Body, http::Request as HttpRequest};
    use conceptory_okf::Schemas;
    use http_body_util::BodyExt as _;
    use tower::ServiceExt as _;

    use super::*;

    const PORT: u16 = 7878;

    fn root() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../conceptory_okf/tests/fixtures/view")
    }

    fn app() -> Router {
        let schemas = Schemas::load(&[]).expect("can load the built-in schemas");
        let state = State::new(vec![root()], schemas).expect("can load the fixture");
        router(Arc::new(state), PORT, vec![
            "http://localhost:5173".to_owned(),
        ])
    }

    async fn send(uri: &str, headers: &[(&str, &str)]) -> Response {
        let mut request = HttpRequest::builder().uri(uri);
        for (name, value) in headers {
            request = request.header(*name, *value);
        }
        app()
            .oneshot(request.body(Body::empty()).expect("can build the request"))
            .await
            .expect("returns a response")
    }

    async fn body(response: Response) -> String {
        let bytes = response
            .into_body()
            .collect()
            .await
            .expect("can read the body")
            .to_bytes();
        String::from_utf8(bytes.to_vec()).expect("body is UTF-8")
    }

    const HOST: (&str, &str) = ("host", "127.0.0.1:7878");

    #[tokio::test]
    async fn rejects_foreign_host_and_origin() {
        let foreign_host = send("/api/overview", &[("host", "evil.example:7878")]).await;
        assert_eq!(foreign_host.status(), StatusCode::FORBIDDEN);
        let no_host = send("/api/overview", &[]).await;
        assert_eq!(no_host.status(), StatusCode::FORBIDDEN);
        let foreign_origin =
            send("/api/overview", &[HOST, ("origin", "http://evil.example")]).await;
        assert_eq!(foreign_origin.status(), StatusCode::FORBIDDEN);

        let allowed_origin = send("/api/overview", &[
            HOST,
            ("origin", "http://localhost:5173"),
        ])
        .await;
        assert_eq!(allowed_origin.status(), StatusCode::OK);
        let localhost = send("/api/overview", &[("host", "localhost:7878")]).await;
        assert_eq!(localhost.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn serves_overview_and_documents() {
        let overview = send("/api/overview", &[HOST]).await;
        assert_eq!(overview.status(), StatusCode::OK);
        assert!(body(overview).await.contains("\"revision\":0"));

        let id = root().join("notes/b.md").display().to_string();
        let document = send(&format!("/api/document?id={id}"), &[HOST]).await;
        assert_eq!(document.status(), StatusCode::OK);
        assert!(body(document).await.contains("\"title\":\"B\""));

        let missing = send("/api/document?id=missing.md", &[HOST]).await;
        assert_eq!(missing.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn serves_only_files_under_roots() {
        let figure = root().join("notes/figure.svg").display().to_string();
        let file = send(&format!("/files?path={figure}"), &[HOST]).await;
        assert_eq!(file.status(), StatusCode::OK);
        assert_eq!(
            file.headers().get(header::CONTENT_SECURITY_POLICY),
            Some(&"sandbox".parse().expect("is a header value"))
        );

        let outside = root().join("../../okf.rs").display().to_string();
        let traversal = send(&format!("/files?path={outside}"), &[HOST]).await;
        assert_eq!(traversal.status(), StatusCode::NOT_FOUND);
    }
}
