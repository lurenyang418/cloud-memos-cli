use std::time::Duration;

use cloud_memos_cli::{
    ApiClient, App, MemoFilters,
    app::{AccessMode, UiMode},
    model::{MemoState, MemoVisibility},
};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use serde_json::{Value, json};
use url::Url;
use wiremock::{
    Mock, MockServer, ResponseTemplate,
    matchers::{body_json, header, method, path, query_param},
};

const TOKEN: &str = "cm_pat_fixture_secret";

fn client(server: &MockServer) -> ApiClient {
    ApiClient::new(
        Url::parse(&server.uri()).expect("mock URL"),
        TOKEN.to_owned(),
    )
    .expect("API client")
}

fn session_json() -> Value {
    json!({
        "viewer": {
            "id": "user-1",
            "name": "测试用户",
            "email": "redacted@example.invalid",
            "username": "tester",
            "role": "USER",
            "status": "ACTIVE"
        },
        "setupRequired": false,
        "appName": "Cloud Memos",
        "publicContact": null
    })
}

fn memo_json(content: &str, version: u64) -> Value {
    json!({
        "id": "memo-1",
        "content": content,
        "visibility": "PRIVATE",
        "state": "ACTIVE",
        "pinned": true,
        "version": version,
        "createdAt": 1700000000000_u64,
        "updatedAt": 1700000001000_u64,
        "deletedAt": null,
        "author": {
            "id": "user-1",
            "name": "测试用户",
            "username": "tester",
            "image": null
        },
        "tags": ["rust"],
        "attachments": []
    })
}

#[tokio::test]
async fn sends_sensitive_bearer_and_decodes_session() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v1/session"))
        .and(header("authorization", format!("Bearer {TOKEN}")))
        .respond_with(ResponseTemplate::new(200).set_body_json(session_json()))
        .expect(1)
        .mount(&server)
        .await;

    let session = client(&server).session().await.expect("session response");
    assert_eq!(session.viewer.expect("viewer").username, "tester");
    assert_eq!(session.app_name, "Cloud Memos");
}

#[tokio::test]
async fn handles_error_codes_and_redacts_server_echoes() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v1/feed"))
        .respond_with(ResponseTemplate::new(403).set_body_json(json!({
            "error": {
                "code": "INSUFFICIENT_SCOPE",
                "message": format!("do not echo {TOKEN}"),
                "details": {"received": TOKEN}
            }
        })))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/v1/memos"))
        .respond_with(ResponseTemplate::new(401).set_body_string("proxy login page"))
        .mount(&server)
        .await;

    let client = client(&server);
    let scope_error = client
        .list_feed(&MemoFilters::default())
        .await
        .expect_err("scope error");
    assert!(scope_error.is_insufficient_scope());
    assert!(!scope_error.to_string().contains(TOKEN));

    let auth_error = client
        .list_memos(&MemoFilters::default())
        .await
        .expect_err("auth error");
    assert_eq!(auth_error.code(), Some("REQUEST_FAILED"));
    assert!(!auth_error.to_string().contains("proxy login page"));
}

#[tokio::test]
async fn sends_cursor_and_filter_query_parameters() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v1/memos"))
        .and(query_param("cursor", "cursor-two"))
        .and(query_param("limit", "20"))
        .and(query_param("tag", "rust"))
        .and(query_param("visibility", "PUBLIC"))
        .and(query_param("state", "ARCHIVED"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "items": [memo_json("第二页", 3)],
            "nextCursor": "cursor-three"
        })))
        .expect(1)
        .mount(&server)
        .await;

    let page = client(&server)
        .list_memos(&MemoFilters {
            cursor: Some("cursor-two".to_owned()),
            limit: Some(20),
            tag: Some("rust".to_owned()),
            visibility: Some(MemoVisibility::Public),
            state: Some(MemoState::Archived),
            ..MemoFilters::default()
        })
        .await
        .expect("memo page");
    assert_eq!(page.items[0].content, "第二页");
    assert_eq!(page.next_cursor.as_deref(), Some("cursor-three"));
}

