use crate::api::CheckvistApi;
use crate::error::{AppResult, ErrorKind};
use crate::token_store;
use serde_json::Value;
use std::collections::{HashMap, HashSet};
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
    lists.iter().filter_map(format_list_line).collect()
}

pub fn format_list_line(item: &Value) -> Option<String> {
    let id = item.get("id")?.as_i64()?;
    let name = item.get("name")?.as_str()?;
    Some(format!("{}\t{}", id, name))
}

pub fn format_notes(notes: &[Value]) -> Vec<String> {
    notes.iter().filter_map(format_note_line).collect()
}

pub fn format_note_line(item: &Value) -> Option<String> {
    let id = item.get("id")?.as_i64()?;
    let text = item.get("text")?.as_str()?;
    Some(format!("{}\t{}", id, text))
}

pub fn format_task_tree(tasks: &[Value]) -> Vec<String> {
    let mut children: HashMap<Option<i64>, Vec<usize>> = HashMap::new();
    let mut all_ids: HashSet<i64> = HashSet::new();

    for task in tasks {
        if let Some(id) = task.get("id").and_then(|v| v.as_i64()) {
            all_ids.insert(id);
        }
    }

    for (idx, task) in tasks.iter().enumerate() {
        if let Some(_id) = task.get("id").and_then(|v| v.as_i64()) {
            let parent = task
                .get("parent_id")
                .and_then(|v| v.as_i64())
                .filter(|pid| all_ids.contains(pid));
            children.entry(parent).or_default().push(idx);
            // ensure we always have an entry for root even if child refers to missing parent
            if parent.is_none() {
                children.entry(None).or_default();
            }
        }
    }

    fn walk(
        tasks: &[Value],
        children: &HashMap<Option<i64>, Vec<usize>>,
        parent: Option<i64>,
        depth: usize,
        lines: &mut Vec<String>,
    ) {
        if let Some(indices) = children.get(&parent) {
            for idx in indices {
                if let Some(task) = tasks.get(*idx) {
                    if let (Some(id), Some(content)) = (
                        task.get("id").and_then(|v| v.as_i64()),
                        task.get("content").and_then(|v| v.as_str()),
                    ) {
                        let indent = "  ".repeat(depth);
                        let pri = if task.get("priority").and_then(|v| v.as_u64()) == Some(1) {
                            "! "
                        } else {
                            ""
                        };
                        let due = task
                            .get("due")
                            .and_then(|v| v.as_str())
                            .and_then(|d| format_due_short(d))
                            .map(|s| format!(" ^{}", s))
                            .unwrap_or_default();
                        lines.push(format!("{}{}\t{}{}{}", indent, id, pri, content, due));
                        walk(tasks, children, Some(id), depth + 1, lines);
                    }
                }
            }
        }
    }

    // root tasks: parent None or parent not found
    let mut lines = Vec::new();
    walk(tasks, &children, None, 0, &mut lines);
    lines
}

fn format_due_short(due: &str) -> Option<String> {
    let normalized = due.replace('/', "-");
    let date = chrono::NaiveDate::parse_from_str(&normalized, "%Y-%m-%d").ok()?;
    Some(date.format("%b %-d").to_string())
}
