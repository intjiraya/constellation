use std::path::{Path, PathBuf};

use axum::body::{Body, to_bytes};
use axum::http::{Method, Request, StatusCode, header};
use serde_json::Value;
use tempfile::TempDir;
use tower::ServiceExt;

use constellation::http::{AppState, build_router};
use constellation::index::Index;

fn fixtures() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/sessions")
}

fn seed(project_dir: &Path, fixture: &str, session_name: &str) {
    std::fs::create_dir_all(project_dir).unwrap();
    std::fs::copy(
        fixtures().join(fixture),
        project_dir.join(format!("{session_name}.jsonl")),
    )
    .unwrap();
}

fn seed_inline(project_dir: &Path, session_id: &str, content_term: &str) {
    std::fs::create_dir_all(project_dir).unwrap();
    let escaped = content_term.replace('"', "\\\"");
    let content = format!(
        "{{\"type\":\"ai-title\",\"aiTitle\":\"test\",\"sessionId\":\"{session_id}\"}}\n\
{{\"type\":\"user\",\"message\":{{\"role\":\"user\",\"content\":\"{escaped}\"}},\
\"uuid\":\"u-1\",\"timestamp\":\"2026-05-25T11:00:00.000Z\",\"sessionId\":\"{session_id}\",\"cwd\":\"/srv/x\"}}\n"
    );
    std::fs::write(project_dir.join(format!("{session_id}.jsonl")), content).unwrap();
}

fn ready_router_with_n_matches(n: usize, term: &str) -> (axum::Router, TempDir) {
    let tmp = TempDir::new().unwrap();
    for i in 0..n {
        seed_inline(
            &tmp.path().join(format!("-proj-{i}")),
            &format!("sess-{i:03}"),
            term,
        );
    }
    let index = Index::new(tmp.path().to_owned());
    index.rebuild();
    let state = AppState::new(index);
    (build_router(state), tmp)
}

fn ready_router() -> (axum::Router, TempDir) {
    let tmp = TempDir::new().unwrap();
    seed(
        &tmp.path().join("-home-test-x"),
        "minimal.jsonl",
        "minimal-uuid",
    );
    seed(
        &tmp.path().join("-home-test-y"),
        "with_tools.jsonl",
        "tools-uuid",
    );
    seed(
        &tmp.path().join("-home-test-x"),
        "with_usage.jsonl",
        "usage-uuid",
    );

    let index = Index::new(tmp.path().to_owned());
    index.rebuild();
    let state = AppState::new(index);
    (build_router(state), tmp)
}

async fn body_to_json(body: Body) -> Value {
    let bytes = to_bytes(body, usize::MAX).await.unwrap();
    serde_json::from_slice(&bytes).unwrap_or(Value::Null)
}

fn req_get(path: &str) -> Request<Body> {
    Request::builder()
        .method(Method::GET)
        .uri(path)
        .header(header::HOST, "127.0.0.1:6767")
        .body(Body::empty())
        .unwrap()
}

fn req_post(path: &str) -> Request<Body> {
    Request::builder()
        .method(Method::POST)
        .uri(path)
        .header(header::HOST, "127.0.0.1:6767")
        .body(Body::empty())
        .unwrap()
}

fn req_get_with_host(path: &str, host: &str) -> Request<Body> {
    Request::builder()
        .method(Method::GET)
        .uri(path)
        .header(header::HOST, host)
        .body(Body::empty())
        .unwrap()
}

fn req_get_with_origin(path: &str, origin: &str) -> Request<Body> {
    Request::builder()
        .method(Method::GET)
        .uri(path)
        .header(header::HOST, "127.0.0.1:6767")
        .header(header::ORIGIN, origin)
        .body(Body::empty())
        .unwrap()
}

#[tokio::test]
async fn get_stats_returns_populated_index_shape() {
    let (app, _tmp) = ready_router();
    let resp = app.oneshot(req_get("/api/stats")).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let v = body_to_json(resp.into_body()).await;
    assert_eq!(v["projects"], 2);
    assert_eq!(v["sessions"], 3);
    assert_eq!(v["scanning"], false);

    for key in ["input", "cache_creation", "cache_read", "output"] {
        assert!(
            v["total_usage"].get(key).is_some(),
            "missing total_usage.{key}"
        );
    }

    assert_eq!(v["total_usage"]["input"], 15);
    assert_eq!(v["total_usage"]["output"], 300);
}

