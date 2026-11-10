//! Route-level tests.
//!
//! These exist because every path-parameter route was declared with `<id>`
//! syntax, which the router treats as a literal path segment rather than a
//! parameter. Fetching a document, downloading an asset, opening a share link
//! and starting an OAuth login all returned 404, and `req.param("id")` was
//! always `None`. Nothing in the unit tests exercised the HTTP layer, so the
//! whole API could be unreachable while every test passed.

use salvo::http::StatusCode;
use salvo::prelude::*;
use salvo::test::{ResponseExt, TestClient};
use serde_json::{json, Value};
use sivgo_web::{config::ServerConfig, router::create_router, store::Store};

const BASE: &str = "http://127.0.0.1:3000";

fn service() -> Service {
    let config = ServerConfig {
        jwt_secret: "test-secret-that-is-long-enough-for-validation".to_string(),
        ..ServerConfig::default()
    };
    create_router(Store::new(), &config)
}

/// Register a user and return its bearer token.
async fn register(service: &Service, email: &str) -> String {
    let mut res = TestClient::post(format!("{BASE}/api/v1/auth/register"))
        .json(&json!({
            "email": email,
            "username": "test-user",
            "password": "correct horse battery staple",
        }))
        .send(service)
        .await;
    assert_eq!(
        res.status_code,
        Some(StatusCode::CREATED),
        "register failed"
    );
    let body: Value = res.take_json().await.expect("register response");
    body["token"]
        .as_str()
        .expect("register response must carry a token")
        .to_string()
}

async fn create_document(service: &Service, token: &str, name: &str) -> String {
    let mut res = TestClient::post(format!("{BASE}/api/v1/documents"))
        .add_header("authorization", format!("Bearer {token}"), true)
        .json(&json!({
            "title": name,
            "svg_content": "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"10\" height=\"10\"/>",
        }))
        .send(service)
        .await;
    assert_eq!(res.status_code, Some(StatusCode::CREATED), "create failed");
    let body: Value = res.take_json().await.expect("create response");
    body["id"].as_str().expect("document id").to_string()
}

#[tokio::test]
async fn health_endpoint_responds() {
    let service = service();
    let res = TestClient::get(format!("{BASE}/api/v1/health"))
        .send(&service)
        .await;
    assert_eq!(res.status_code, Some(StatusCode::OK));
}

#[tokio::test]
async fn agent_route_requires_authentication_before_contacting_a_provider() {
    let service = service();
    let res = TestClient::post(format!("{BASE}/api/v1/agent/suggest"))
        .json(&json!({"svg": "<svg/>", "prompt": "Add a rectangle"}))
        .send(&service)
        .await;
    assert_eq!(res.status_code, Some(StatusCode::UNAUTHORIZED));
}

#[tokio::test]
async fn document_id_route_is_reachable() {
    let service = service();
    let token = register(&service, "doc-route@example.com").await;
    let id = create_document(&service, &token, "Doc").await;

    let mut res = TestClient::get(format!("{BASE}/api/v1/documents/{id}"))
        .add_header("authorization", format!("Bearer {token}"), true)
        .send(&service)
        .await;
    assert_eq!(
        res.status_code,
        Some(StatusCode::OK),
        "GET /documents/{{id}} must resolve the id parameter"
    );
    let body: Value = res.take_json().await.expect("document response");
    assert_eq!(body["id"].as_str(), Some(id.as_str()));
    assert_eq!(body["title"].as_str(), Some("Doc"));
}

