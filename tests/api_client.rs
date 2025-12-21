use checkvist_cli::api::{CheckvistApi, Order};
use checkvist_cli::error::ErrorKind;
use serde_json::json;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::thread;

#[test]
fn login_returns_token_on_success() {
    let server = StubServer::new(vec![StubResponse::json(
        "POST",
        "/auth/login.json?version=2",
        200,
        json!({"token": "LOGIN_TOKEN"}).to_string(),
    )]);

    let api = CheckvistApi::new(server.base_url());
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
        json!({"token": "NEW_TOKEN"}).to_string(),
    )]);

    let api = CheckvistApi::new(server.base_url());
    let token = api.refresh_token("OLD_TOKEN").expect("refresh succeeds");
    assert_eq!(token, "NEW_TOKEN");
}

#[test]
fn get_checklists_uses_token_header() {
    let response_body = json!([
        {"id": 1, "name": "List One"},
        {"id": 2, "name": "List Two"}
    ])
    .to_string();

    let server = StubServer::new(vec![StubResponse::json_with_header_check(
        "GET",
        "/checklists.json?order=id%3Aasc",
        200,
        response_body,
        ("x-client-token", "MYTOKEN"),
    )]);

    let api = CheckvistApi::new(server.base_url());
    let lists = api
        .get_checklists("MYTOKEN", Some(false), Some(Order::IdAsc), Some(false))
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

    let api = CheckvistApi::new(server.base_url());
    let err = api
        .get_checklists("TOKEN", None, None, None)
        .expect_err("should fail");

    assert_eq!(err.kind(), ErrorKind::ApiData);
}

#[derive(Clone)]
struct StubResponse {
    method: &'static str,
    path: &'static str,
    status: u16,
    body: String,
    expected_header: Option<(&'static str, &'static str)>,
}

impl StubResponse {
    fn json(method: &'static str, path: &'static str, status: u16, body: String) -> Self {
        Self {
            method,
            path,
            status,
            body,
            expected_header: None,
        }
    }

    fn json_with_header_check(
        method: &'static str,
        path: &'static str,
        status: u16,
        body: String,
        header: (&'static str, &'static str),
    ) -> Self {
        Self {
            method,
            path,
            status,
            body,
            expected_header: Some(header),
        }
    }

    fn raw(method: &'static str, path: &'static str, status: u16, body: &str) -> Self {
        Self {
            method,
            path,
            status,
            body: body.to_string(),
            expected_header: None,
        }
    }
}

struct StubServer {
    addr: String,
}

impl StubServer {
    fn new(responses: Vec<StubResponse>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let addr = listener.local_addr().unwrap();
        thread::spawn(move || {
            for mut resp in responses {
                let (mut stream, _) = listener.accept().expect("accept");
                handle_connection(&mut stream, &mut resp);
            }
        });

        Self {
            addr: format!("http://{}", addr),
        }
    }

    fn base_url(&self) -> String {
        self.addr.clone()
    }
}

fn handle_connection(stream: &mut TcpStream, expected: &mut StubResponse) {
    let request = read_request(stream);
    let request_lower = request.to_lowercase();
    let mut lines = request.lines();
    let request_line = lines.next().unwrap_or("");
    let mut parts = request_line.split_whitespace();
    let method = parts.next().unwrap_or("");
    let path = parts.next().unwrap_or("");

    assert_eq!(method, expected.method);
    assert_eq!(path, expected.path);

    if let Some((header, value)) = expected.expected_header {
        let needle = format!("{}: {}", header.to_lowercase(), value.to_lowercase());
        assert!(
            request_lower.contains(&needle),
            "missing header {} in request: {}",
            header,
            request
        );
    }

    let body = expected.body.clone();
    let response = format!(
        "HTTP/1.1 {} OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
        expected.status,
        body.len(),
        body
    );

    stream
        .write_all(response.as_bytes())
        .expect("write response");
    stream.flush().expect("flush");
}

fn read_request(stream: &mut TcpStream) -> String {
    let mut data = Vec::new();
    let mut buffer = [0u8; 1024];
    loop {
        let n = stream.read(&mut buffer).expect("read request chunk");
        if n == 0 {
            break;
        }
        data.extend_from_slice(&buffer[..n]);
        if data.windows(4).any(|w| w == b"\r\n\r\n") || data.windows(2).any(|w| w == b"\n\n") {
            break;
        }
    }
    String::from_utf8_lossy(&data).into_owned()
}
