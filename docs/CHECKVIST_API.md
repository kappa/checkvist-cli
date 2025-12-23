# Checkvist Open API Documentation

**Source:** https://checkvist.com/auth/api
**Fetched:** 2025-12-22

## Overview

Checkvist provides a REST-based API supporting both XML and JSON responses. The API enables read and write operations for integration with external tools via Zapier or direct implementation.

## Authentication

### Token-Based Authentication (Recommended)
Obtain a token via `/auth/login.json?version=2` with username and remote API key, then pass it as a `token` parameter or `X-Client-Token` header.

**Token validity:** 1 day (refreshable for 90 days)

### Basic HTTP Authentication
Use user credentials or Remote API Key obtained from the user profile page.

### Two-Factor Verification
When 2FA is enabled, include the `token2fa` parameter containing the authentication app code.

## Core Endpoints

### User Information

#### Get Current User
```
GET /auth/curr_user.(json|xml)
```

**Authentication:** Required (token or HTTP auth)

**Response:** User object with username, email, etc.

---

### Checklists

#### List User Checklists
```
GET /checklists.(json|xml)
```

**Authentication:** Required (token or HTTP auth)

**Parameters:**
- `archived` (optional): boolean - if true, return only archived lists
- `order` (optional): string - sorting order
- `skip_stats` (optional): boolean - skip statistics for performance

**Response:** Array of checklist objects

**Checklist fields:**
- `id`: integer - checklist ID
- `name`: string - checklist name
- `public`: boolean - whether checklist is public
- `role`: string - user's role (owner, editor, viewer)
- `updated_at`: timestamp - last update time
- `user_updated_at`: timestamp - last update by user
- `created_at`: timestamp - creation time
- `tags`: object - tags used in the checklist
- `task_count`: integer - total number of tasks
- `task_completed`: integer - number of completed tasks
- `percent_completed`: integer - completion percentage
- `read_only`: boolean - whether user has write access
- `archived`: boolean - whether list is archived
- `markdown?`: boolean - whether markdown is enabled
- `options`: integer - option flags
- `user_count`: integer - number of users with access

#### Get Specific Checklist
```
GET /checklists/{id}.(json|xml)
```

**Authentication:** Required

**Parameters:**
- `{id}`: integer - checklist ID

**Response:** Single checklist object

#### Create Checklist
```
POST /checklists.(json|xml)
```

**Authentication:** Required

**Parameters (form-encoded):**
- `checklist[name]`: string - checklist name (REQUIRED)
- `checklist[public]`: boolean - make checklist public (optional)

**Important:** Parameters must be nested under `checklist[...]` prefix!

**Response:** Created checklist object with HTTP 201 status

**Example:**
```bash
curl -X POST "https://checkvist.com/checklists.json" \
  -H "X-Client-Token: YOUR_TOKEN" \
  --data-urlencode "checklist[name]=My New List" \
  --data-urlencode "checklist[public]=false"
```

#### Update Checklist
```
PUT /checklists/{id}.(json|xml)
```

**Authentication:** Required

**Parameters (form-encoded):**
- `checklist[name]`: string - new name (optional)
- `checklist[public]`: boolean - public flag (optional)
- `archived`: boolean - archive status (optional)

**Response:** Updated checklist object

#### Delete Checklist
```
DELETE /checklists/{id}.(json|xml)
```

**Authentication:** Required

**Parameters:**
- `{id}`: integer - checklist ID

**Response:** HTTP 200 on success

**Note:** List must be empty to delete

---

### Tasks/List Items

#### List All Tasks
```
GET /checklists/{id}/tasks.(json|xml)
```

**Authentication:** Required

**Parameters:**
- `{id}`: integer - checklist ID

**Response:** Array of task objects

**Task fields:**
- `id`: integer - task ID
- `content`: string - task content/text
- `status`: integer - 0=open, 1=closed, 2=invalidated
- `checklist_id`: integer - parent checklist ID
- `parent_id`: integer - parent task ID (for subtasks)
- `position`: integer - position in list
- `comments_count`: integer - number of notes/comments
- `priority`: integer - task priority (0-3)
- `assignee_ids`: array - user IDs assigned to task
- `tags`: string - comma-separated tags
- `due`: string - due date
- `updated_at`: timestamp - last update time
- `created_at`: timestamp - creation time

#### Get Specific Task
```
GET /checklists/{id}/tasks/{task_id}.(json|xml)
```

**Authentication:** Required

**Parameters:**
- `{id}`: integer - checklist ID
- `{task_id}`: integer - task ID

**Response:** Task object with parent hierarchy

#### Create Task
```
POST /checklists/{id}/tasks.(json|xml)
```