#[tokio::test]
async fn document_update_and_delete_reach_their_handlers() {
    let service = service();
    let token = register(&service, "doc-mutate@example.com").await;
    let id = create_document(&service, &token, "Before").await;

    let res = TestClient::put(format!("{BASE}/api/v1/documents/{id}"))
        .add_header("authorization", format!("Bearer {token}"), true)
        .json(&json!({ "title": "After" }))
        .send(&service)
        .await;
    assert_eq!(res.status_code, Some(StatusCode::OK), "PUT must be routed");

    let mut res = TestClient::get(format!("{BASE}/api/v1/documents/{id}"))
        .add_header("authorization", format!("Bearer {token}"), true)
        .send(&service)
        .await;
    let body: Value = res.take_json().await.expect("document response");
    assert_eq!(body["title"].as_str(), Some("After"), "update must persist");

    let res = TestClient::delete(format!("{BASE}/api/v1/documents/{id}"))
        .add_header("authorization", format!("Bearer {token}"), true)
        .send(&service)
        .await;
    assert!(
        res.status_code == Some(StatusCode::OK) || res.status_code == Some(StatusCode::NO_CONTENT),
        "DELETE must be routed, got {:?}",
        res.status_code
    );

    let res = TestClient::get(format!("{BASE}/api/v1/documents/{id}"))
        .add_header("authorization", format!("Bearer {token}"), true)
        .send(&service)
        .await;
    assert_eq!(res.status_code, Some(StatusCode::NOT_FOUND));
}

#[tokio::test]
async fn nested_versions_and_shares_routes_are_reachable() {
    let service = service();
    let token = register(&service, "nested@example.com").await;
    let id = create_document(&service, &token, "Nested").await;

    let res = TestClient::get(format!("{BASE}/api/v1/documents/{id}/versions"))
        .add_header("authorization", format!("Bearer {token}"), true)
        .send(&service)
        .await;
    assert_eq!(res.status_code, Some(StatusCode::OK), "versions must route");

    let mut res = TestClient::post(format!("{BASE}/api/v1/documents/{id}/shares"))
        .add_header("authorization", format!("Bearer {token}"), true)
        .json(&json!({ "permission": "View" }))
        .send(&service)
        .await;
    assert!(
        res.status_code == Some(StatusCode::CREATED) || res.status_code == Some(StatusCode::OK),
        "creating a share must route, got {:?}",
        res.status_code
    );
    let share: Value = res.take_json().await.expect("share response");
    let token_value = share["token"].as_str().expect("share token").to_string();

    // The public share link is the other route that used the old syntax.
    let res = TestClient::get(format!("{BASE}/api/v1/shared/{token_value}"))
        .send(&service)
        .await;
    assert_eq!(
        res.status_code,
        Some(StatusCode::OK),
        "public share link must resolve the token parameter"
    );

    let res = TestClient::get(format!("{BASE}/api/v1/documents/{id}/shares"))
        .add_header("authorization", format!("Bearer {token}"), true)
        .send(&service)
        .await;
    assert_eq!(res.status_code, Some(StatusCode::OK), "listing must route");
}

#[tokio::test]
async fn asset_id_route_is_reachable() {
    let service = service();
    let token = register(&service, "asset@example.com").await;

    let mut res = TestClient::post(format!("{BASE}/api/v1/assets"))
        .add_header("authorization", format!("Bearer {token}"), true)
        .json(&json!({
            "filename": "pixel.txt",
            "content_type": "text/plain",
            // "hello" in base64
            "data": "aGVsbG8=",
        }))
        .send(&service)
        .await;
    assert_eq!(res.status_code, Some(StatusCode::CREATED), "upload failed");
    let body: Value = res.take_json().await.expect("upload response");
    let asset_id = body["id"].as_str().expect("asset id").to_string();

    let res = TestClient::get(format!("{BASE}/api/v1/assets/{asset_id}"))
        .add_header("authorization", format!("Bearer {token}"), true)
        .send(&service)
        .await;
    assert_eq!(
        res.status_code,
        Some(StatusCode::OK),
        "asset download must resolve the id parameter"
    );

    let res = TestClient::delete(format!("{BASE}/api/v1/assets/{asset_id}"))
        .add_header("authorization", format!("Bearer {token}"), true)
        .send(&service)
        .await;
    assert!(
        res.status_code == Some(StatusCode::OK) || res.status_code == Some(StatusCode::NO_CONTENT),
        "asset delete must route, got {:?}",
        res.status_code
    );
}

