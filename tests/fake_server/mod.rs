use serde_json::{json, Value};
use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::thread;

mod lists;

/// Fake Checkvist API server for testing
///
/// This server simulates the behavior of the real Checkvist API
/// without persistence. It's designed for fast, reliable testing.
pub struct FakeServer {
    base_url: String,
    _handle: thread::JoinHandle<()>,
    state: Arc<Mutex<ServerState>>,
}

#[derive(Default)]
struct ServerState {
    checklists: HashMap<i64, Checklist>,
    next_checklist_id: i64,
    // Will add tasks, notes, etc. as needed
}

#[derive(Clone, Debug)]
pub struct Checklist {
    pub id: i64,
    pub name: String,
    pub public: bool,
    pub archived: bool,
    pub created_at: String,
    pub updated_at: String,
    pub user_updated_at: String,
    pub markdown: bool,
    pub read_only: bool,
    pub options: i32,
    pub user_count: i32,
    pub task_count: i32,
    pub task_completed: i32,
    pub percent_completed: i32,
    pub item_count: i32,
}

impl Checklist {
    fn to_json(&self) -> Value {
        json!({
            "id": self.id,
            "name": self.name,
            "public": self.public,
            "archived": self.archived,
            "created_at": self.created_at,
            "updated_at": self.updated_at,
            "user_updated_at": self.user_updated_at,
            "markdown?": self.markdown,
            "read_only": self.read_only,
            "options": self.options,
            "user_count": self.user_count,
            "task_count": self.task_count,
            "task_completed": self.task_completed,
            "percent_completed": self.percent_completed,
            "item_count": self.item_count,
            "tags": {},
            "tags_as_text": "",
            "related_task_ids": null,
        })
    }
}

struct Request {
    method: String,
    path: String,
    headers: HashMap<String, String>,
    body: String,
}

struct Response {
    status: u16,
    body: String,
    content_type: String,
}

impl Response {
    fn json(status: u16, body: Value) -> Self {
        Self {
            status,
            body: body.to_string(),
            content_type: "application/json".to_string(),
        }
    }

    fn error(status: u16, message: &str) -> Self {
        Self::json(status, json!({"error": message}))
    }
}

impl FakeServer {
    pub fn new() -> Self {
        let state = Arc::new(Mutex::new(ServerState::default()));
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind fake server");
        let addr = listener.local_addr().unwrap();

        let state_clone = state.clone();
        let handle = thread::spawn(move || {
            for stream in listener.incoming() {
                if let Ok(stream) = stream {
                    let state = state_clone.clone();
                    thread::spawn(move || handle_connection(stream, state));
                }
            }
        });

        FakeServer {
            base_url: format!("http://{}", addr),
            _handle: handle,
            state,
        }
    }

    pub fn base_url(&self) -> &str {
        &self.base_url
    }
}

fn handle_connection(mut stream: TcpStream, state: Arc<Mutex<ServerState>>) {
    let request = match parse_request(&mut stream) {
        Ok(req) => req,
        Err(e) => {
            eprintln!("Failed to parse request: {}", e);
            return;
        }
    };

    // Route the request
    let response = route_request(&request, &state);

    // Send response
    send_response(&mut stream, &response);
}

fn route_request(req: &Request, state: &Arc<Mutex<ServerState>>) -> Response {
    // Check authentication
    if !req.headers.contains_key("x-client-token") {
        return Response::error(401, "Unauthorized");
    }

    // Route based on method and path
    match (req.method.as_str(), req.path.as_str()) {
        ("POST", "/checklists.json") => lists::create_checklist(req, state),
        ("GET", "/checklists.json") => lists::get_checklists(req, state),
        _ => Response::error(404, "Not Found"),
    }
}

