# Changelog

## 2026-03-20

FEATURE: Add Grid struct to wmux-core — flat Vec<Cell> with stride-based indexing, per-row dirty tracking, cursor integration, scroll/resize/insert/delete operations; 21 unit tests
FEATURE: Add QuadPipeline for GPU-accelerated colored rectangle rendering — instanced wgpu pipeline with WGSL shader, batch API (push_quad/prepare/render/clear), 4096-quad capacity, pixel-to-NDC viewport uniform, alpha blending
FIX: Cap QuadPipeline push_quad at buffer capacity to prevent unbounded Vec growth
FIX: Filter NaN/infinity values in push_quad to prevent GPU rendering artifacts
FIX: Guard QuadPipeline::resize against zero dimensions to prevent shader division by zero
REFACTOR: Replace Cell.grapheme String with CompactString (compact_str v0.8) — eliminate heap allocation for graphemes ≤24 bytes
REFACTOR: Replace CoreError::General(String) catch-all with domain-specific variants — OutOfBounds, InvalidScrollRegion, InvalidConfig
FEATURE: Add domain model types to wmux-core — WindowId, WorkspaceId, PaneId, SurfaceId newtypes, Cell struct, CellFlags/TerminalMode bitflags, Color enum, CursorShape/CursorState, SplitDirection, PanelKind, SurfaceInfo; 34 unit tests
FIX: Widen CellFlags from u8 to u16 for future extensibility, add BLINK flag (SGR 5)
FIX: Use from_bits_truncate() for CellFlags/TerminalMode serde deserialization (forward-compat)
FIX: ID Default returns nil UUID instead of random — deterministic
CHORE: Add Eq derive to Cell/SurfaceInfo, Hash derive to CursorState, #[must_use] to ID constructors
CHORE: Add uuid, bitflags, compact_str workspace dependencies

## 2026-03-19

FEATURE: Add error types and tracing infrastructure to 6 stub crates — CoreError, PtyError, IpcError, BrowserError, ConfigError (thiserror v2); wmux-cli gets anyhow + tracing-subscriber with RUST_LOG env filter
REFACTOR: Replace anyhow with thiserror typed errors in wmux-render (RenderError) and wmux-ui (UiError)
REFACTOR: Add .context() to error propagation in wmux-app binary crate
REFACTOR: Add #[inline] to cross-crate hot-path methods in GpuContext and GlyphonRenderer
REFACTOR: Use structured tracing fields instead of format strings in GPU and window logging
FIX: Replace unchecked index access on surface_caps.formats[0] and alpha_modes[0] with .first().expect() in GpuContext::new()
FIX: Replace bare .unwrap() with .expect() documenting invariant in App::render()
CHORE: Add Send + Sync compile-time assertion tests to all 7 error types
CHORE: Add thiserror v2 to workspace dependencies
CHORE: Enhance release profile with strip = "symbols" and panic = "abort"
CHORE: Remove unused anyhow/pollster dependencies from wmux-render
