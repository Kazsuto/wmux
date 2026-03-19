# ADR-0002: GPU Rendering — Custom wgpu Pipeline (not iced/egui)

> **Status**: Accepted
> **Date**: 2026-03-19
> **Confidence**: High
> **Deciders**: wmux team

## Context

wmux must render a terminal grid at 60fps with thousands of cells per frame, update only dirty rows, handle cursor blinking, selection highlighting, colored backgrounds, and overlay UI (sidebar, command palette, search). The rendering approach determines whether the terminal feels snappy or sluggish.

## Decision Drivers

- Terminal grid requires per-cell control (foreground color, background color, bold/italic/underline attributes, cursor overlay) at 60fps
- Dirty-row optimization: only upload changed rows to GPU, not full screen
- UI chrome (sidebar, overlays) must coexist with terminal grid in the same window
- WezTerm, Alacritty, Rio all use custom renderers for terminals — this is the proven pattern
- Must work with Direct3D 12 on Windows (primary GPU API)

## Decision

Custom wgpu 28 rendering pipeline:
- **Terminal grid**: glyphon for text rendering (glyph atlas + text buffers), custom dirty-row tracking
- **UI rectangles**: Custom `QuadPipeline` (WGSL shader for colored quads — dividers, backgrounds, selection highlights, cursor)
- **Overlays**: Same wgpu pipeline with alpha blending for command palette, search overlay

All render state (GpuContext, TextAtlas, Viewport) owned centrally by the App, not per-pane.

## Alternatives Considered

### iced (Rust GUI framework)
- **Pros**: Declarative Elm-like API. Built on wgpu. Widget library (buttons, text inputs, scrollables). Good for forms and settings
- **Cons**: Not designed for high-frequency grid rendering. No per-cell dirty tracking. Layout engine adds overhead per frame. Cannot bypass the widget tree for raw GPU access
- **Why rejected**: iced's abstraction layer prevents the per-row dirty optimization needed for 60fps terminal rendering. Fine for a settings dialog, but not for the terminal grid that is 95% of the render surface

### egui (immediate-mode GUI)
- **Pros**: Immediate mode is simple to code. Good for debug UIs and overlays. Fast for simple cases
- **Cons**: Redraws entire UI every frame (no dirty tracking). Font rendering quality lower than glyphon. Not designed for fixed-grid monospace rendering. Would fight the layout engine to achieve pixel-perfect cell alignment
- **Why rejected**: Immediate-mode redraws everything every frame — the opposite of what a terminal needs (redraw only changed rows). Font quality is insufficient for a primary terminal

### Direct2D / DirectWrite (raw Win32)
- **Pros**: Native Windows text rendering. Excellent ClearType. No crate dependency
- **Cons**: CPU-based rendering (Direct2D is not GPU-accelerated for text in the way wgpu is). Windows-only forever. Complex COM API. Cannot share render context with wgpu for overlays
- **Why rejected**: Locks out future cross-platform potential. Mixing Direct2D with wgpu for overlays creates compositing complexity. glyphon + wgpu provides GPU-accelerated text with better performance

## Consequences

### Positive
- Full control over rendering: per-cell dirty tracking, custom cursor, selection overlay, colored backgrounds
- 60fps achievable with minimal GPU upload (only changed rows)
- Single rendering backend (wgpu) for both terminal grid and UI chrome — no compositing between frameworks
- Cross-platform potential: wgpu maps to Vulkan (Linux), Metal (macOS) if wmux expands later

### Negative (acknowledged trade-offs)
- Significant upfront implementation cost: glyph atlas management, quad pipeline, shader code, resize handling
- No widget library — sidebar, command palette, search overlay must be hand-built
- Font rendering quality depends on glyphon/cosmic-text — may not match DirectWrite ClearType hinting (acceptable for monospace)

### Mandatory impact dimensions
- **Security**: wgpu is sandboxed — GPU commands are validated before submission. No direct GPU memory access from user code
- **Cost**: $0. wgpu and glyphon are MIT-licensed. No GPU licensing
- **Latency**: Dirty-row optimization means typical frames upload < 100 rows. Sub-16ms frame time validated by WezTerm at scale

## Revisit Triggers

- If glyphon font quality is noticeably worse than Windows Terminal's DirectWrite rendering in user feedback, investigate a hybrid approach (DirectWrite for text, wgpu for quads)
- If iced adds a "raw canvas" mode that allows per-cell dirty rendering without widget overhead, reconsider for UI chrome (sidebar, overlays)
- If wgpu 29 breaks the rendering pipeline significantly, evaluate the migration cost vs staying on 28
