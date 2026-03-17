#!/usr/bin/env python3
"""
Fake Checkvist API server for testing checkvist-cli

This server simulates the behavior of the real Checkvist API without persistence.
It uses only Python stdlib (no external dependencies).

Usage:
    python tests/fake_server.py [port]

Default port: 8080
"""

from http.server import HTTPServer, BaseHTTPRequestHandler
from urllib.parse import parse_qs, urlparse, unquote
import json
import re
import sys
from datetime import datetime
from threading import Lock


class ServerState:
    """In-memory state for the fake server."""

    def __init__(self):
        self.checklists = {}
        self.tasks = {}       # keyed by (list_id, task_id)
        self.notes = {}       # keyed by (list_id, task_id, note_id)
        self.next_checklist_id = 1
        self.next_task_id = 1
        self.next_note_id = 1
        self.lock = Lock()
        # Valid token for auth checks
        self.valid_token = "TEST_TOKEN"
        self.refreshed_token = "REFRESHED_TOKEN"


def now():
    """Return fixed timestamp for deterministic testing."""
    return "2025/12/22 20:00:00 +0000"


def extract_tags_from_content(content):
    """Extract #hashtags from content, simulating Checkvist create behavior."""
    tags = {}
    tag_pattern = re.compile(r'#(\w+)')
    for match in tag_pattern.finditer(content):
        tags[match.group(1)] = False
    # Remove tags from content
    cleaned = tag_pattern.sub('', content).strip()
    # Collapse multiple spaces
    cleaned = re.sub(r'\s+', ' ', cleaned)
    return cleaned, tags


def extract_due_from_content(content):
    """Extract ^date from content, simulating Checkvist smart syntax."""
    due = None
    due_pattern = re.compile(r'\^(\d{4}[/-]\d{2}[/-]\d{2})')
    match = due_pattern.search(content)
    if match:
        due_str = match.group(1).replace('-', '/')
        due = due_str
        content = due_pattern.sub('', content).strip()
        content = re.sub(r'\s+', ' ', content)
    return content, due


def make_task(state, list_id, content, parent_id=0, priority=None,
              status=0, tags=None, due=None, parse_content=True):
    """Create a task dict and register it in state."""
    task_tags = tags or {}

    if parse_content:
        content, extracted_tags = extract_tags_from_content(content)
        task_tags.update(extracted_tags)
        content, extracted_due = extract_due_from_content(content)
        if extracted_due and due is None:
            due = extracted_due

    task_id = state.next_task_id
    state.next_task_id += 1

    task = {
        'id': task_id,
        'checklist_id': list_id,
        'parent_id': parent_id,
        'content': content,
        'position': len([t for t in state.tasks.values()
                        if t['checklist_id'] == list_id and t['parent_id'] == parent_id]) + 1,
        'status': status,
        'priority': priority,
        'due': due,
        'collapsed': False,
        'comments_count': 0,
        'assignee_ids': [],
        'details': {},
        'link_ids': [],
        'backlink_ids': [],
        'tags': task_tags,
        'tags_as_text': ', '.join(sorted(task_tags.keys())) if task_tags else '',
        'tasks': [],
        'created_at': now(),
        'updated_at': now(),
        'update_line': 'created by test user',
    }

    if priority and priority > 0:
        task['color'] = {'text': 'red'} if priority == 1 else {}
        task['details']['mark'] = f'fg{priority}'

    state.tasks[(list_id, task_id)] = task

    # Update parent's tasks list
    for key, t in state.tasks.items():
        if key[0] == list_id and t['id'] == parent_id:
            if task_id not in t['tasks']:
                t['tasks'].append(task_id)
            break

    return task