#[tokio::test]
async fn oauth_provider_route_is_reachable() {
    let service = service();
    // No provider is configured in tests, so the handler answers rather than
    // the router 404-ing on an unmatched literal segment.
    let res = TestClient::get(format!("{BASE}/api/v1/auth/oauth/google"))
        .send(&service)
        .await;
    assert_ne!(
        res.status_code,
        Some(StatusCode::NOT_FOUND),
        "the {{provider}} segment must be treated as a parameter"
    );
}

#[tokio::test]
async fn plugin_id_routes_are_reachable() {
    let service = service();
    let token = register(&service, "plugin@example.com").await;

    let res = TestClient::post(format!("{BASE}/api/v1/plugins/some-plugin/install"))
        .add_header("authorization", format!("Bearer {token}"), true)
        .send(&service)
        .await;
    assert_ne!(
        res.status_code,
        Some(StatusCode::NOT_FOUND),
        "the {{pluginId}} segment must be treated as a parameter"
    );

    let res = TestClient::get(format!("{BASE}/api/v1/plugins/installed"))
        .add_header("authorization", format!("Bearer {token}"), true)
        .send(&service)
        .await;
    assert_eq!(res.status_code, Some(StatusCode::OK));
}

#[tokio::test]
async fn another_users_document_is_not_reachable() {
    let service = service();
    let owner = register(&service, "owner@example.com").await;
    let other = register(&service, "other@example.com").await;
    let id = create_document(&service, &owner, "Private").await;

    let res = TestClient::get(format!("{BASE}/api/v1/documents/{id}"))
        .add_header("authorization", format!("Bearer {other}"), true)
        .send(&service)
        .await;
    assert_eq!(
        res.status_code,
        Some(StatusCode::NOT_FOUND),
        "documents must not be readable across accounts"
    );
}

#[tokio::test]
async fn authenticated_routes_reject_a_missing_token() {
    let service = service();
    let res = TestClient::get(format!("{BASE}/api/v1/documents"))
        .send(&service)
        .await;
    assert_eq!(res.status_code, Some(StatusCode::UNAUTHORIZED));
}

#[tokio::test]
async fn documents_larger_than_the_transport_default_are_accepted() {
    // Salvo caps a JSON body at 64 KiB unless told otherwise, so without the
    // size middleware every document above that failed with a parse error and
    // the handlers' own 16 MiB limit was unreachable.
    let service = service();
    let token = register(&service, "big-doc@example.com").await;

    let filler = "<rect width=\"1\" height=\"1\"/>".repeat(8000);
    let svg = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"10\" height=\"10\">{filler}</svg>"
    );
    assert!(svg.len() > 200_000, "test document must exceed the old cap");

    let res = TestClient::post(format!("{BASE}/api/v1/documents"))
        .add_header("authorization", format!("Bearer {token}"), true)
        .json(&json!({ "title": "Big", "svg_content": svg }))
        .send(&service)
        .await;
    assert_eq!(
        res.status_code,
        Some(StatusCode::CREATED),
        "a 200 KB document must be accepted"
    );
}

#[tokio::test]
async fn export_refuses_to_read_local_files_through_image_href() {
    // `<image href>` is attacker controlled on this public endpoint. Rendering
    // must not touch the file system, whatever the path says.
    let service = service();
    let svg = "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"8\" height=\"8\">\
               <image href=\"/etc/hostname\" width=\"8\" height=\"8\"/></svg>";

    let res = TestClient::post(format!("{BASE}/api/v1/export"))
        .json(&json!({ "svg_content": svg, "format": "Png", "width": 8, "height": 8 }))
        .send(&service)
        .await;
    assert_eq!(
        res.status_code,
        Some(StatusCode::OK),
        "the request should succeed with the image simply omitted"
    );
}

