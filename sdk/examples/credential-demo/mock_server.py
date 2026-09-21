#!/usr/bin/env python3
"""Loopback-only credential validator for the credential-demo app."""

from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import json


TOKEN_PROFILES = {
    "demo-token-a": "demo-account-a",
    "demo-token-b": "demo-account-b",
    "demo-token-replacement": "demo-account-a-rotated",
}


class Handler(BaseHTTPRequestHandler):
    def do_POST(self) -> None:
        if self.path != "/validate":
            self._json(404, {"validated": False})
            return

        authorization = self.headers.get("authorization", "")
        token = authorization.removeprefix("Bearer ")
        credential_profile = TOKEN_PROFILES.get(token)
        if credential_profile is None:
            self._json(401, {"validated": False})
            return

        optional_present = bool(self.headers.get("x-demo-account-tag", "").strip())
        self._json(
            200,
            {
                "validated": True,
                "credential_profile": credential_profile,
                "optional_credential_present": optional_present,
                "service": "local-credential-mock",
            },
        )

    def _json(self, status: int, payload: dict[str, object]) -> None:
        body = json.dumps(payload, separators=(",", ":")).encode()
        self.send_response(status)
        self.send_header("content-type", "application/json")
        self.send_header("content-length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def log_message(self, _format: str, *_args: object) -> None:
        pass


if __name__ == "__main__":
    try:
        server = ThreadingHTTPServer(("127.0.0.1", 18080), Handler)
    except OSError as error:
        raise SystemExit(f"cannot bind credential-demo mock to 127.0.0.1:18080: {error}")
    print("credential-demo mock listening on http://127.0.0.1:18080")
    try:
        server.serve_forever()
    except KeyboardInterrupt:
        pass
    finally:
        server.server_close()
