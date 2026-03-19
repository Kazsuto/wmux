---
paths:
  - "wmux-core/src/persistence*"
  - "wmux-config/**/*.rs"
---
# Persistence & Config Rules — wmux

## Session Persistence (CRITICAL)
- Session state serialized as JSON to `%APPDATA%\wmux\session.json`. Use serde.
- **ALWAYS** include a schema version field (`"version": 1`) at the root. Check version on load — if incompatible, start fresh instead of crashing.
- Auto-save every 8 seconds via tokio interval. Save is non-blocking: serialize on main, write on spawned task.
- Scrollback limit per terminal: 4000 lines / 400K chars max. Truncate before serializing.
- If session file is corrupt or unreadable, log warning and start fresh — NEVER crash.

## Config Files
- Config at `%APPDATA%\wmux\config`. Ghostty-compatible format (`key = value`, `#` comments).
- Themes at `%APPDATA%\wmux\themes/`. Bundle 10-20 popular themes in resources.
- NEVER store secrets (auth_secret) in the config file — separate file with restricted permissions.