#[tokio::test]
async fn export_survives_a_self_referencing_use_element() {
    // This used to overflow the stack, which aborts the process rather than
    // failing one request.
    let service = service();
    let svg = "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"16\" height=\"16\">\
               <g id=\"a\"><rect width=\"4\" height=\"4\"/><use href=\"#a\"/></g></svg>";

    let res = TestClient::post(format!("{BASE}/api/v1/export"))
        .json(&json!({ "svg_content": svg, "format": "Png", "width": 16, "height": 16 }))
        .send(&service)
        .await;
    assert_eq!(res.status_code, Some(StatusCode::OK));
}

#[tokio::test]
async fn export_survives_a_filter_that_used_to_panic() {
    let service = service();
    let svg = "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"16\" height=\"16\">\
               <defs><filter id=\"f\"><feMerge/><feComposite in2=\"SourceGraphic\"/></filter></defs>\
               <rect width=\"8\" height=\"8\" fill=\"red\" filter=\"url(#f)\"/></svg>";

    let res = TestClient::post(format!("{BASE}/api/v1/export"))
        .json(&json!({ "svg_content": svg, "format": "Png", "width": 16, "height": 16 }))
        .send(&service)
        .await;
    assert_eq!(res.status_code, Some(StatusCode::OK));
}

#[tokio::test]
async fn export_rejects_absurdly_deep_nesting_without_crashing() {
    let service = service();
    let body = "<g>".repeat(5000);
    let close = "</g>".repeat(5000);
    let svg = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"8\" height=\"8\">{body}<rect width=\"1\" height=\"1\"/>{close}</svg>"
    );

    let res = TestClient::post(format!("{BASE}/api/v1/export"))
        .json(&json!({ "svg_content": svg, "format": "Png", "width": 8, "height": 8 }))
        .send(&service)
        .await;
    assert_eq!(
        res.status_code,
        Some(StatusCode::BAD_REQUEST),
        "deep nesting should be a client error, not a crash"
    );
}

#[tokio::test]
async fn deleting_a_document_also_drops_its_versions_and_shares() {
    // Versions and shares used to outlive the document they belonged to, so
    // memory only ever grew and revoked share tokens still resolved far enough
    // to be touched on every lookup.
    let service = service();
    let token = register(&service, "cascade@example.com").await;
    let id = create_document(&service, &token, "Doomed").await;

    let mut res = TestClient::post(format!("{BASE}/api/v1/documents/{id}/shares"))
        .add_header("authorization", format!("Bearer {token}"), true)
        .json(&json!({ "permission": "View" }))
        .send(&service)
        .await;
    let share: Value = res.take_json().await.expect("share response");
    let share_token = share["token"].as_str().expect("share token").to_string();

    let res = TestClient::delete(format!("{BASE}/api/v1/documents/{id}"))
        .add_header("authorization", format!("Bearer {token}"), true)
        .send(&service)
        .await;
    assert!(res.status_code.is_some_and(|s| s.is_success()));

    let res = TestClient::get(format!("{BASE}/api/v1/shared/{share_token}"))
        .send(&service)
        .await;
    assert_eq!(
        res.status_code,
        Some(StatusCode::NOT_FOUND),
        "a share of a deleted document must not resolve"
    );
}

#[tokio::test]
async fn version_history_is_capped() {
    let service = service();
    let token = register(&service, "versions@example.com").await;
    let id = create_document(&service, &token, "Churn").await;

    let cap = sivgo_web::store::Store::MAX_VERSIONS_PER_DOCUMENT;
    for i in 0..(cap + 10) {
        let res = TestClient::put(format!("{BASE}/api/v1/documents/{id}"))
            .add_header("authorization", format!("Bearer {token}"), true)
            .json(&json!({ "title": format!("v{i}") }))
            .send(&service)
            .await;
        assert_eq!(res.status_code, Some(StatusCode::OK));
    }

    let mut res = TestClient::get(format!("{BASE}/api/v1/documents/{id}/versions"))
        .add_header("authorization", format!("Bearer {token}"), true)
        .send(&service)
        .await;
    let versions: Value = res.take_json().await.expect("versions response");
    let count = versions.as_array().map(Vec::len).unwrap_or(0);
    assert!(
        count <= cap,
        "history should be capped at {cap} entries, found {count}"
    );
    assert!(count > 0, "some history should remain");
}

