use crate::error::{AppError, AppResult, ErrorKind};
use serde::Deserialize;
use serde_json::Value;
use std::time::Duration;
use ureq::{Agent, AgentBuilder};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Order {
    IdAsc,
    IdDesc,
    UpdatedAtAsc,
}

impl Order {
    fn as_query(&self) -> &'static str {
        match self {
            Order::IdAsc => "id:asc",
            Order::IdDesc => "id:desc",
            Order::UpdatedAtAsc => "updated_at:asc",
        }
    }
}

pub struct CheckvistApi {
    base_url: String,
    agent: Agent,
}

impl CheckvistApi {
    pub fn new(base_url: String) -> Self {
        let agent = AgentBuilder::new()
            .timeout_connect(Duration::from_secs(5))
            .timeout_read(Duration::from_secs(20))
            .timeout_write(Duration::from_secs(20))
            .build();

        Self {
            base_url: base_url.trim_end_matches('/').to_string(),
            agent,
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

        let response = self
            .agent
            .post(&url)
            .set("Accept", "application/json")
            .send_form(&params)
            .map_err(map_network_error)?;

        let parsed: TokenResponse = response.into_json().map_err(|err| {
            AppError::new(
                ErrorKind::ApiData,
                format!("invalid login response: {}", err),
            )
        })?;
        Ok(parsed.token)
    }

    pub fn refresh_token(&self, token: &str) -> AppResult<String> {
        let url = format!("{}/auth/refresh_token.json?version=2", self.base_url);
        let response = self
            .agent
            .post(&url)
            .set("Accept", "application/json")
            .set("X-Client-Token", token)
            .call()
            .map_err(map_network_error)?;

        let parsed: TokenResponse = response.into_json().map_err(|err| {
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
                format!("invalid JSON from lists endpoint: {}", err),
            )
        })?;

        match value {
            Value::Array(items) => Ok(items),
            _ => Err(AppError::new(
                ErrorKind::ApiData,
                "expected array of lists from API",
            )),
        }
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
