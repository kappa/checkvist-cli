use crate::error::{AppError, AppResult, ErrorKind};
use crate::{log::redact_sensitive, vlog};
use serde::Deserialize;
use serde_json::Value;
use std::time::Duration;
use ureq::{Agent, AgentBuilder};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Order {
    IdAsc,
    IdDesc,
    UpdatedAtAsc,
    UpdatedAtDesc,
}

impl Order {
    fn as_query(&self) -> &'static str {
        match self {
            Order::IdAsc => "id:asc",
            Order::IdDesc => "id:desc",
            Order::UpdatedAtAsc => "updated_at:asc",
            Order::UpdatedAtDesc => "updated_at:desc",
        }
    }
}

pub struct ChecklistsResponse {
    pub items: Vec<Value>,
    pub raw: String,
}

pub struct CheckvistApi {
    base_url: String,
    agent: Agent,
}

impl CheckvistApi {
    pub fn new(base_url: String) -> AppResult<Self> {
        let mut builder = AgentBuilder::new()
            .timeout_connect(Duration::from_secs(5))
            .timeout_read(Duration::from_secs(20))
            .timeout_write(Duration::from_secs(20));

        let tls_connector = ureq::native_tls::TlsConnector::new().map_err(|err| {
            AppError::new(ErrorKind::Local, format!("failed to init TLS: {}", err))
        })?;
        builder = builder.tls_connector(std::sync::Arc::new(tls_connector));

        let agent = builder.build();

        Ok(Self {
            base_url: base_url.trim_end_matches('/').to_string(),
            agent,
        })
    }

    fn log_request(&self, method: &str, url: &str, headers: &[(&str, &str)], body: Option<&str>) {
        vlog!(1, "HTTP {} {}", method, url);
        for (key, value) in headers {
            if key == &"X-Client-Token" {
                vlog!(1, "  Header: {}: {}", key, redact_sensitive(value, 8));
            } else {
                vlog!(1, "  Header: {}: {}", key, value);
            }
        }
        if let Some(body) = body {
            vlog!(1, "  Body: {}", body);
        }
    }

    fn log_response(&self, status: u16, body: &str) {
        vlog!(1, "HTTP Response: {}", status);
        if body.len() > 500 {
            vlog!(1, "  Body: {}... ({} bytes)", &body[..500], body.len());
        } else {
            vlog!(1, "  Body: {}", body);
        }
    }

    pub fn login(
        &self,
        username: &str,
        remote_key: &str,
        token2fa: Option<&str>,
    ) -> AppResult<String> {
        let url = format!("{}/auth/login.json?version=2", self.base_url);
        let mut params = vec![("username", username), ("remote_key", remote_key)];
        if let Some(token2fa) = token2fa {
            params.push(("token2fa", token2fa));
        }

        let body_str = format!(
            "username={}&remote_key={}{}",
            urlencoding::encode(username),
            redact_sensitive(remote_key, 3),
            token2fa.map(|t| format!("&token2fa={}", redact_sensitive(t, 2))).unwrap_or_default()
        );
        self.log_request("POST", &url, &[("Accept", "application/json")], Some(&body_str));

        let response = self
            .agent
            .post(&url)
            .set("Accept", "application/json")
            .send_form(&params)
            .map_err(map_network_error)?;

        let status = response.status();
        let body = response.into_string().map_err(|err| {
            AppError::new(
                ErrorKind::ApiData,
                format!("failed to read login response: {}", err),
            )
        })?;

        self.log_response(status, &body);

        let parsed: TokenResponse = serde_json::from_str(&body).map_err(|err| {
            AppError::new(
                ErrorKind::ApiData,
                format!("invalid login response: {}", err),
            )
        })?;
        Ok(parsed.token)
    }