#[tokio::test]
async fn document_count_per_account_is_capped() {
    let service = service();
    let token = register(&service, "quota@example.com").await;

    // Fill the quota, then confirm one more is refused.
    let cap = sivgo_web::store::Store::MAX_DOCUMENTS_PER_USER;
    for i in 0..cap {
        let res = TestClient::post(format!("{BASE}/api/v1/documents"))
            .add_header("authorization", format!("Bearer {token}"), true)
            .json(&json!({ "title": format!("doc {i}") }))
            .send(&service)
            .await;
        assert_eq!(res.status_code, Some(StatusCode::CREATED), "doc {i}");
    }

    let res = TestClient::post(format!("{BASE}/api/v1/documents"))
        .add_header("authorization", format!("Bearer {token}"), true)
        .json(&json!({ "title": "one too many" }))
        .send(&service)
        .await;
    assert_eq!(
        res.status_code,
        Some(StatusCode::BAD_REQUEST),
        "exceeding the quota must be refused"
    );
}

#[tokio::test]
async fn an_oauth_login_cannot_take_over_an_existing_password_account() {
    // Resolving an OAuth login by email address would let anyone who can get a
    // provider token for that address sign in as the existing local account.
    // The provider's own subject id is the only safe key.
    use sivgo_web::store::Store;

    let store = Store::new();
    let victim = store.create_user("victim@example.com", "victim", "hash");

    // No identity is bound yet, so the address alone must not resolve.
    assert!(
        store
            .find_user_by_oauth_identity("google", "attacker-subject")
            .is_none(),
        "an unbound identity must not resolve to any account"
    );

    // Binding is explicit, and only then does the identity resolve.
    store.link_oauth_identity("google", "victim-subject", victim.id);
    assert_eq!(
        store
            .find_user_by_oauth_identity("google", "victim-subject")
            .map(|u| u.id),
        Some(victim.id)
    );

    // A different subject on the same provider is still a different person.
    assert!(store
        .find_user_by_oauth_identity("google", "someone-else")
        .is_none());

    // So is the same subject on a different provider.
    assert!(store
        .find_user_by_oauth_identity("github", "victim-subject")
        .is_none());
}

#[tokio::test]
async fn registering_the_same_email_twice_is_refused() {
    let service = service();
    let _ = register(&service, "dup@example.com").await;

    let res = TestClient::post(format!("{BASE}/api/v1/auth/register"))
        .json(&json!({
            "email": "dup@example.com",
            "username": "other",
            "password": "correct horse battery staple",
        }))
        .send(&service)
        .await;
    assert_eq!(res.status_code, Some(StatusCode::CONFLICT));
}

#[tokio::test]
async fn login_rejects_a_wrong_password_and_an_unknown_account_alike() {
    let service = service();
    let _ = register(&service, "login@example.com").await;

    let wrong = TestClient::post(format!("{BASE}/api/v1/auth/login"))
        .json(&json!({ "email": "login@example.com", "password": "not the password" }))
        .send(&service)
        .await;
    let unknown = TestClient::post(format!("{BASE}/api/v1/auth/login"))
        .json(&json!({ "email": "nobody@example.com", "password": "not the password" }))
        .send(&service)
        .await;

    // Identical responses: the API must not reveal which accounts exist.
    assert_eq!(wrong.status_code, Some(StatusCode::UNAUTHORIZED));
    assert_eq!(unknown.status_code, wrong.status_code);
}

#[tokio::test]
async fn the_config_debug_output_hides_the_jwt_secret() {
    let config = ServerConfig {
        jwt_secret: "super-secret-value-that-must-not-leak".to_string(),
        ..ServerConfig::default()
    };
    let printed = format!("{config:?}");
    assert!(
        !printed.contains("super-secret-value-that-must-not-leak"),
        "the secret leaked into Debug output: {printed}"
    );
    assert!(printed.contains("redacted"));
}
