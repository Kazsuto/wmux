# ADR-0009: Session Persistence — JSON File with Auto-save

> **Status**: Accepted
> **Date**: 2026-03-19
> **Confidence**: High
> **Deciders**: wmux team

## Context

wmux must persist session state across application restarts: workspace names and order, pane tree layouts (split directions, ratios), terminal working directories, scrollback text (up to 4K lines per terminal), browser URLs, sidebar metadata, and window geometry. The persistence mechanism must be reliable (no data loss), fast (< 3s restore for 10 workspaces), and simple (no server, no migration tooling).

## Decision Drivers

- Desktop application with local-only persistence — no server, no cloud sync
- Session file is written every 8 seconds (auto-save) — writes must be fast and non-blocking
- Must handle corruption gracefully (crash mid-write)
- Schema must be versionable (future session format changes)
- Human-readable format preferred for debugging and manual recovery
- No external database runtime (SQLite DLL, etc.) — single binary distribution

## Decision

**JSON file** at `%APPDATA%\wmux\session.json`. Auto-saved every 8 seconds via the persistence actor (serialize on main thread, write via `tokio::spawn`). Atomic writes via temp file + rename pattern.

Write strategy:
1. Serialize state to JSON string (main thread, < 5ms for 10 workspaces)
2. Write to `session.tmp` (async I/O via tokio)
3. Rename `session.tmp` → `session.json` (atomic on NTFS)
4. On read failure: log warning, start fresh session (never crash)

## Alternatives Considered

### SQLite (via rusqlite)
- **Pros**: ACID transactions. Handles concurrent reads/writes. Partial updates (update one workspace without rewriting everything). FTS for scrollback search. Well-tested corruption recovery
- **Cons**: Requires shipping SQLite DLL (or static linking adds ~1MB). Schema migrations needed for format changes. Overkill for a single-writer desktop app. More complex debugging (need SQLite tools to inspect)
- **Why rejected**: wmux has exactly one writer (the persistence actor) and one reader (startup restore). SQLite's concurrency benefits are unnecessary. The added binary size (~1MB) and migration complexity don't justify the benefits for a local session file. If scrollback search needs FTS, it can be added later without changing the persistence layer

### Binary format (bincode / MessagePack)
- **Pros**: Faster serialization (~3x vs JSON). Smaller file size (~50% of JSON). No parsing overhead
- **Cons**: Not human-readable — cannot inspect or manually edit session files. Format changes require version-aware deserializers (binary formats are fragile across versions). Debugging is harder (need custom tools)
- **Why rejected**: Human readability is important for debugging ("why did my session restore wrong?"). JSON's overhead is acceptable — serializing 10 workspaces with 4K-line scrollback takes ~20ms, well within the 8s save interval. If file size becomes an issue (large scrollback), compress with `flate2` while keeping JSON as the underlying format

### Memory-mapped file (memmap2)
- **Pros**: Zero-copy reads. OS handles write-back. No explicit save needed
- **Cons**: Complex state layout in fixed memory. No schema versioning. Corruption risk if process crashes mid-write (no atomic semantics). Platform-specific behavior. Cannot easily add new fields
- **Why rejected**: Memory-mapped files are designed for high-frequency random access — session persistence is write-every-8s, read-once-at-startup. The complexity and corruption risk far outweigh the performance benefit

## Consequences

### Positive
- Human-readable session file — users can inspect, debug, and even manually edit
- Atomic writes (temp + rename) prevent corruption on crash
- Schema versioning via `"version": 1` field — incompatible versions start fresh gracefully
- serde_json is already a dependency (used by IPC protocol) — no additional binary size
- Simple implementation: ~100 lines of Rust for the full persistence actor

### Negative (acknowledged trade-offs)
- JSON is verbose — 10 workspaces with 4K-line scrollback produces a ~5-10MB file. Acceptable for modern SSDs
- Full rewrite every 8 seconds (no partial updates) — acceptable for session-sized data (< 10MB)
- Scrollback text doubles memory usage (in-memory grid + serialized JSON string) during save — mitigated by serializing in chunks
- No concurrent access — if a second wmux instance writes to the same file, last write wins

### Mandatory impact dimensions
- **Security**: Session file contains terminal scrollback — may include sensitive output (commands, API keys visible in terminal). File is stored in user's `%APPDATA%` with default user-only permissions. Users are responsible for disk encryption if needed. No passwords or auth tokens stored by design
- **Cost**: $0. serde_json is already a dependency. File I/O is free
- **Latency**: JSON serialization: ~5ms for 10 workspaces (no scrollback), ~20ms with scrollback. File write: ~2ms on SSD. Total < 25ms, non-blocking (runs in tokio task)

## Revisit Triggers

- If session file exceeds 50MB (very large scrollback across many workspaces), investigate compressed JSON (flate2) or truncating scrollback before serialization
- If restore time exceeds 3s for 10 workspaces, profile and consider streaming JSON parser (serde_json::StreamDeserializer) or parallel deserialization
- If users request session sharing or sync across machines, evaluate SQLite or a structured format with merge semantics