fn parse_request(stream: &mut TcpStream) -> Result<Request, String> {
    let mut buffer = vec![0u8; 4096];
    let n = stream
        .read(&mut buffer)
        .map_err(|e| format!("read error: {}", e))?;

    let data = String::from_utf8_lossy(&buffer[..n]);
    let mut lines = data.lines();

    // Parse request line
    let request_line = lines.next().ok_or("empty request")?;
    let mut parts = request_line.split_whitespace();
    let method = parts.next().ok_or("no method")?.to_string();
    let path = parts.next().ok_or("no path")?.to_string();

    // Parse headers
    let mut headers = HashMap::new();
    let mut body_start = 0;
    for (i, line) in lines.enumerate() {
        if line.is_empty() {
            body_start = data
                .char_indices()
                .nth(
                    data.lines()
                        .take(i + 2)
                        .map(|l| l.len() + 1)
                        .sum::<usize>(),
                )
                .map(|(pos, _)| pos)
                .unwrap_or(data.len());
            break;
        }
        if let Some((key, value)) = line.split_once(':') {
            headers.insert(
                key.trim().to_lowercase(),
                value.trim().to_string(),
            );
        }
    }

    // Extract body
    let body = if body_start < data.len() {
        data[body_start..].trim().to_string()
    } else {
        String::new()
    };

    Ok(Request {
        method,
        path,
        headers,
        body,
    })
}

fn send_response(stream: &mut TcpStream, response: &Response) {
    let status_text = match response.status {
        200 => "OK",
        201 => "Created",
        400 => "Bad Request",
        401 => "Unauthorized",
        404 => "Not Found",
        500 => "Internal Server Error",
        _ => "Unknown",
    };

    let response_str = format!(
        "HTTP/1.1 {} {}\r\nContent-Type: {}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        response.status,
        status_text,
        response.content_type,
        response.body.len(),
        response.body
    );

    let _ = stream.write_all(response_str.as_bytes());
    let _ = stream.flush();
}

/// Parse URL-encoded form data
pub(crate) fn parse_form_data(body: &str) -> HashMap<String, String> {
    if body.is_empty() {
        return HashMap::new();
    }

    body.split('&')
        .filter_map(|pair| {
            let mut parts = pair.splitn(2, '=');
            let key = parts.next()?;
            let value = parts.next().unwrap_or("");
            // In application/x-www-form-urlencoded, + means space
            let key_decoded = urlencoding::decode(&key.replace('+', " ")).ok()?.to_string();
            let value_decoded = urlencoding::decode(&value.replace('+', " ")).ok()?.to_string();
            Some((key_decoded, value_decoded))
        })
        .collect()
}

fn now() -> String {
    // For testing, use a fixed timestamp
    "2025/12/22 20:00:00 -0800".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_form_data_with_nested_parameters() {
        // Test that checklist[name] is correctly parsed
        let body = "checklist%5Bname%5D=My+List";
        let params = parse_form_data(body);

        assert_eq!(params.get("checklist[name]"), Some(&"My List".to_string()));
    }

    #[test]
    fn test_parse_form_data_with_simple_parameters() {
        // Test that simple name parameter is parsed
        let body = "name=My+List";
        let params = parse_form_data(body);

        assert_eq!(params.get("name"), Some(&"My List".to_string()));
        assert_eq!(params.get("checklist[name]"), None);
    }

    #[test]
    fn test_parse_form_data_with_multiple_parameters() {
        let body = "checklist%5Bname%5D=Test&checklist%5Bpublic%5D=true";
        let params = parse_form_data(body);

        assert_eq!(params.get("checklist[name]"), Some(&"Test".to_string()));
        assert_eq!(params.get("checklist[public]"), Some(&"true".to_string()));
    }

    #[test]
    fn test_parse_form_data_with_unicode() {
        let body = "checklist%5Bname%5D=%D0%A2%D0%B5%D1%81%D1%82";  // "Тест" in UTF-8
        let params = parse_form_data(body);

        assert_eq!(params.get("checklist[name]"), Some(&"Тест".to_string()));
    }

    #[test]
    fn test_parse_form_data_empty_body() {
        let body = "";
        let params = parse_form_data(body);

        assert_eq!(params.len(), 0);
    }
}
