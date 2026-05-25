use std::sync::Arc;

use axum::Json;
use axum::Router;
use axum::body::Bytes;
use axum::extract::{Path as AxumPath, Query, State, WebSocketUpgrade};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use parking_lot::Mutex;
use rust_embed::RustEmbed;
use serde::Deserialize;
use std::collections::HashMap;
use tracing::{error, instrument, warn};

use crate::dto::{IndexStats, ProjectOut};
use crate::index::Index;
use crate::parser::{SessionMeta, Usage, parse_session};
use crate::pty::{spawn_new_chat_bridge, spawn_resume_bridge};

#[derive(RustEmbed)]
#[folder = "static/"]
struct Assets;

#[derive(Default)]
struct SessionCache {
    inner: Mutex<HashMap<String, CachedSession>>,
}

struct CachedSession {
    file_size: u64,
    json: Bytes,
}

impl SessionCache {
    fn get_if_fresh(&self, id: &str, file_size: u64) -> Option<Bytes> {
        let guard = self.inner.lock();
        let cached = guard.get(id)?;
        if cached.file_size == file_size {
            Some(cached.json.clone())
        } else {
            None
        }
    }
    fn insert(&self, id: String, file_size: u64, json: Bytes) {
        self.inner
            .lock()
            .insert(id, CachedSession { file_size, json });
    }
    fn clear(&self) {
        self.inner.lock().clear();
    }
}

#[derive(Clone)]
pub struct AppState {
    pub index: Index,
    cache: Arc<SessionCache>,
}

impl AppState {
    pub fn new(index: Index) -> Self {
        Self {
            index,
            cache: Arc::new(SessionCache::default()),
        }
    }
}

#[derive(Debug, Deserialize, Default)]
pub struct ResumeQuery {
    #[serde(default)]
    pub fork: bool,
}

pub fn build_router(state: AppState) -> Router {
    let ws_routes = Router::new()
        .route("/api/projects/{sanitized_name}/new-chat", get(ws_new_chat))
        .route("/api/sessions/{session_id}/pty", get(ws_resume))
        .layer(middleware::from_fn(reject_non_loopback_origin));

    let api_routes = Router::new()
        .route("/api/stats", get(get_stats))
        .route("/api/reindex", post(post_reindex))
        .route("/api/projects", get(list_projects))
        .route(
            "/api/projects/{sanitized_name}/sessions",
            get(list_project_sessions),
        )
        .route("/api/sessions/{session_id}", get(get_session));

    Router::new()
        .merge(api_routes)
        .merge(ws_routes)
        .route("/", get(serve_index))
        .route("/static/{*path}", get(serve_static))
        .layer(middleware::from_fn(add_security_headers))
        .with_state(state)
}

async fn add_security_headers(req: axum::extract::Request, next: Next) -> Response {
    let mut resp = next.run(req).await;
    let h = resp.headers_mut();

    h.insert(
        header::CONTENT_SECURITY_POLICY,
        HeaderValue::from_static(
            "default-src 'self'; \
             script-src 'self' 'wasm-unsafe-eval'; \
             style-src 'self' 'unsafe-inline' https://fonts.googleapis.com; \
             font-src 'self' https://fonts.gstatic.com; \
             img-src 'self' data:; \
             connect-src 'self' ws://127.0.0.1:* ws://localhost:*; \
             frame-ancestors 'none'; \
             form-action 'none'; \
             base-uri 'self'",
        ),
    );
    h.insert(header::X_FRAME_OPTIONS, HeaderValue::from_static("DENY"));
    h.insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    h.insert(
        header::REFERRER_POLICY,
        HeaderValue::from_static("no-referrer"),
    );
    resp
}

async fn reject_non_loopback_origin(req: axum::extract::Request, next: Next) -> Response {
    if !origin_is_loopback(req.headers()) {
        warn!(
            origin = ?req.headers().get(header::ORIGIN),
            "rejecting WS upgrade: origin not loopback",
        );
        return (StatusCode::FORBIDDEN, "origin not allowed").into_response();
    }
    next.run(req).await
}

fn origin_is_loopback(headers: &HeaderMap) -> bool {
    let Some(origin) = headers.get(header::ORIGIN).and_then(|v| v.to_str().ok()) else {
        return false;
    };
    let Some(parsed) = parse_origin(origin) else {
        return false;
    };
    if parsed.scheme != "http" && parsed.scheme != "https" {
        return false;
    }
    parsed.host == "127.0.0.1" || parsed.host == "localhost" || parsed.host == "[::1]"
}

struct ParsedOrigin<'a> {
    scheme: &'a str,
    host: &'a str,
}

fn parse_origin(s: &str) -> Option<ParsedOrigin<'_>> {
    let (scheme, rest) = s.split_once("://")?;
    let authority = rest.split_once('/').map(|(a, _)| a).unwrap_or(rest);
    if authority.starts_with('[') {
        let end = authority.find(']')?;
        return Some(ParsedOrigin {
            scheme,
            host: &authority[..=end],
        });
    }
    let host = authority
        .rsplit_once(':')
        .map(|(h, _)| h)
        .unwrap_or(authority);
    Some(ParsedOrigin { scheme, host })
}

fn build_index_stats(state: &AppState) -> IndexStats {
    let snap = state.index.read();
    let total_usage = snap.projects.iter().fold(Usage::default(), |mut acc, p| {
        acc.add(&p.total_usage());
        acc
    });
    IndexStats {
        projects: snap.project_count(),
        sessions: snap.session_count(),
        last_scan: snap.last_scan,
        scanning: state.index.is_scanning(),
        total_usage,
    }
}

