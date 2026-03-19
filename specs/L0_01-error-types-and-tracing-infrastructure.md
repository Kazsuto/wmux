# Task L0_01: Add Error Types and Tracing Infrastructure to Stub Crates

> **Phase**: Scaffold
> **Priority**: P0-Critical
> **Estimated effort**: 1.5 hours

## Context

The wmux-render, wmux-ui, and wmux-app crates already have proper error types (thiserror) and tracing setup. The remaining 6 crates (wmux-core, wmux-pty, wmux-ipc, wmux-cli, wmux-browser, wmux-config) are stubs with only a comment line. This task establishes the error handling and logging foundation for all crates, following the Architecture §3 Cross-Cutting Concerns and ADR-0008.

PRD §3 CLI & API IPC requires robust error propagation. Architecture §5 mandates thiserror for library crates, anyhow for binaries.

## Prerequisites

None — this task can start immediately.

## Scope

### Deliverables
- `CoreError` enum in `wmux-core/src/error.rs`
- `PtyError` enum in `wmux-pty/src/error.rs`
- `IpcError` enum in `wmux-ipc/src/error.rs`
- `BrowserError` enum in `wmux-browser/src/error.rs`
- `ConfigError` enum in `wmux-config/src/error.rs`
- Updated `Cargo.toml` for each crate with `thiserror` and `tracing` dependencies
- Updated `lib.rs` for each library crate to expose error module
- Updated `main.rs` for wmux-cli with `anyhow` and `tracing` setup

### Explicitly Out of Scope
- Implementing any business logic in these crates
- Adding specific error variants beyond initial placeholders
- Modifying wmux-render, wmux-ui, or wmux-app (already done)

## Implementation Details

### Files to Create/Modify
| Action | Path | Purpose |
|--------|------|---------|
| Create | `wmux-core/src/error.rs` | `CoreError` enum with thiserror derive |
| Create | `wmux-pty/src/error.rs` | `PtyError` enum with thiserror derive |
| Create | `wmux-ipc/src/error.rs` | `IpcError` enum with thiserror derive |
| Create | `wmux-browser/src/error.rs` | `BrowserError` enum with thiserror derive |
| Create | `wmux-config/src/error.rs` | `ConfigError` enum with thiserror derive |
| Modify | `wmux-core/Cargo.toml` | Add thiserror, tracing workspace deps |
| Modify | `wmux-pty/Cargo.toml` | Add thiserror, tracing workspace deps |
| Modify | `wmux-ipc/Cargo.toml` | Add thiserror, tracing workspace deps |
| Modify | `wmux-browser/Cargo.toml` | Add thiserror, tracing workspace deps |
| Modify | `wmux-config/Cargo.toml` | Add thiserror, tracing workspace deps |
| Modify | `wmux-cli/Cargo.toml` | Add anyhow, tracing, tracing-subscriber |
| Modify | `wmux-core/src/lib.rs` | Export error module |
| Modify | `wmux-pty/src/lib.rs` | Export error module |
| Modify | `wmux-ipc/src/lib.rs` | Export error module |
| Modify | `wmux-browser/src/lib.rs` | Export error module |
| Modify | `wmux-config/src/lib.rs` | Export error module |
| Modify | `wmux-cli/src/main.rs` | Add tracing-subscriber init, anyhow Result |

### Key Decisions
- **thiserror v2 for all library crates** (ADR architecture §3): Typed errors, never expose `anyhow::Error` in public APIs
- **anyhow for wmux-cli binary**: Ergonomic `.context()` propagation
- **Initial error variants are placeholders**: Each crate gets a `General(String)` and `Io(#[from] std::io::Error)` variant to start; specific variants added as features are implemented

### Patterns to Follow
- Architecture §3 Cross-Cutting Concerns: `thiserror 2` for libs, `anyhow 1` for bins
- `.claude/rules/rust-architecture.md`: Never bare `unwrap()`, use `expect("reason")` for invariants
- Structured tracing fields, not format strings

### Technical Notes
- wmux-core error will eventually include VTE, grid, and workspace error variants
- wmux-ipc error will include JSON-RPC specific codes matching cmux
- wmux-browser error needs COM-specific variants (HRESULT wrapping)
- All error types must be `Send + Sync` for async compatibility

## Success Criteria

- [ ] All 9 workspace crates compile with `cargo build`
- [ ] Each library crate exposes a typed error enum via `pub mod error`
- [ ] wmux-cli binary initializes tracing-subscriber with `RUST_LOG` env filter
- [ ] `cargo clippy -- -W clippy::all` produces zero warnings
- [ ] `cargo test` passes for all crates

## Validation Steps

### Automated Checks
```bash
cargo build --workspace
cargo clippy --workspace -- -W clippy::all
cargo test --workspace
cargo fmt --all -- --check
```

### Manual Verification
1. Run `RUST_LOG=wmux=debug cargo run -p wmux-cli` and verify tracing output appears
2. Verify each error type derives `Debug`, `Display`, `Error`

### Edge Cases to Test
- Verify `#[from]` conversions compile (e.g., `std::io::Error` → `CoreError`)
- Verify error types are `Send + Sync` (compile test with `fn assert_send<T: Send>() {}`)

## Dependencies

**Blocks** (tasks that cannot start until this completes):
- Task L0_02: Domain Model Types in wmux-core
- Task L0_03: QuadPipeline for Colored Rectangles
- Task L1_05: ConPTY Shell Spawning
- Task L3_03: WebView2 COM Initialization
- Task L3_11: Ghostty-Compatible Config Parser
- Task L4_04: Auto-Update System
- Task L4_05: Mica/Acrylic Visual Effects

## References
- **PRD**: All features (error handling is cross-cutting)
- **Architecture**: §3 Cross-Cutting Concerns, §5 Component Breakdown (error patterns per crate)
- **ADR**: ADR-0001 (Rust), ADR-0008 (Actor pattern requires Send+Sync errors)
