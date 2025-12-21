use serde_json::Value;
use std::io::{Read, Write};
use std::net::{Shutdown, TcpListener, TcpStream};
use std::thread;

#[derive(Clone)]
pub struct StubResponse {
    pub method: &'static str,
    pub path: &'static str,
    pub status: u16,
    pub body: String,
    pub required_header: Option<(&'static str, &'static str)>,
    pub required_body_contains: Option<&'static str>,
    pub content_type: &'static str,
}

#[allow(dead_code)]
impl StubResponse {
    pub fn json(method: &'static str, path: &'static str, status: u16, body: Value) -> Self {
        Self {
            method,
            path,
            status,
            body: body.to_string(),
            required_header: None,
            required_body_contains: None,
            content_type: "application/json",
        }
    }

    pub fn json_with_header(
        method: &'static str,
        path: &'static str,
        status: u16,
        body: Value,
        header: (&'static str, &'static str),
    ) -> Self {
        Self {
            method,
            path,
            status,
            body: body.to_string(),
            required_header: Some(header),
            required_body_contains: None,
            content_type: "application/json",
        }
    }

    pub fn raw(method: &'static str, path: &'static str, status: u16, body: &str) -> Self {
        Self {
            method,
            path,
            status,
            body: body.to_string(),
            required_header: None,
            required_body_contains: None,
            content_type: "text/plain",
        }
    }

    pub fn with_body_check(mut self, needle: &'static str) -> Self {
        self.required_body_contains = Some(needle);
        self
    }
}

pub struct StubServer {
    addr: String,
    _handle: thread::JoinHandle<()>,
}

impl StubServer {
    pub fn new(responses: Vec<StubResponse>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let addr = listener.local_addr().unwrap();

        let handle = thread::spawn(move || {
            for mut resp in responses {
                let (mut stream, _) = listener.accept().expect("accept");
                handle_connection(&mut stream, &mut resp);
            }
        });

        Self {
            addr: format!("http://{}", addr),
            _handle: handle,
        }
    }

    pub fn base_url(&self) -> String {
        self.addr.clone()
    }
}

fn handle_connection(stream: &mut TcpStream, expected: &mut StubResponse) {
    let request = read_request(stream);
    let request_lower = request.raw.to_lowercase();
    let mut lines = request.raw.lines();
    let request_line = lines.next().unwrap_or("");
    let mut parts = request_line.split_whitespace();
    let method = parts.next().unwrap_or("");
    let path = parts.next().unwrap_or("");

    assert_eq!(method, expected.method);
    assert_eq!(path, expected.path);

    if let Some((header, value)) = expected.required_header {
        let needle = format!("{}: {}", header.to_lowercase(), value.to_lowercase());
        assert!(
            request_lower.contains(&needle),
            "missing header {} in request: {}",
            header,
            request.raw
        );
    }

    if let Some(body_needle) = expected.required_body_contains {
        assert!(
            request.body.contains(body_needle),
            "missing body content {} in request: {}",
            body_needle,
            request.raw
        );
    }

    let reason = match expected.status {
        200 => "OK",
        401 => "Unauthorized",
        403 => "Forbidden",
        500 => "Internal Server Error",
        _ => "OK",
    };

    let response = format!(
        "HTTP/1.1 {} {}\r\nContent-Type: {}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        expected.status,
        reason,
        expected.content_type,
        expected.body.len(),
        expected.body
    );

    stream
        .write_all(response.as_bytes())
        .expect("write response");
    stream.flush().expect("flush");
    let _ = stream.shutdown(Shutdown::Both);
}

struct RequestCapture {
    raw: String,
    body: String,
}

fn read_request(stream: &mut TcpStream) -> RequestCapture {
    let mut data = Vec::new();
    let mut buffer = [0u8; 1024];
    let mut header_end = None;
    let mut delimiter: &[u8] = b"\r\n\r\n";

    loop {
        let n = stream.read(&mut buffer).expect("read request chunk");
        if n == 0 {
            break;
        }
        data.extend_from_slice(&buffer[..n]);
        if let Some(idx) = find_header_end(&data) {
            header_end = Some(idx.0);
            delimiter = idx.1;
            break;
        }
    }

    let header_end = header_end.unwrap_or(data.len());
    let header_str = String::from_utf8_lossy(&data[..header_end]).to_string();
    let content_length = parse_content_length(&header_str);

    let body_start = header_end + delimiter.len();
    let target_len = body_start + content_length;
    while data.len() < target_len {
        let mut buffer = [0u8; 1024];
        let n = stream.read(&mut buffer).unwrap_or(0);
        if n == 0 {
            break;
        }
        data.extend_from_slice(&buffer[..n]);
    }

    let raw = String::from_utf8_lossy(&data).into_owned();
    let body = raw
        .splitn(2, std::str::from_utf8(delimiter).unwrap())
        .nth(1)
        .unwrap_or("")
        .to_string();

    RequestCapture { raw, body }
}

fn find_header_end(data: &[u8]) -> Option<(usize, &'static [u8])> {
    if let Some(idx) = data.windows(4).position(|w| w == b"\r\n\r\n") {
        return Some((idx, b"\r\n\r\n"));
    }
    if let Some(idx) = data.windows(2).position(|w| w == b"\n\n") {
        return Some((idx, b"\n\n"));
    }
    None
}

fn parse_content_length(headers: &str) -> usize {
    for line in headers.lines() {
        let mut parts = line.splitn(2, ':');
        let name = parts.next().unwrap_or("").trim().to_lowercase();
        if name == "content-length" {
            if let Some(value) = parts.next() {
                return value.trim().parse().unwrap_or(0);
            }
        }
    }
    0
}
