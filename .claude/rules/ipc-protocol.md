---
paths:
  - "wmux-ipc/**/*.rs"
  - "wmux-cli/**/*.rs"
---
# IPC Protocol Rules — wmux

## JSON-RPC v2 Compatibility (CRITICAL)
- Method names MUST match cmux: `workspace.list`, `surface.send_text`, `browser.navigate`, etc.
- Request format: `{"id":"...", "method":"domain.action", "params":{...}}`
- Success response: `{"id":"...", "ok":true, "result":{...}}`
- Error response: `{"id":"...", "ok":false, "error":{"code":"...", "message":"..."}}`
- Messages are newline-delimited (`\n`). One JSON object per line.
- NEVER add custom fields to the JSON-RPC envelope that cmux doesn't have.

## Security Modes (CRITICAL)
- Default mode: `wmux_only` (only child processes can connect).
- `password` mode uses HMAC-SHA256 challenge-response. Secret auto-generated in `%APPDATA%\wmux\auth_secret`.
- Unauthenticated clients can ONLY call `system.ping` and `auth.login`. All other methods require auth.
- NEVER log auth secrets or HMAC tokens — even in debug mode.

## CLI Conventions
- `wmux` CLI uses one-shot connections: connect → send → receive → disconnect.
- Pipe discovery: check `WMUX_SOCKET_PATH` env var first, fallback to `\\.\pipe\wmux-{pid}`.
- Exit code 0 on success, 1 on error. `--json` flag for machine-readable output.