    pub fn refresh_token(&self, token: &str) -> AppResult<String> {
        let url = format!("{}/auth/refresh_token.json?version=2", self.base_url);

        self.log_request(
            "POST",
            &url,
            &[("Accept", "application/json"), ("X-Client-Token", token)],
            None,
        );

        let response = self
            .agent
            .post(&url)
            .set("Accept", "application/json")
            .set("X-Client-Token", token)
            .call()
            .map_err(map_network_error)?;

        let status = response.status();
        let body = response.into_string().map_err(|err| {
            AppError::new(
                ErrorKind::ApiData,
                format!("failed to read refresh response: {}", err),
            )
        })?;

        self.log_response(status, &body);

        let parsed: TokenResponse = serde_json::from_str(&body).map_err(|err| {
            AppError::new(
                ErrorKind::ApiData,
                format!("invalid refresh response: {}", err),
            )
        })?;
        Ok(parsed.token)
    }

    pub fn get_checklists(
        &self,
        token: &str,
        archived: Option<bool>,
        order: Option<Order>,
        skip_stats: Option<bool>,
    ) -> AppResult<Vec<Value>> {
        Ok(self
            .get_checklists_raw(token, archived, order, skip_stats)?
            .items)
    }

    pub fn get_checklists_raw(
        &self,
        token: &str,
        archived: Option<bool>,
        order: Option<Order>,
        skip_stats: Option<bool>,
    ) -> AppResult<ChecklistsResponse> {
        let url = self.checklists_url(archived, order, skip_stats);

        let response = self
            .agent
            .get(&url)
            .set("Accept", "application/json")
            .set("X-Client-Token", token)
            .call()
            .map_err(map_network_error)?;

        let raw = response.into_string().map_err(|err| {
            AppError::new(
                ErrorKind::ApiData,
                format!("invalid response body from lists endpoint: {}", err),
            )
        })?;

        let value: serde_json::Value = serde_json::from_str(&raw).map_err(|err| {
            AppError::new(
                ErrorKind::ApiData,
                format!("invalid JSON from lists endpoint: {}", err),
            )
        })?;

        match value {
            Value::Array(items) => Ok(ChecklistsResponse { items, raw }),
            _ => Err(AppError::new(
                ErrorKind::ApiData,
                "expected array of lists from API",
            )),
        }
    }

    fn checklists_url(
        &self,
        archived: Option<bool>,
        order: Option<Order>,
        skip_stats: Option<bool>,
    ) -> String {
        let mut url = format!("{}/checklists.json", self.base_url);

        let mut params = vec![];
        if let Some(true) = archived {
            params.push(("archived", "true".to_string()));
        }
        if let Some(order) = order {
            params.push(("order", order.as_query().to_string()));
        }
        if let Some(true) = skip_stats {
            params.push(("skip_stats", "true".to_string()));
        }

        if !params.is_empty() {
            let query: Vec<String> = params
                .into_iter()
                .map(|(k, v)| format!("{}={}", k, urlencoding::encode(&v)))
                .collect();
            url.push('?');
            url.push_str(&query.join("&"));
        }
        url
    }

    pub fn create_checklist(&self, token: &str, name: &str) -> AppResult<Value> {
        let url = format!("{}/checklists.json", self.base_url);
        let response = self
            .agent
            .post(&url)
            .set("Accept", "application/json")
            .set("X-Client-Token", token)
            .send_form(&[("checklist[name]", name)])
            .map_err(map_network_error)?;

        response.into_json().map_err(|err| {
            AppError::new(
                ErrorKind::ApiData,
                format!("invalid JSON from checklist create: {}", err),
            )
        })
    }

    pub fn delete_checklist(&self, token: &str, list_id: i64) -> AppResult<()> {
        let url = format!("{}/checklists/{}.json", self.base_url, list_id);
        self.agent
            .delete(&url)
            .set("Accept", "application/json")
            .set("X-Client-Token", token)
            .call()
            .map_err(map_network_error)?;

        Ok(())
    }