class CheckvistHandler(BaseHTTPRequestHandler):

    def do_POST(self):
        parsed = urlparse(self.path)
        path = parsed.path

        # Auth endpoints
        if path == '/auth/login.json':
            return self.handle_login()
        if path == '/auth/refresh_token.json':
            return self.handle_refresh_token()

        # Checklist create
        if path == '/checklists.json':
            return self.create_checklist()

        # Task endpoints
        m = re.match(r'^/checklists/(\d+)/tasks\.json$', path)
        if m:
            return self.create_task(int(m.group(1)))

        # Task close/reopen
        m = re.match(r'^/checklists/(\d+)/tasks/(\d+)/(close|reopen|invalidate)\.json$', path)
        if m:
            return self.task_status_action(int(m.group(1)), int(m.group(2)), m.group(3))

        # Note create
        m = re.match(r'^/checklists/(\d+)/tasks/(\d+)/notes\.json$', path)
        if m:
            return self.create_note(int(m.group(1)), int(m.group(2)))

        self.send_error(404, "Not Found")

    def do_GET(self):
        parsed = urlparse(self.path)
        path = parsed.path

        # Auth status
        if path == '/auth/curr_user.json':
            return self.handle_auth_status()

        # List checklists
        if path == '/checklists.json':
            return self.get_checklists(parsed.query)

        # Get single checklist
        m = re.match(r'^/checklists/(\d+)\.json$', path)
        if m:
            return self.get_checklist(int(m.group(1)))

        # Get tasks
        m = re.match(r'^/checklists/(\d+)/tasks\.json$', path)
        if m:
            return self.get_tasks(int(m.group(1)))

        # Get single task (returns task + parents)
        m = re.match(r'^/checklists/(\d+)/tasks/(\d+)\.json$', path)
        if m:
            return self.get_task(int(m.group(1)), int(m.group(2)))

        # Get notes for task
        m = re.match(r'^/checklists/(\d+)/tasks/(\d+)/notes\.json$', path)
        if m:
            return self.get_notes(int(m.group(1)), int(m.group(2)))

        # OPML export for a checklist
        m = re.match(r'^/checklists/(\d+)\.opml$', path)
        if m:
            return self.get_checklist_opml(int(m.group(1)))

        self.send_error(404, "Not Found")

    def do_PUT(self):
        parsed = urlparse(self.path)
        path = parsed.path

        # Update checklist
        m = re.match(r'^/checklists/(\d+)\.json$', path)
        if m:
            return self.update_checklist(int(m.group(1)))

        # Update task
        m = re.match(r'^/checklists/(\d+)/tasks/(\d+)\.json$', path)
        if m:
            return self.update_task(int(m.group(1)), int(m.group(2)))

        # Update note
        m = re.match(r'^/checklists/(\d+)/tasks/(\d+)/notes/(\d+)\.json$', path)
        if m:
            return self.update_note(int(m.group(1)), int(m.group(2)), int(m.group(3)))

        self.send_error(404, "Not Found")

    def do_DELETE(self):
        parsed = urlparse(self.path)
        path = parsed.path

        # Delete checklist
        m = re.match(r'^/checklists/(\d+)\.json$', path)
        if m:
            return self.delete_checklist(int(m.group(1)))

        # Delete task
        m = re.match(r'^/checklists/(\d+)/tasks/(\d+)\.json$', path)
        if m:
            return self.delete_task(int(m.group(1)), int(m.group(2)))

        # Delete note
        m = re.match(r'^/checklists/(\d+)/tasks/(\d+)/notes/(\d+)\.json$', path)
        if m:
            return self.delete_note(int(m.group(1)), int(m.group(2)), int(m.group(3)))

        self.send_error(404, "Not Found")

    # ---- Helpers ----

    def check_auth(self):
        """Check X-Client-Token header. Returns True if valid.

        Accepts any non-empty token value for simplicity.
        The auth endpoints (login, refresh) issue known tokens,
        but we don't reject unknown tokens here — the fake server
        is for behavior testing, not auth security testing.
        """
        token = self.headers.get('X-Client-Token')
        if not token:
            self.send_json_response(401, {"message": "Unauthenticated: no valid authentication data in request"})
            return False
        return True

    def read_form_params(self):
        """Read and parse URL-encoded form body."""
        content_length = int(self.headers.get('Content-Length', 0))
        body = self.rfile.read(content_length).decode('utf-8')
        return parse_qs(body)

    def send_json_response(self, status_code, data):
        """Send a JSON response."""
        self.send_response(status_code)
        self.send_header('Content-Type', 'application/json')
        body = json.dumps(data, ensure_ascii=False).encode('utf-8')
        self.send_header('Content-Length', str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    # ---- Auth ----

    def handle_login(self):
        params = self.read_form_params()
        username = params.get('username', [None])[0]
        remote_key = params.get('remote_key', [None])[0]
        if not username or not remote_key:
            self.send_json_response(403, {"message": "Forbidden"})
            return
        self.send_json_response(200, {"token": state.valid_token})

    def handle_refresh_token(self):
        token = self.headers.get('X-Client-Token')
        if not token:
            self.send_json_response(403, {"message": "Forbidden"})
            return
        self.send_json_response(200, {"token": state.refreshed_token})

    def handle_auth_status(self):
        if not self.check_auth():
            return
        self.send_json_response(200, {
            "user": {
                "id": 1,
                "username": "test@example.com",
                "email": "test@example.com",
                "pro": False,
            }
        })

    # ---- Checklists ----

    def create_checklist(self):
        if not self.check_auth():
            return
        params = self.read_form_params()
        name = params.get('checklist[name]', ['Name this list'])[0]
        public = params.get('checklist[public]', ['false'])[0].lower() == 'true'

        with state.lock:
            checklist_id = state.next_checklist_id
            state.next_checklist_id += 1
            checklist = {
                'id': checklist_id,
                'name': name,
                'public': public,
                'archived': False,
                'created_at': now(),
                'updated_at': now(),
                'user_updated_at': now(),
                'markdown?': True,
                'read_only': False,
                'options': 2,
                'user_count': 1,
                'task_count': 0,
                'task_completed': 0,
                'percent_completed': 0,
                'item_count': 0,
                'tags': {},
                'tags_as_text': '',
                'related_task_ids': None,
            }
            state.checklists[checklist_id] = checklist

        self.send_json_response(201, checklist)

    def get_checklists(self, query_string):
        if not self.check_auth():
            return
        params = parse_qs(query_string) if query_string else {}
        archived_filter = params.get('archived', ['false'])[0].lower() == 'true'

        with state.lock:
            checklists = [
                cl for cl in state.checklists.values()
                if not archived_filter or cl['archived'] == archived_filter
            ]
        self.send_json_response(200, checklists)

    def get_checklist(self, list_id):
        if not self.check_auth():
            return
        with state.lock:
            cl = state.checklists.get(list_id)
        if cl is None:
            self.send_json_response(404, {"message": "Not found"})
        else:
            self.send_json_response(200, cl)

    def get_checklist_opml(self, list_id):
        if not self.check_auth():
            return
        with state.lock:
            cl = state.checklists.get(list_id)
        if cl is None:
            self.send_error(404, "Not Found")
            return
        # Generate a simple OPML representation
        name = cl.get('name', 'Untitled')
        tasks = []
        with state.lock:
            for key, t in state.tasks.items():
                if key[0] == list_id:
                    tasks.append(t)
        opml_items = ''
        for t in tasks:
            content = t.get('content', '')
            status_attr = ' _status="1"' if t.get('status', 0) == 1 else ''
            opml_items += '      <outline text="{}"{}/>\n'.format(
                content.replace('&', '&amp;').replace('"', '&quot;').replace('<', '&lt;'),
                status_attr
            )
        opml = ('<?xml version="1.0"?>\n'
                '<opml version="2.0">\n'
                '  <head>\n'
                '    <title>{}</title>\n'
                '  </head>\n'
                '  <body>\n'
                '{}'
                '  </body>\n'
                '</opml>').format(
                    name.replace('&', '&amp;').replace('"', '&quot;').replace('<', '&lt;'),
                    opml_items)
        self.send_response(200)
        self.send_header('Content-Type', 'application/xml')
        data = opml.encode('utf-8')
        self.send_header('Content-Length', str(len(data)))
        self.end_headers()
        self.wfile.write(data)

    def update_checklist(self, list_id):
        if not self.check_auth():
            return
        params = self.read_form_params()

        with state.lock:
            cl = state.checklists.get(list_id)
            if cl is None:
                self.send_json_response(404, {"message": "Not found"})
                return
            if 'checklist[name]' in params:
                cl['name'] = params['checklist[name]'][0]
            if 'archived' in params:
                cl['archived'] = params['archived'][0].lower() == 'true'
            if 'public' in params:
                cl['public'] = params['public'][0].lower() == 'true'
            cl['updated_at'] = now()

        self.send_json_response(200, cl)

    def delete_checklist(self, list_id):
        if not self.check_auth():
            return
        with state.lock:
            cl = state.checklists.pop(list_id, None)
            if cl is None:
                self.send_json_response(404, {"message": "Not found"})
                return
            # Remove associated tasks and notes
            to_remove_tasks = [k for k in state.tasks if k[0] == list_id]
            for k in to_remove_tasks:
                del state.tasks[k]
            to_remove_notes = [k for k in state.notes if k[0] == list_id]
            for k in to_remove_notes:
                del state.notes[k]

        self.send_json_response(200, cl)

    # ---- Tasks ----

    def get_tasks(self, list_id):
        if not self.check_auth():
            return
        with state.lock:
            tasks = [
                t for t in state.tasks.values()
                if t['checklist_id'] == list_id
            ]
            # Sort by parent_id, position (like real API)
            tasks.sort(key=lambda t: (t['parent_id'], t['position']))
        self.send_json_response(200, tasks)

    def get_task(self, list_id, task_id):
        if not self.check_auth():
            return
        with state.lock:
            task = state.tasks.get((list_id, task_id))
            if task is None:
                self.send_json_response(404, {"message": "Not found"})
                return
            # Return task + parents chain
            result = [task]
            current = task
            while current['parent_id'] != 0:
                parent = state.tasks.get((list_id, current['parent_id']))
                if parent:
                    result.append(parent)
                    current = parent
                else:
                    break
        self.send_json_response(200, result)

    def create_task(self, list_id):
        if not self.check_auth():
            return
        with state.lock:
            if list_id not in state.checklists:
                self.send_json_response(404, {"message": "Checklist not found"})
                return

        params = self.read_form_params()
        content = params.get('task[content]', [''])[0]
        parent_id = int(params.get('task[parent_id]', ['0'])[0])
        priority_str = params.get('task[priority]', [None])[0]
        priority = int(priority_str) if priority_str else None
        tags_str = params.get('task[tags]', [None])[0]
        due_str = params.get('task[due_date]', [None])[0]

        tags = {}
        if tags_str:
            for tag in tags_str.split(','):
                tag = tag.strip()
                if tag:
                    tags[tag] = False

        with state.lock:
            task = make_task(state, list_id, content, parent_id=parent_id,
                           priority=priority, tags=tags, due=due_str,
                           parse_content=True)

        self.send_json_response(201, task)

    def update_task(self, list_id, task_id):
        if not self.check_auth():
            return
        params = self.read_form_params()
        do_parse = params.get('parse', ['false'])[0].lower() == 'true'

        with state.lock:
            task = state.tasks.get((list_id, task_id))
            if task is None:
                self.send_json_response(404, {"message": "Not found"})
                return

            if 'task[content]' in params:
                content = params['task[content]'][0]
                if do_parse:
                    content, extracted_tags = extract_tags_from_content(content)
                    task['tags'].update(extracted_tags)
                    task['tags_as_text'] = ', '.join(sorted(task['tags'].keys())) if task['tags'] else ''
                    content, extracted_due = extract_due_from_content(content)
                    if extracted_due:
                        task['due'] = extracted_due
                task['content'] = content

            if 'task[status]' in params:
                status_str = params['task[status]'][0]
                if status_str in ('0', 'open'):
                    task['status'] = 0
                elif status_str in ('1', 'done'):
                    task['status'] = 1
                elif status_str in ('2', 'invalidated'):
                    task['status'] = 2

            if 'task[parent_id]' in params:
                task['parent_id'] = int(params['task[parent_id]'][0])

            if 'task[priority]' in params:
                p = int(params['task[priority]'][0])
                task['priority'] = p if p > 0 else None

            if 'task[tags]' in params:
                tags_str = params['task[tags]'][0]
                task['tags'] = {}
                for tag in tags_str.split(','):
                    tag = tag.strip()
                    if tag:
                        task['tags'][tag] = False
                task['tags_as_text'] = ', '.join(sorted(task['tags'].keys())) if task['tags'] else ''

            if 'task[due_date]' in params:
                task['due'] = params['task[due_date]'][0]

            task['updated_at'] = now()
            task['update_line'] = 'edited by test user'

        self.send_json_response(200, task)

    def task_status_action(self, list_id, task_id, action):
        """Handle POST .../close.json, .../reopen.json, .../invalidate.json"""
        if not self.check_auth():
            return
        with state.lock:
            task = state.tasks.get((list_id, task_id))
            if task is None:
                self.send_json_response(404, {"message": "Not found"})
                return

            if action == 'close':
                task['status'] = 1
            elif action == 'reopen':
                task['status'] = 0
            elif action == 'invalidate':
                task['status'] = 2

            task['updated_at'] = now()
            task['update_line'] = f'{action}d by test user'

            # Return task + children
            result = [task]
            children = [t for t in state.tasks.values()
                       if t['checklist_id'] == list_id and t['parent_id'] == task_id]
            result.extend(children)

        self.send_json_response(200, result)

    def delete_task(self, list_id, task_id):
        if not self.check_auth():
            return
        with state.lock:
            task = state.tasks.pop((list_id, task_id), None)
            if task is None:
                self.send_json_response(404, {"message": "Not found"})
                return
            # Remove children recursively
            children_to_remove = [k for k in state.tasks
                                 if k[0] == list_id and state.tasks[k]['parent_id'] == task_id]
            for k in children_to_remove:
                del state.tasks[k]
            # Remove associated notes
            notes_to_remove = [k for k in state.notes if k[0] == list_id and k[1] == task_id]
            for k in notes_to_remove:
                del state.notes[k]

        self.send_json_response(200, task)

    # ---- Notes ----

    def get_notes(self, list_id, task_id):
        if not self.check_auth():
            return
        with state.lock:
            notes = [
                n for n in state.notes.values()
                if n['checklist_id'] == list_id and n['task_id'] == task_id
            ]
            notes.sort(key=lambda n: n['id'])
        self.send_json_response(200, notes)

    def create_note(self, list_id, task_id):
        if not self.check_auth():
            return
        with state.lock:
            task = state.tasks.get((list_id, task_id))
            if task is None:
                self.send_json_response(404, {"message": "Task not found"})
                return

        params = self.read_form_params()
        # CLI sends "text" param, but API docs say "comment[comment]"
        # Support both for robustness
        text = params.get('text', params.get('comment[comment]', ['']))[0]

        with state.lock:
            note_id = state.next_note_id
            state.next_note_id += 1

            note = {
                'id': note_id,
                'checklist_id': list_id,
                'task_id': task_id,
                'text': text,
                'text_html': f'<p>{text}</p>',
                'created_at': now(),
                'updated_at': now(),
                'username': 'test@example.com',
            }
            state.notes[(list_id, task_id, note_id)] = note

            # Update task comments_count
            task = state.tasks.get((list_id, task_id))
            if task:
                task['comments_count'] = len([
                    n for n in state.notes.values()
                    if n['checklist_id'] == list_id and n['task_id'] == task_id
                ])

        self.send_json_response(201, note)

    def update_note(self, list_id, task_id, note_id):
        if not self.check_auth():
            return
        params = self.read_form_params()

        with state.lock:
            note = state.notes.get((list_id, task_id, note_id))
            if note is None:
                self.send_json_response(404, {"message": "Note not found"})
                return

            text = params.get('text', params.get('comment[comment]', [None]))[0]
            if text is not None:
                note['text'] = text
                note['text_html'] = f'<p>{text}</p>'
            note['updated_at'] = now()

        self.send_json_response(200, note)

    def delete_note(self, list_id, task_id, note_id):
        if not self.check_auth():
            return
        with state.lock:
            note = state.notes.pop((list_id, task_id, note_id), None)
            if note is None:
                self.send_json_response(404, {"message": "Note not found"})
                return

            # Update task comments_count
            task = state.tasks.get((list_id, task_id))
            if task:
                task['comments_count'] = len([
                    n for n in state.notes.values()
                    if n['checklist_id'] == list_id and n['task_id'] == task_id
                ])

        self.send_json_response(200, note)

    # ---- Logging ----

    def log_message(self, format, *args):
        """Suppress default logging unless errors."""
        pass


def run_server(port=8080):
    """Start the fake server."""
    global state
    state = ServerState()

    server_address = ('127.0.0.1', port)
    httpd = HTTPServer(server_address, CheckvistHandler)

    # Print a ready signal for test harnesses
    print(f"READY:{port}", flush=True)

    try:
        httpd.serve_forever()
    except KeyboardInterrupt:
        httpd.shutdown()


if __name__ == '__main__':
    port = int(sys.argv[1]) if len(sys.argv) > 1 else 8080
    run_server(port)
