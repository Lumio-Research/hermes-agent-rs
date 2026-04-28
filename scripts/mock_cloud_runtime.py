import json
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from urllib.parse import urlparse

BRIDGE_KEY = "mock-bridge-key"
AGENT_PORT = 19091


class AgentHandler(BaseHTTPRequestHandler):
    def _send(self, code: int, payload: dict):
        body = json.dumps(payload).encode("utf-8")
        self.send_response(code)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def do_POST(self):
        p = urlparse(self.path)
        if p.path.startswith("/v1/sessions/") and p.path.endswith("/messages"):
            length = int(self.headers.get("Content-Length", "0"))
            raw = self.rfile.read(length).decode("utf-8") if length else "{}"
            req = json.loads(raw or "{}")
            text = req.get("text", "")
            self._send(200, {"reply": f"[mock-agent] 已处理: {text}", "provider_message_count": 1})
            return
        self._send(404, {"error": "not_found"})


class BridgeHandler(BaseHTTPRequestHandler):
    def _auth(self) -> bool:
        return self.headers.get("Authorization", "") == f"Bearer {BRIDGE_KEY}"

    def _send(self, code: int, payload: dict):
        body = json.dumps(payload).encode("utf-8")
        self.send_response(code)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def do_POST(self):
        if not self._auth():
            self._send(401, {"error": "unauthorized"})
            return
        p = urlparse(self.path)
        if p.path == "/v1/sandbox":
            self._send(200, {"sandbox_id": "sb-local-mock"})
            return
        if p.path == "/v1/sandbox/sb-local-mock/exec":
            self._send(200, {"stdout": "ok"})
            return
        self._send(404, {"error": "not_found"})

    def do_GET(self):
        if not self._auth():
            self._send(401, {"error": "unauthorized"})
            return
        p = urlparse(self.path)
        if p.path == "/v1/sandbox/sb-local-mock/ports/8080":
            self._send(200, {"url": f"http://127.0.0.1:{AGENT_PORT}"})
            return
        self._send(404, {"error": "not_found"})

    def do_DELETE(self):
        if not self._auth():
            self._send(401, {"error": "unauthorized"})
            return
        p = urlparse(self.path)
        if p.path == "/v1/sandbox/sb-local-mock":
            self._send(200, {"ok": True})
            return
        self._send(404, {"error": "not_found"})


def main():
    import threading

    bridge = ThreadingHTTPServer(("127.0.0.1", 19090), BridgeHandler)
    agent = ThreadingHTTPServer(("127.0.0.1", AGENT_PORT), AgentHandler)
    threading.Thread(target=agent.serve_forever, daemon=True).start()
    print("mock bridge: 127.0.0.1:19090")
    print(f"mock agent: 127.0.0.1:{AGENT_PORT}")
    bridge.serve_forever()


if __name__ == "__main__":
    main()