    pub fn update_checklist(
        &self,
        token: &str,
        list_id: i64,
        archived: Option<bool>,
        public: Option<bool>,
    ) -> AppResult<Value> {
        let url = format!("{}/checklists/{}.json", self.base_url, list_id);
        let mut params: Vec<(&str, String)> = Vec::new();
        if let Some(archived) = archived {
            params.push(("archived", archived.to_string()));
        }
        if let Some(public) = public {
            params.push(("public", public.to_string()));
        }

        let param_refs: Vec<(&str, &str)> = params.iter().map(|(k, v)| (*k, v.as_str())).collect();

        let response = self
            .agent
            .put(&url)
            .set("Accept", "application/json")
            .set("X-Client-Token", token)
            .send_form(&param_refs)
            .map_err(map_network_error)?;

        response.into_json().map_err(|err| {
            AppError::new(
                ErrorKind::ApiData,
                format!("invalid JSON from checklist update: {}", err),
            )
        })
    }

    pub fn get_checklist(&self, token: &str, list_id: i64) -> AppResult<Value> {
        let url = format!("{}/checklists/{}.json", self.base_url, list_id);
        let response = self
            .agent
            .get(&url)
            .set("Accept", "application/json")
            .set("X-Client-Token", token)
            .call()
            .map_err(map_network_error)?;

        response.into_json().map_err(|err| {
            AppError::new(
                ErrorKind::ApiData,
                format!("invalid JSON from checklist get: {}", err),
            )
        })
    }

    pub fn get_tasks(&self, token: &str, list_id: i64) -> AppResult<Vec<Value>> {
        let url = format!("{}/checklists/{}/tasks.json", self.base_url, list_id);
        let response = self
            .agent
            .get(&url)
            .set("Accept", "application/json")
            .set("X-Client-Token", token)
            .call()
            .map_err(map_network_error)?;

        let value: serde_json::Value = response.into_json().map_err(|err| {
            AppError::new(
                ErrorKind::ApiData,
                format!("invalid JSON from tasks endpoint: {}", err),
            )
        })?;

        match value {
            Value::Array(items) => Ok(items),
            _ => Err(AppError::new(
                ErrorKind::ApiData,
                "expected array of tasks from API",
            )),
        }
    }

    pub fn create_task(
        &self,
        token: &str,
        list_id: i64,
        content: &str,
        parent_id: Option<i64>,
    ) -> AppResult<Value> {
        let url = format!("{}/checklists/{}/tasks.json", self.base_url, list_id);
        let mut params: Vec<(&str, String)> = vec![("task[content]", content.to_string())];
        if let Some(parent_id) = parent_id {
            params.push(("task[parent_id]", parent_id.to_string()));
        }
        let param_refs: Vec<(&str, &str)> = params.iter().map(|(k, v)| (*k, v.as_str())).collect();
        let response = self
            .agent
            .post(&url)
            .set("Accept", "application/json")
            .set("X-Client-Token", token)
            .send_form(&param_refs)
            .map_err(map_network_error)?;

        response.into_json().map_err(|err| {
            AppError::new(
                ErrorKind::ApiData,
                format!("invalid JSON from task create: {}", err),
            )
        })
    }

    pub fn update_task(
        &self,
        token: &str,
        list_id: i64,
        task_id: i64,
        content: Option<&str>,
        status: Option<&str>,
        parent_id: Option<i64>,
        parse: bool,
    ) -> AppResult<Value> {
        let url = format!(
            "{}/checklists/{}/tasks/{}.json",
            self.base_url, list_id, task_id
        );
        let mut params: Vec<(&str, String)> = Vec::new();
        if let Some(content) = content {
            params.push(("task[content]", content.to_string()));
        }
        if let Some(status) = status {
            params.push(("task[status]", status.to_string()));
        }
        if let Some(parent_id) = parent_id {
            params.push(("task[parent_id]", parent_id.to_string()));
        }
        if parse {
            params.push(("parse", "true".to_string()));
        }
        let param_refs: Vec<(&str, &str)> = params.iter().map(|(k, v)| (*k, v.as_str())).collect();

        let response = self
            .agent
            .put(&url)
            .set("Accept", "application/json")
            .set("X-Client-Token", token)
            .send_form(&param_refs)
            .map_err(map_network_error)?;

        response.into_json().map_err(|err| {
            AppError::new(
                ErrorKind::ApiData,
                format!("invalid JSON from task update: {}", err),
            )
        })
    }