async fn get_stats(State(s): State<AppState>) -> Json<IndexStats> {
    Json(build_index_stats(&s))
}

#[instrument(skip_all, name = "reindex")]
async fn post_reindex(State(s): State<AppState>) -> Result<Json<IndexStats>, StatusCode> {
    let idx = s.index.clone();
    tokio::task::spawn_blocking(move || idx.rebuild())
        .await
        .map_err(|e| {
            error!(error = %e, "reindex panicked");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    s.cache.clear();
    Ok(Json(build_index_stats(&s)))
}

async fn list_projects(State(s): State<AppState>) -> Json<Vec<ProjectOut>> {
    let snap = s.index.read();
    Json(snap.projects.iter().map(ProjectOut::from).collect())
}

async fn list_project_sessions(
    State(s): State<AppState>,
    AxumPath(name): AxumPath<String>,
) -> Result<Json<Vec<SessionMeta>>, StatusCode> {
    let snap = s.index.read();
    let project = snap.project_by_name(&name).ok_or(StatusCode::NOT_FOUND)?;
    Ok(Json(project.sessions.clone()))
}

#[instrument(skip(s), fields(session_id = %session_id))]
async fn get_session(
    State(s): State<AppState>,
    AxumPath(session_id): AxumPath<String>,
) -> Result<Response, StatusCode> {
    let meta = {
        let snap = s.index.read();
        snap.by_session_id.get(&session_id).cloned()
    };
    let meta = meta.ok_or(StatusCode::NOT_FOUND)?;

    let file_size = match std::fs::metadata(&meta.path) {
        Ok(m) => m.len(),
        Err(_) => return Err(StatusCode::NOT_FOUND),
    };

    let json = if let Some(cached) = s.cache.get_if_fresh(&session_id, file_size) {
        cached
    } else {
        let path = meta.path.clone();
        let session = tokio::task::spawn_blocking(move || parse_session(&path))
            .await
            .map_err(|e| {
                error!(error = %e, "parse_session panicked");
                StatusCode::INTERNAL_SERVER_ERROR
            })?;
        let bytes = Bytes::from(serde_json::to_vec(&session).map_err(|e| {
            error!(error = %e, "serialise session failed");
            StatusCode::INTERNAL_SERVER_ERROR
        })?);
        s.cache.insert(session_id, file_size, bytes.clone());
        bytes
    };
    Ok(([(header::CONTENT_TYPE, "application/json")], json).into_response())
}

async fn ws_resume(
    State(s): State<AppState>,
    AxumPath(session_id): AxumPath<String>,
    Query(q): Query<ResumeQuery>,
    ws: WebSocketUpgrade,
) -> Response {
    let meta = {
        let snap = s.index.read();
        snap.by_session_id.get(&session_id).cloned()
    };
    let Some(meta) = meta else {
        return (StatusCode::NOT_FOUND, "session not found").into_response();
    };
    ws.on_upgrade(move |socket| async move {
        spawn_resume_bridge(socket, session_id, meta.cwd, q.fork).await;
    })
}

async fn ws_new_chat(
    State(s): State<AppState>,
    AxumPath(name): AxumPath<String>,
    ws: WebSocketUpgrade,
) -> Response {
    let cwd = {
        let snap = s.index.read();
        snap.project_by_name(&name).map(|p| p.cwd.clone())
    };
    let Some(cwd) = cwd else {
        return (StatusCode::NOT_FOUND, "project not found").into_response();
    };
    ws.on_upgrade(move |socket| async move {
        spawn_new_chat_bridge(socket, cwd).await;
    })
}

async fn serve_index() -> Response {
    serve_asset("index.html").await
}

async fn serve_static(AxumPath(path): AxumPath<String>) -> Response {
    serve_asset(&path).await
}

async fn serve_asset(path: &str) -> Response {
    match Assets::get(path) {
        Some(file) => {
            let mime = mime_guess::from_path(path).first_or_octet_stream();
            (
                [(header::CONTENT_TYPE, mime.essence_str().to_owned())],
                file.data,
            )
                .into_response()
        }
        None => (StatusCode::NOT_FOUND, "not found").into_response(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderValue;

    fn hdrs(origin: &str) -> HeaderMap {
        let mut h = HeaderMap::new();
        h.insert(header::ORIGIN, HeaderValue::from_str(origin).unwrap());
        h
    }

    #[test]
    fn origin_check_accepts_loopback_ipv4() {
        assert!(origin_is_loopback(&hdrs("http://127.0.0.1:6767")));
        assert!(origin_is_loopback(&hdrs("http://127.0.0.1")));
    }

    #[test]
    fn origin_check_accepts_localhost() {
        assert!(origin_is_loopback(&hdrs("http://localhost:6767")));
    }

    #[test]
    fn origin_check_accepts_ipv6_loopback() {
        assert!(origin_is_loopback(&hdrs("http://[::1]:6767")));
    }

    #[test]
    fn origin_check_rejects_missing_origin() {
        assert!(!origin_is_loopback(&HeaderMap::new()));
    }

    #[test]
    fn origin_check_rejects_external_host() {
        assert!(!origin_is_loopback(&hdrs("http://evil.example.com:6767")));
    }

    #[test]
    fn origin_check_rejects_non_http_scheme() {
        assert!(!origin_is_loopback(&hdrs("file://127.0.0.1:6767")));
    }

    #[test]
    fn origin_check_rejects_null_origin() {
        assert!(!origin_is_loopback(&hdrs("null")));
    }
}
