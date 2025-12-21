use crate::api::CheckvistApi;
use crate::error::{AppResult, ErrorKind};
use crate::token_store;
use serde_json::Value;
use std::path::Path;

pub fn with_token_retry<T>(
    api: &CheckvistApi,
    token_path: &Path,
    initial_token: String,
    request: impl Fn(&CheckvistApi, &str) -> AppResult<T>,
    login: impl Fn() -> AppResult<String>,
) -> AppResult<T> {
    match request(api, &initial_token) {
        Ok(val) => Ok(val),
        Err(err) if err.kind() == ErrorKind::Auth => {
            // try refresh
            match api.refresh_token(&initial_token) {
                Ok(new_token) => {
                    token_store::write_token(token_path, &new_token)?;
                    match request(api, &new_token) {
                        Ok(val) => return Ok(val),
                        Err(err) if err.kind() == ErrorKind::Auth => {
                            // fall through to relogin
                        }
                        Err(err) => return Err(err),
                    }
                }
                Err(refresh_err) if refresh_err.kind() == ErrorKind::Auth => {
                    // proceed to relogin
                }
                Err(other) => return Err(other),
            }

            // relogin
            let new_token = login()?;
            token_store::write_token(token_path, &new_token)?;
            request(api, &new_token)
        }
        Err(err) => Err(err),
    }
}

pub fn format_lists(lists: &[Value]) -> Vec<String> {
    lists
        .iter()
        .filter_map(|item| {
            let id = item.get("id")?.as_i64()?;
            let name = item.get("name")?.as_str()?;
            Some(format!("{}\t{}", id, name))
        })
        .collect()
}
