# ADR-0001: Language — Rust

> **Status**: Accepted
> **Date**: 2026-03-19
> **Confidence**: High
> **Deciders**: wmux team

## Context

wmux is a native Windows terminal multiplexer that requires: GPU-accelerated text rendering, pseudo-terminal management (ConPTY), async I/O (IPC, PTY reads), low input-to-display latency (< 16ms), and safe interop with Win32/COM APIs. The language choice affects ecosystem access, safety guarantees, and long-term maintainability.

## Decision Drivers

- Rich terminal ecosystem: existing crates for VTE parsing, PTY, GPU rendering, clipboard
- Memory safety without garbage collector latency (critical for 60fps render loop)
- Proven track record: WezTerm, Alacritty, Rio are production Rust terminals on Windows
- Async ecosystem (tokio) for IPC server, PTY I/O, and session persistence
- Strong Windows support via `windows` crate (Microsoft-maintained)

## Decision

**Rust** (edition 2021, MSRV 1.80) as the sole implementation language for all wmux crates. Go is retained only for the reused cmux SSH daemon (`wmuxd-remote`).

## Alternatives Considered

### Zig
- **Pros**: Excellent for low-level terminal engines (Ghostty is written in Zig). Good C interop. No hidden allocations
- **Cons**: No terminal-relevant crate ecosystem (no portable-pty, no wgpu, no winit). No async runtime. Limited Windows GUI tooling. Package management immature
- **Why rejected**: The ecosystem gap is insurmountable. Ghostty works because it's a terminal *engine* with a Swift/AppKit UI layer on top. wmux needs the full stack in one language, and Zig's ecosystem cannot provide it

### C++
- **Pros**: Full Win32/DirectX access. Windows Terminal is C++. Mature tooling
- **Cons**: Manual memory management (CVE risk in a network-exposed IPC server). No equivalent of portable-pty/vte crates — everything is manual. Build system complexity (CMake/MSBuild). Slower iteration
- **Why rejected**: Safety risk in IPC/PTY code where buffer overflows are likely attack surfaces. Development velocity too slow without the Rust crate ecosystem

### C# (WinUI 3)
- **Pros**: First-class Windows citizen. WinUI 3 for native UI. .NET async
- **Cons**: GC pauses break 60fps render loop. No terminal crates. WinUI 3 cannot do custom GPU text rendering at terminal speeds. Runtime dependency (.NET 8+)
- **Why rejected**: GC latency is incompatible with the < 16ms frame budget. WinUI 3 is designed for business apps, not GPU-intensive terminal rendering

## Consequences

### Positive
- Access to battle-tested crates: vte, portable-pty, wgpu, winit, glyphon, tokio
- Compile-time memory safety eliminates entire classes of bugs (use-after-free, buffer overflows) in security-sensitive IPC code
- `cargo` workspace provides clean crate decomposition with checked dependency boundaries
- Same language for all layers (rendering, IPC, CLI) reduces context switching

### Negative (acknowledged trade-offs)
- Steeper learning curve for new contributors vs C# or TypeScript
- Longer compile times (mitigated by workspace incremental builds, LTO only in release)
- Go daemon for SSH is a second language in the project (acceptable — it's a reused, stable component)

### Mandatory impact dimensions
- **Security**: Rust's borrow checker prevents memory corruption in IPC server and PTY I/O — the highest-risk attack surfaces. `unsafe` blocks are explicitly marked and auditable
- **Cost**: $0. Rust toolchain is free. No runtime license. MIT-licensed crates
- **Latency**: Zero-cost abstractions and no GC mean predictable frame timing. `tokio` async avoids thread-per-connection overhead in IPC server

## Revisit Triggers

- If Zig ecosystem gains wgpu bindings, async runtime, and terminal crates, reconsider for performance-critical subsystems (terminal engine)
- If compile times exceed 5 minutes for incremental debug builds, investigate `cranelift` codegen backend or workspace structure changes
