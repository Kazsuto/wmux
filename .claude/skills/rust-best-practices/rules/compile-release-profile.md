---
title: Configure Release Profile for Maximum Performance
impact: CRITICAL
impactDescription: up to 20% speedup and 43% binary size reduction
tags: compilation, release, lto, codegen-units, strip, cargo
---

## Configure Release Profile for Maximum Performance

Default Cargo release settings prioritize compile speed. Configure `lto`, `codegen-units`, `strip`, and `panic` for production.

**Incorrect (default profile — leaves performance on the table):**

```toml
# Cargo.toml — implicit defaults for `cargo build --release`
# lto = false          → no cross-crate inlining or dead code elimination
# codegen-units = 16   → parallel compilation limits optimization scope
# strip = "none"       → debug symbols included, inflating binary size
# panic = "unwind"     → unwind tables compiled in, adds size and overhead

[package]
name = "myapp"
version = "0.1.0"
edition = "2021"

[dependencies]
# ... dependencies ...
```

**Correct (optimized for production binaries):**

```toml
# Cargo.toml
[package]
name = "myapp"
version = "0.1.0"
edition = "2021"

[dependencies]
# ... dependencies ...

# Production release: maximum optimization, minimum binary size.
[profile.release]
lto = "fat"           # Full LTO: cross-crate inlining and dead code elimination
codegen-units = 1     # Single codegen unit: enables whole-program optimization
strip = "symbols"     # Remove debug symbols: significant binary size reduction
panic = "abort"       # Abort on panic: removes unwind tables, smaller and faster

# Fast development build with some optimization (optional but recommended).
# Use: cargo build --profile dev-opt
[profile.dev-opt]
inherits = "dev"
opt-level = 1         # Basic optimization without sacrificing compile speed
```

**Note:** These settings increase compile times significantly. `lto = "fat"` and `codegen-units = 1` can multiply link time by 3-5x on large projects. Keep the default `dev` profile unchanged for fast iteration. Use `lto = "thin"` as a compromise: most of the benefit of fat LTO at roughly half the link time cost. The `panic = "abort"` setting is incompatible with libraries that use `catch_unwind`; omit it in those cases.
