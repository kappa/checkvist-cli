use super::*;

/// Handle POST /checklists.json - Create a new checklist
///
/// Simulates real Checkvist API behavior:
/// - Expects checklist[name] parameter (nested!)
/// - If checklist[name] missing, returns "Name this list" placeholder
/// - Returns 201 Created with checklist object
pub(crate) fn create_checklist(req: &Request, state: &Arc<Mutex<ServerState>>) -> Response {
    let params = parse_form_data(&req.body);

    // Simulate real API behavior: check for nested parameter
    let name = params
        .get("checklist[name]")
        .map(|s| s.as_str())
        .unwrap_or("Name this list"); // Default placeholder if parameter missing

    let public = params
        .get("checklist[public]")
        .and_then(|s| s.parse::<bool>().ok())
        .unwrap_or(false);

    // Create checklist
    let mut state = state.lock().unwrap();
    let id = state.next_checklist_id;
    state.next_checklist_id += 1;

    let checklist = Checklist {
        id,
        name: name.to_string(),
        public,
        archived: false,
        created_at: now(),
        updated_at: now(),
        user_updated_at: now(),
        markdown: true,
        read_only: false,
        options: 2,
        user_count: 1,
        task_count: 0,
        task_completed: 0,
        percent_completed: 0,
        item_count: 0,
    };

    state.checklists.insert(id, checklist.clone());

    Response::json(201, checklist.to_json())
}

/// Handle GET /checklists.json - List checklists
///
/// Simulates real Checkvist API behavior:
/// - Returns array of checklists
/// - Supports archived parameter (filter archived lists)
/// - Returns 200 OK with array
pub(crate) fn get_checklists(req: &Request, _state: &Arc<Mutex<ServerState>>) -> Response {
    // Parse query parameters from path
    let archived_filter = req.path.contains("archived=true");

    let state = _state.lock().unwrap();

    let checklists: Vec<Value> = state
        .checklists
        .values()
        .filter(|c| !archived_filter || c.archived == archived_filter)
        .map(|c| c.to_json())
        .collect();

    Response::json(200, Value::Array(checklists))
}