#[tokio::test]
async fn api_search_rejects_non_loopback_host() {
    let (app, _tmp) = ready_router();
    let resp = app
        .oneshot(req_get_with_host(
            "/api/search?q=readme",
            "evil.example.com",
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn api_stats_rejects_non_loopback_host() {
    let (app, _tmp) = ready_router();
    let resp = app
        .oneshot(req_get_with_host("/api/stats", "attacker.example.com"))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn api_rejects_external_origin_even_when_host_is_loopback() {
    let (app, _tmp) = ready_router();
    let resp = app
        .oneshot(req_get_with_origin(
            "/api/stats",
            "https://evil.example.com",
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn api_accepts_loopback_host_with_no_origin() {
    let (app, _tmp) = ready_router();
    let resp = app.oneshot(req_get("/api/stats")).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}

#[tokio::test]
async fn api_accepts_loopback_host_with_loopback_origin() {
    let (app, _tmp) = ready_router();
    let resp = app
        .oneshot(req_get_with_origin("/api/stats", "http://127.0.0.1:6767"))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}

#[tokio::test]
async fn get_search_finds_term_inside_session_body() {
    let (app, _tmp) = ready_router();
    let resp = app.oneshot(req_get("/api/search?q=readme")).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let v = body_to_json(resp.into_body()).await;
    let arr = v.as_array().expect("search returns array");
    assert!(!arr.is_empty(), "expected at least one hit");
    assert_eq!(arr[0]["session_id"], "tools-uuid");
    let snippets = arr[0]["snippets"].as_array().expect("snippets array");
    assert!(!snippets.is_empty());
    let first = &snippets[0];
    assert!(first["text"].as_str().is_some());
    assert!(first["matches"].as_array().is_some());
}

#[tokio::test]
async fn get_search_empty_query_returns_empty() {
    let (app, _tmp) = ready_router();
    let resp = app.oneshot(req_get("/api/search?q=")).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let v = body_to_json(resp.into_body()).await;
    assert_eq!(v.as_array().unwrap().len(), 0);
}

#[tokio::test]
async fn get_search_clamps_limit_max_actually_caps_results() {
    let (app, _tmp) = ready_router_with_n_matches(5, "alpha");
    let resp = app
        .oneshot(req_get("/api/search?q=alpha&limit=2"))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let v = body_to_json(resp.into_body()).await;
    let arr = v.as_array().unwrap();
    assert_eq!(
        arr.len(),
        2,
        "limit=2 should cap at 2 hits despite 5 matching sessions",
    );
}

#[tokio::test]
async fn get_search_clamps_limit_zero_to_min() {
    let (app, _tmp) = ready_router_with_n_matches(3, "alpha");
    let resp = app
        .oneshot(req_get("/api/search?q=alpha&limit=0"))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let v = body_to_json(resp.into_body()).await;
    let arr = v.as_array().unwrap();
    assert_eq!(arr.len(), 1, "limit=0 should clamp up to MIN=1");
}

#[tokio::test]
async fn get_search_quoted_phrase_is_anded_by_server_tokenize() {
    let (app, _tmp) = ready_router_with_n_matches(1, "Authentication module");
    let resp = app
        .oneshot(req_get("/api/search?q=%22Authentication+module%22"))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let v = body_to_json(resp.into_body()).await;
    let arr = v.as_array().unwrap();
    assert!(
        !arr.is_empty(),
        "server should tokenize the quoted phrase into AND'd terms and find the session",
    );
}

#[tokio::test]
async fn get_search_no_q_param_returns_empty_array() {
    let (app, _tmp) = ready_router();
    let resp = app.oneshot(req_get("/api/search")).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let v = body_to_json(resp.into_body()).await;
    assert_eq!(v.as_array().unwrap().len(), 0);
}

#[tokio::test]
async fn post_reindex_returns_fresh_stats_with_scanning_false() {
    let (app, _tmp) = ready_router();
    let resp = app.oneshot(req_post("/api/reindex")).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let v = body_to_json(resp.into_body()).await;
    assert_eq!(v["scanning"], false);
    assert_eq!(v["sessions"], 3);
}

#[tokio::test]
async fn list_projects_returns_correct_shape() {
    let (app, _tmp) = ready_router();
    let resp = app.oneshot(req_get("/api/projects")).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let v = body_to_json(resp.into_body()).await;
    let arr = v.as_array().unwrap();
    assert_eq!(arr.len(), 2);
    for project in arr {
        for key in [
            "sanitized_name",
            "cwd",
            "display_path",
            "session_count",
            "total_messages",
            "total_tools",
            "total_usage",
            "latest_at",
        ] {
            assert!(project.get(key).is_some(), "missing project.{key}");
        }
    }
}

#[tokio::test]
async fn list_project_sessions_404_for_unknown_name() {
    let (app, _tmp) = ready_router();
    let resp = app
        .oneshot(req_get("/api/projects/-nope/sessions"))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn list_project_sessions_200_for_known_name() {
    let (app, _tmp) = ready_router();
    let resp = app
        .oneshot(req_get("/api/projects/-home-test-x/sessions"))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let v = body_to_json(resp.into_body()).await;
    let arr = v.as_array().unwrap();
    assert_eq!(arr.len(), 2);

    let first_id = arr[0]["id"].as_str().unwrap();
    assert!(first_id == "minimal-uuid" || first_id == "usage-uuid");
}

#[tokio::test]
async fn get_session_404_for_unknown_id() {
    let (app, _tmp) = ready_router();
    let resp = app
        .oneshot(req_get("/api/sessions/this-does-not-exist"))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn get_session_200_returns_typed_blocks() {
    let (app, _tmp) = ready_router();
    let resp = app
        .oneshot(req_get("/api/sessions/minimal-uuid"))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let v = body_to_json(resp.into_body()).await;

    assert_eq!(v["id"], "minimal-uuid");
    assert!(v["turns"].is_array());
    let first_turn = &v["turns"][0];
    assert_eq!(first_turn["role"], "user");
    let block = &first_turn["blocks"][0];
    assert_eq!(block["kind"], "text");
    assert!(block["text"].as_str().unwrap().contains("JSONL"));
}

#[tokio::test]
async fn ws_resume_rejects_missing_origin_with_403() {
    let (app, _tmp) = ready_router();
    let req = Request::builder()
        .method(Method::GET)
        .uri("/api/sessions/minimal-uuid/pty")
        .header(header::UPGRADE, "websocket")
        .header(header::CONNECTION, "Upgrade")
        .header(header::SEC_WEBSOCKET_VERSION, "13")
        .header(header::SEC_WEBSOCKET_KEY, "dGhlIHNhbXBsZSBub25jZQ==")
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn ws_resume_rejects_cross_origin_with_403() {
    let (app, _tmp) = ready_router();
    let req = Request::builder()
        .method(Method::GET)
        .uri("/api/sessions/minimal-uuid/pty")
        .header(header::UPGRADE, "websocket")
        .header(header::CONNECTION, "Upgrade")
        .header(header::SEC_WEBSOCKET_VERSION, "13")
        .header(header::SEC_WEBSOCKET_KEY, "dGhlIHNhbXBsZSBub25jZQ==")
        .header(header::ORIGIN, "https://evil.example.com")
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn ws_resume_unknown_session_does_not_200() {
    let (app, _tmp) = ready_router();
    let req = Request::builder()
        .method(Method::GET)
        .uri("/api/sessions/nope/pty")
        .header(header::UPGRADE, "websocket")
        .header(header::CONNECTION, "Upgrade")
        .header(header::SEC_WEBSOCKET_VERSION, "13")
        .header(header::SEC_WEBSOCKET_KEY, "dGhlIHNhbXBsZSBub25jZQ==")
        .header(header::ORIGIN, "http://127.0.0.1:6767")
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert!(!resp.status().is_success());
}

#[tokio::test]
async fn ws_new_chat_unknown_project_does_not_200() {
    let (app, _tmp) = ready_router();
    let req = Request::builder()
        .method(Method::GET)
        .uri("/api/projects/-nope/new-chat")
        .header(header::UPGRADE, "websocket")
        .header(header::CONNECTION, "Upgrade")
        .header(header::SEC_WEBSOCKET_VERSION, "13")
        .header(header::SEC_WEBSOCKET_KEY, "dGhlIHNhbXBsZSBub25jZQ==")
        .header(header::ORIGIN, "http://localhost:6767")
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert!(!resp.status().is_success());
}

#[tokio::test]
async fn security_headers_are_set_on_every_response() {
    let (app, _tmp) = ready_router();
    let resp = app.oneshot(req_get("/api/stats")).await.unwrap();
    let h = resp.headers();
    assert!(h.get(header::CONTENT_SECURITY_POLICY).is_some());
    assert_eq!(h.get(header::X_FRAME_OPTIONS).unwrap(), "DENY");
    assert_eq!(h.get(header::X_CONTENT_TYPE_OPTIONS).unwrap(), "nosniff");
    assert_eq!(h.get(header::REFERRER_POLICY).unwrap(), "no-referrer");
}

#[tokio::test]
async fn root_serves_index_html() {
    let (app, _tmp) = ready_router();
    let resp = app.oneshot(req_get("/")).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let ct = resp.headers().get(header::CONTENT_TYPE).unwrap();
    assert!(ct.to_str().unwrap().starts_with("text/html"));
}

#[tokio::test]
async fn static_unknown_path_404() {
    let (app, _tmp) = ready_router();
    let resp = app
        .oneshot(req_get("/static/nonexistent.js"))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn static_known_vendor_asset_served() {
    let (app, _tmp) = ready_router();
    let resp = app
        .oneshot(req_get("/static/vendor/purify.min.js"))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let ct = resp.headers().get(header::CONTENT_TYPE).unwrap();
    assert!(ct.to_str().unwrap().contains("javascript"));
}