#[tokio::test]
async fn enforces_timeout() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v1/session"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_delay(Duration::from_millis(150))
                .set_body_json(session_json()),
        )
        .mount(&server)
        .await;
    let client = ApiClient::with_timeout(
        Url::parse(&server.uri()).expect("mock URL"),
        TOKEN.to_owned(),
        Duration::from_millis(20),
    )
    .expect("API client");

    let error = client.session().await.expect_err("request should time out");
    assert!(error.to_string().contains("网络请求失败"));
    assert!(!error.to_string().contains(TOKEN));
}

#[tokio::test]
async fn blocks_cross_origin_redirect_before_token_leaks() {
    let first = MockServer::start().await;
    let second = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v1/session"))
        .respond_with(
            ResponseTemplate::new(302)
                .insert_header("location", format!("{}/capture", second.uri())),
        )
        .mount(&first)
        .await;
    Mock::given(method("GET"))
        .and(path("/capture"))
        .respond_with(ResponseTemplate::new(200).set_body_json(session_json()))
        .mount(&second)
        .await;

    let error = client(&first)
        .session()
        .await
        .expect_err("cross-origin redirect must be blocked");
    assert!(error.to_string().contains("网络请求失败"));
    assert!(
        second
            .received_requests()
            .await
            .expect("requests")
            .is_empty()
    );
    assert!(!error.to_string().contains(TOKEN));
}

#[tokio::test]
async fn follows_same_origin_redirect() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v1/session"))
        .respond_with(ResponseTemplate::new(307).insert_header("location", "/session-final"))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/session-final"))
        .and(header("authorization", format!("Bearer {TOKEN}")))
        .respond_with(ResponseTemplate::new(200).set_body_json(session_json()))
        .expect(1)
        .mount(&server)
        .await;

    assert_eq!(
        client(&server)
            .session()
            .await
            .expect("same-origin redirect")
            .app_name,
        "Cloud Memos"
    );
}

#[tokio::test]
async fn insufficient_write_scope_downgrades_current_run() {
    let server = MockServer::start().await;
    Mock::given(method("PATCH"))
        .and(path("/api/v1/memos/memo-1"))
        .and(body_json(json!({"pinned": false, "version": 1})))
        .respond_with(ResponseTemplate::new(403).set_body_json(json!({
            "error": {
                "code": "INSUFFICIENT_SCOPE",
                "message": "API 令牌缺少 memos:write 权限"
            }
        })))
        .expect(1)
        .mount(&server)
        .await;
    let mut app = App::new(client(&server), AccessMode::ReadWrite);
    let mut memo: cloud_memos_cli::model::Memo =
        serde_json::from_value(memo_json("原文", 1)).expect("memo fixture");
    memo.pinned = true;
    app.items = vec![memo];

    app.handle_key(KeyEvent::new(KeyCode::Char('p'), KeyModifiers::NONE))
        .await;
    assert_eq!(app.access_mode, AccessMode::ReadOnly);
    assert!(app.status.contains("自动降级为只读"));
}

#[tokio::test]
async fn version_conflict_keeps_draft_and_loads_server_copy() {
    let server = MockServer::start().await;
    Mock::given(method("PATCH"))
        .and(path("/api/v1/memos/memo-1"))
        .and(body_json(json!({"content": "本地草稿", "version": 1})))
        .respond_with(ResponseTemplate::new(409).set_body_json(json!({
            "error": {
                "code": "VERSION_CONFLICT",
                "message": "Memo 已在其他位置更新",
                "details": {"currentVersion": 2}
            }
        })))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/v1/memos/memo-1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(memo_json("服务端新正文", 2)))
        .expect(1)
        .mount(&server)
        .await;
    let mut app = App::new(client(&server), AccessMode::ReadWrite);
    app.items = vec![serde_json::from_value(memo_json("原文", 1)).expect("memo fixture")];

    app.handle_key(KeyEvent::new(KeyCode::Char('e'), KeyModifiers::NONE))
        .await;
    app.replace_editor_content("本地草稿".to_owned());
    app.handle_key(KeyEvent::new(KeyCode::Char('s'), KeyModifiers::CONTROL))
        .await;

    assert_eq!(app.mode, UiMode::Conflict);
    let conflict = app.conflict.expect("conflict state");
    assert_eq!(conflict.local.content, "本地草稿");
    assert_eq!(conflict.server.content, "服务端新正文");
    assert_eq!(conflict.server.version, 2);
}
