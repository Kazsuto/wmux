---
paths:
  - "**/*.rs"
---
# Rust Architecture Rules — wmux

## Crate Boundaries (CRITICAL)
- **Library crates** (wmux-core, wmux-pty, wmux-render, wmux-ui, wmux-ipc, wmux-browser, wmux-config): use `thiserror` v2 for typed error enums. NEVER expose `anyhow::Error` in public APIs.
- **Binary crates** (wmux-app, wmux-cli): use `anyhow::Result` with `.context()` for error propagation.
- Public functions in library crates accept `&str`/`&[T]`, not `&String`/`&Vec<T>`.

## Async & Tokio (CRITICAL)
- **NEVER** block the tokio runtime with `std::thread::sleep`, `std::fs::*`, or CPU-heavy work. Use `tokio::time::sleep`, `tokio::fs::*`, or `spawn_blocking`.
- Use **bounded channels** (`mpsc::channel(N)`) for backpressure — NEVER unbounded.
- Prefer the **actor pattern** (channel + dedicated task) over `Arc<Mutex<T>>` for shared async state (IPC server, PTY manager, notification store).

## Memory & Performance (HIGH)
- Terminal render loop at 60fps. **Reuse allocations** in hot paths — don't allocate/drop per frame.
- Pre-allocate `Vec` with `with_capacity()` when size is known (grid rows, scrollback).
- Grid cells stored contiguously for cache efficiency. Dirty row flags for minimal GPU upload.

## Unsafe & FFI (CRITICAL)
- Wrap all Win32/COM FFI in **RAII safe abstractions** with `Drop` (WebView2, DWM, Named Pipes, Toast).
- Every `unsafe` block must have a `// SAFETY:` comment explaining the invariant.

## Logging (IMPORTANT)
- Use `tracing` crate for all logging. NEVER use `println!` or `eprintln!` in library crates.
- Use structured fields: `tracing::info!(workspace_id = %id, "workspace created")`.
- Span-based tracing for render loop and IPC request handling (performance profiling).
