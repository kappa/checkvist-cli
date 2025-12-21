mod common;

use checkvist_cli::api::{CheckvistApi, Order};
use checkvist_cli::error::ErrorKind;
use common::{StubResponse, StubServer};
use serde_json::json;

#[test]
fn login_returns_token_on_success() {
    let server = StubServer::new(vec![StubResponse::json(
        "POST",
        "/auth/login.json?version=2",
        200,
        json!({"token": "LOGIN_TOKEN"}),
    )]);

    let api = CheckvistApi::new(server.base_url()).expect("api init");
    let token = api
        .login("user@example.com", "REMOTE", None)
        .expect("login succeeds");
    assert_eq!(token, "LOGIN_TOKEN");
}

#[test]
fn refresh_returns_new_token() {
    let server = StubServer::new(vec![StubResponse::json(
        "POST",
        "/auth/refresh_token.json?version=2",
        200,
        json!({"token": "NEW_TOKEN"}),
    )]);

    let api = CheckvistApi::new(server.base_url()).expect("api init");
    let token = api.refresh_token("OLD_TOKEN").expect("refresh succeeds");
    assert_eq!(token, "NEW_TOKEN");
}

#[test]
fn get_checklists_uses_token_header() {
    let response_body = json!([
        {"id": 1, "name": "List One"},
        {"id": 2, "name": "List Two"}
    ]);

    let server = StubServer::new(vec![StubResponse::json_with_header(
        "GET",
        "/checklists.json?order=id%3Aasc&skip_stats=true",
        200,
        response_body,
        ("x-client-token", "MYTOKEN"),
    )]);

    let api = CheckvistApi::new(server.base_url()).expect("api init");
    let lists = api
        .get_checklists("MYTOKEN", Some(false), Some(Order::IdAsc), Some(true))
        .expect("lists succeed");

    assert_eq!(lists.len(), 2);
    assert_eq!(lists[0]["id"], 1);
    assert_eq!(lists[1]["name"], "List Two");
}

#[test]
fn invalid_json_returns_data_error() {
    let server = StubServer::new(vec![StubResponse::raw(
        "GET",
        "/checklists.json",
        200,
        "not json",
    )]);

    let api = CheckvistApi::new(server.base_url()).expect("api init");
    let err = api
        .get_checklists("TOKEN", None, None, None)
        .expect_err("should fail");

    assert_eq!(err.kind(), ErrorKind::ApiData);
}
