---
paths:
  - "**/*.rs"
---
# Testing Rules — wmux

- Unit tests in `#[cfg(test)]` modules within each source file. Integration tests in `tests/` directories.
- Run `cargo clippy -- -W clippy::all` after every change — zero warnings policy.
- Run `cargo fmt` before committing.
- NEVER skip `cargo test` before marking a task complete.
- Use `#[ignore]` for tests requiring a real PTY or GPU — run them with `cargo test -- --ignored`.