    pub fn close_task(&self, token: &str, list_id: i64, task_id: i64) -> AppResult<Value> {
        let url = format!(
            "{}/checklists/{}/tasks/{}/close.json",
            self.base_url, list_id, task_id
        );

        self.log_request(
            "POST",
            &url,
            &[("Accept", "application/json"), ("X-Client-Token", token)],
            None,
        );

        let response = self
            .agent
            .post(&url)
            .set("Accept", "application/json")
            .set("X-Client-Token", token)
            .call()
            .map_err(map_network_error)?;

        let status = response.status();
        let body = response.into_string().map_err(|err| {
            AppError::new(
                ErrorKind::ApiData,
                format!("failed to read close response: {}", err),
            )
        })?;

        self.log_response(status, &body);

        // API returns an array; extract the first element
        let arr: Vec<Value> = serde_json::from_str(&body).map_err(|err| {
            AppError::new(
                ErrorKind::ApiData,
                format!("invalid JSON from task close: {}", err),
            )
        })?;
        arr.into_iter().next().ok_or_else(|| {
            AppError::new(ErrorKind::ApiData, String::from("empty response from task close"))
        })
    }

    pub fn reopen_task(&self, token: &str, list_id: i64, task_id: i64) -> AppResult<Value> {
        let url = format!(
            "{}/checklists/{}/tasks/{}/reopen.json",
            self.base_url, list_id, task_id
        );

        self.log_request(
            "POST",
            &url,
            &[("Accept", "application/json"), ("X-Client-Token", token)],
            None,
        );

        let response = self
            .agent
            .post(&url)
            .set("Accept", "application/json")
            .set("X-Client-Token", token)
            .call()
            .map_err(map_network_error)?;

        let status = response.status();
        let body = response.into_string().map_err(|err| {
            AppError::new(
                ErrorKind::ApiData,
                format!("failed to read reopen response: {}", err),
            )
        })?;

        self.log_response(status, &body);

        // API returns an array; extract the first element
        let arr: Vec<Value> = serde_json::from_str(&body).map_err(|err| {
            AppError::new(
                ErrorKind::ApiData,
                format!("invalid JSON from task reopen: {}", err),
            )
        })?;
        arr.into_iter().next().ok_or_else(|| {
            AppError::new(ErrorKind::ApiData, String::from("empty response from task reopen"))
        })
    }

    pub fn delete_task(&self, token: &str, list_id: i64, task_id: i64) -> AppResult<()> {
        let url = format!(
            "{}/checklists/{}/tasks/{}.json",
            self.base_url, list_id, task_id
        );
        self.agent
            .delete(&url)
            .set("Accept", "application/json")
            .set("X-Client-Token", token)
            .call()
            .map_err(map_network_error)?;

        Ok(())
    }

    pub fn get_checklist_opml(&self, token: &str, list_id: i64) -> AppResult<String> {
        let url = format!(
            "{}/checklists/{}.opml?export_status=true&export_notes=true&export_details=true&export_color=true",
            self.base_url, list_id
        );
        let response = self
            .agent
            .get(&url)
            .set("Accept", "application/xml")
            .set("X-Client-Token", token)
            .call()
            .map_err(map_network_error)?;

        response.into_string().map_err(|err| {
            AppError::new(
                ErrorKind::ApiData,
                format!("invalid OPML response: {}", err),
            )
        })
    }

