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
from urllib.parse import parse_qs, urlparse
import json
import sys
from datetime import datetime
from threading import Lock

# In-memory state
class ServerState:
    def __init__(self):
        self.checklists = {}
        self.next_id = 0
        self.lock = Lock()

state = ServerState()

def now():
    """Return fixed timestamp for testing"""
    return "2025/12/22 20:00:00 -0800"

class CheckvistHandler(BaseHTTPRequestHandler):
    def do_POST(self):
        """Handle POST requests"""
        if self.path == '/checklists.json':
            self.create_checklist()
        else:
            self.send_error(404, "Not Found")

    def do_GET(self):
        """Handle GET requests"""
        parsed = urlparse(self.path)
        if parsed.path == '/checklists.json':
            self.get_checklists(parsed.query)
        else:
            self.send_error(404, "Not Found")

    def create_checklist(self):
        """POST /checklists.json - Create a new checklist"""
        # Check authentication
        token = self.headers.get('X-Client-Token')
        if not token:
            self.send_json_response(401, {"error": "Unauthorized"})
            return

        # Parse form data
        content_length = int(self.headers.get('Content-Length', 0))
        body = self.rfile.read(content_length).decode('utf-8')
        params = parse_qs(body)

        # Simulate real Checkvist API behavior:
        # - Expects checklist[name] parameter (nested!)
        # - If checklist[name] missing, returns "Name this list" placeholder
        name = params.get('checklist[name]', ['Name this list'])[0]
        public = params.get('checklist[public]', ['false'])[0].lower() == 'true'

        # Create checklist
        with state.lock:
            checklist_id = state.next_id
            state.next_id += 1

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
        """GET /checklists.json - List checklists"""
        # Check authentication
        token = self.headers.get('X-Client-Token')
        if not token:
            self.send_json_response(401, {"error": "Unauthorized"})
            return

        # Parse query parameters
        params = parse_qs(query_string) if query_string else {}
        archived_filter = params.get('archived', ['false'])[0].lower() == 'true'

        # Filter checklists
        with state.lock:
            checklists = [
                checklist for checklist in state.checklists.values()
                if not archived_filter or checklist['archived'] == archived_filter
            ]

        self.send_json_response(200, checklists)

    def send_json_response(self, status, data):
        """Send a JSON response"""
        self.send_response(status)
        self.send_header('Content-Type', 'application/json')
        body = json.dumps(data).encode('utf-8')
        self.send_header('Content-Length', str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def log_message(self, format, *args):
        """Override to suppress default logging (too verbose)"""
        # Only log errors
        if args[1] != '200' and args[1] != '201':
            super().log_message(format, *args)

def run_server(port=8080):
    """Start the fake server"""
    server_address = ('127.0.0.1', port)
    httpd = HTTPServer(server_address, CheckvistHandler)

    print()
    print("=" * 50)
    print("Fake Checkvist API Server")
    print("=" * 50)
    print()
    print(f"Server running at: http://127.0.0.1:{port}")
    print()
    print("Test commands:")
    print()
    print("# Create list with WRONG parameter (should return 'Name this list'):")
    print(f"curl -X POST http://127.0.0.1:{port}/checklists.json \\")
    print("  -H 'X-Client-Token: TEST' \\")
    print("  --data-urlencode 'name=My List'")
    print()
    print("# Create list with CORRECT parameter (should use provided name):")
    print(f"curl -X POST http://127.0.0.1:{port}/checklists.json \\")
    print("  -H 'X-Client-Token: TEST' \\")
    print("  --data-urlencode 'checklist[name]=My List'")
    print()
    print("# List all checklists:")
    print(f"curl -H 'X-Client-Token: TEST' http://127.0.0.1:{port}/checklists.json")
    print()
    print("# Test without auth (should return 401):")
    print(f"curl http://127.0.0.1:{port}/checklists.json")
    print()
    print("=" * 50)
    print("Press Ctrl+C to stop")
    print("=" * 50)
    print()

    try:
        httpd.serve_forever()
    except KeyboardInterrupt:
        print("\nShutting down server...")
        httpd.shutdown()

if __name__ == '__main__':
    port = int(sys.argv[1]) if len(sys.argv) > 1 else 8080
    run_server(port)