**Authentication:** Required

**Parameters (form-encoded):**
- `task[content]`: string - task content (REQUIRED)
- `task[parent_id]`: integer - parent task ID (optional, for subtasks)
- `task[tags]`: string - comma-separated tags (optional)
- `task[due]`: string - due date (optional)
- `task[position]`: integer - position in list (optional)
- `task[priority]`: integer - priority 0-3 (optional)
- `task[status]`: integer - initial status (optional)

**Important:** Parameters must be nested under `task[...]` prefix!

**Response:** Created task object

#### Bulk Import Tasks
```
POST /checklists/{id}/import.(json|xml)
```

**Authentication:** Required

**Parameters:**
- `import_content`: text - multi-line task content
- Advanced options available (see API docs)

**Response:** Import status

#### Update Task
```
PUT /checklists/{id}/tasks/{task_id}.(json|xml)
```

**Authentication:** Required

**Parameters (form-encoded):**
- `task[content]`: string - new content (optional)
- `task[parent_id]`: integer - move to new parent (optional)
- `task[status]`: integer - new status (optional)
- Other task fields as needed

**Response:** Updated task object

#### Change Task Status
```
POST /checklists/{id}/tasks/{task_id}/{action}.(json|xml)
```

**Authentication:** Required

**Actions:**
- `close` - mark as completed
- `reopen` - mark as open
- `invalidate` - mark as invalid

**Response:** Updated task object

#### Set Repeating Schedule
```
POST /checklists/{id}/tasks/{task_id}/repeat.(json|xml)
```

**Authentication:** Required

**Parameters:** Schedule configuration

**Response:** Task with repeat schedule

#### Delete Task
```
DELETE /checklists/{id}/tasks/{task_id}.(json|xml)
```

**Authentication:** Required

**Response:** HTTP 200 on success

---

### Notes/Comments

#### Get Task Notes
```
GET /checklists/{id}/tasks/{task_id}/comments.(json|xml)
```

**Authentication:** Required

**Response:** Array of note/comment objects

**Note fields:**
- `id`: integer - note ID
- `comment`: string - note text
- `task_id`: integer - parent task ID
- `user_id`: integer - author user ID
- `username`: string - author username
- `created_at`: timestamp - creation time
- `updated_at`: timestamp - last update time

#### Create Note
```
POST /checklists/{id}/tasks/{task_id}/comments.(json|xml)
```

**Authentication:** Required

**Parameters (form-encoded):**
- `comment[comment]`: string - note text (REQUIRED)

**Important:** Parameter must be nested under `comment[...]` prefix!

**Response:** Created note object

#### Update Note
```
PUT /checklists/{id}/tasks/{task_id}/comments/{note_id}.(json|xml)
```

**Authentication:** Required

**Parameters:**
- `comment[comment]`: string - new note text

**Response:** Updated note object

#### Delete Note
```
DELETE /checklists/{id}/tasks/{task_id}/comments/{note_id}.(json|xml)
```

**Authentication:** Required

**Response:** HTTP 200 on success

---

## Important Implementation Notes

### Parameter Nesting

**CRITICAL:** The Checkvist API uses Rails-style nested parameters. Most POST/PUT endpoints require parameters to be nested under a resource name:

- Checklist operations: `checklist[name]`, `checklist[public]`, etc.
- Task operations: `task[content]`, `task[parent_id]`, etc.
- Note operations: `comment[comment]`, etc.

**Incorrect (won't work):**
```
name=My List
content=My Task
```

**Correct:**
```
checklist[name]=My List
task[content]=My Task
comment[comment]=My Note
```

### Form Encoding

All POST/PUT requests should use `application/x-www-form-urlencoded` content type.

### Authentication Header

Preferred method: `X-Client-Token: YOUR_TOKEN`

Alternative: `?token=YOUR_TOKEN` query parameter

### Response Status Codes

- `200 OK` - Successful GET/PUT/DELETE
- `201 Created` - Successful POST (resource created)
- `400 Bad Request` - Invalid parameters
- `401 Unauthorized` - Missing or invalid credentials
- `403 Forbidden` - Insufficient permissions
- `404 Not Found` - Resource doesn't exist
- `500 Internal Server Error` - Server error

---

## Testing Notes

When implementing the fake server:

1. **Respect parameter nesting** - Check for `checklist[name]`, not `name`
2. **Return defaults** - If required parameter missing, return default placeholder
3. **Generate IDs** - Use incrementing integers for simplicity
4. **No persistence** - In-memory only, reset between tests
5. **Simulate errors** - Invalid tokens, missing params, etc.

---

**For full details and advanced features, see:** https://checkvist.com/auth/api