    pub fn get_notes(&self, token: &str, list_id: i64, task_id: i64) -> AppResult<Vec<Value>> {
        let url = format!(
            "{}/checklists/{}/tasks/{}/notes.json",
            self.base_url, list_id, task_id
        );
        let response = self
            .agent
            .get(&url)
            .set("Accept", "application/json")
            .set("X-Client-Token", token)
            .call()
            .map_err(map_network_error)?;

        let value: serde_json::Value = response.into_json().map_err(|err| {
            AppError::new(
                ErrorKind::ApiData,
                format!("invalid JSON from notes endpoint: {}", err),
            )
        })?;

        match value {
            Value::Array(items) => Ok(items),
            _ => Err(AppError::new(
                ErrorKind::ApiData,
                "expected array of notes from API",
            )),
        }
    }

    pub fn create_note(
        &self,
        token: &str,
        list_id: i64,
        task_id: i64,
        text: &str,
    ) -> AppResult<Value> {
        let url = format!(
            "{}/checklists/{}/tasks/{}/notes.json",
            self.base_url, list_id, task_id
        );
        let response = self
            .agent
            .post(&url)
            .set("Accept", "application/json")
            .set("X-Client-Token", token)
            .send_form(&[("text", text)])
            .map_err(map_network_error)?;

        response.into_json().map_err(|err| {
            AppError::new(
                ErrorKind::ApiData,
                format!("invalid JSON from note create: {}", err),
            )
        })
    }

    pub fn update_note(
        &self,
        token: &str,
        list_id: i64,
        task_id: i64,
        note_id: i64,
        text: Option<&str>,
    ) -> AppResult<Value> {
        let url = format!(
            "{}/checklists/{}/tasks/{}/notes/{}.json",
            self.base_url, list_id, task_id, note_id
        );
        let mut params: Vec<(&str, String)> = Vec::new();
        if let Some(text) = text {
            params.push(("text", text.to_string()));
        }
        let param_refs: Vec<(&str, &str)> = params.iter().map(|(k, v)| (*k, v.as_str())).collect();

        let response = self
            .agent
            .put(&url)
            .set("Accept", "application/json")
            .set("X-Client-Token", token)
            .send_form(&param_refs)
            .map_err(map_network_error)?;

        response.into_json().map_err(|err| {
            AppError::new(
                ErrorKind::ApiData,
                format!("invalid JSON from note update: {}", err),
            )
        })
    }

    pub fn delete_note(
        &self,
        token: &str,
        list_id: i64,
        task_id: i64,
        note_id: i64,
    ) -> AppResult<()> {
        let url = format!(
            "{}/checklists/{}/tasks/{}/notes/{}.json",
            self.base_url, list_id, task_id, note_id
        );
        self.agent
            .delete(&url)
            .set("Accept", "application/json")
            .set("X-Client-Token", token)
            .call()
            .map_err(map_network_error)?;

        Ok(())
    }

    pub fn auth_status(&self, token: &str) -> AppResult<Value> {
        let url = format!("{}/auth/curr_user.json", self.base_url);
        let response = self
            .agent
            .get(&url)
            .set("Accept", "application/json")
            .set("X-Client-Token", token)
            .call()
            .map_err(map_network_error)?;

        response.into_json().map_err(|err| {
            AppError::new(
                ErrorKind::ApiData,
                format!("invalid JSON from auth status: {}", err),
            )
        })
    }
}

#[derive(Debug, Deserialize)]
struct TokenResponse {
    token: String,
}

fn map_network_error(err: ureq::Error) -> AppError {
    match err {
        ureq::Error::Status(code, response) => {
            let kind = if code == 401 || code == 403 {
                ErrorKind::Auth
            } else {
                ErrorKind::ApiData
            };
            AppError::new(
                kind,
                format!("unexpected status {}: {}", code, response.status_text()),
            )
        }
        ureq::Error::Transport(inner) => {
            AppError::new(ErrorKind::Network, format!("network error: {}", inner))
        }
    }
}
