---
paths:
  - "wmux-render/**/*.rs"
  - "wmux-ui/**/*.rs"
---
# Rendering Rules — wmux

## GPU Rendering (CRITICAL)
- **NEVER** use iced/egui for terminal grid rendering — custom wgpu renderer only.
- Terminal surfaces rendered via glyphon + wgpu. UI chrome (sidebar, overlays) also via wgpu.
- Target < 16ms per frame (60fps). Profile with `tracing` spans in debug builds.

## wgpu 28 Patterns (CRITICAL — version-specific)
- `request_adapter()` returns `Result`, NOT `Option` — do not use `.ok_or()`.
- `RenderPassColorAttachment` requires `depth_slice: None`. `RenderPassDescriptor` requires `multiview_mask: None`.
- `DeviceDescriptor` accepts `memory_hints: MemoryHints::default()` — use it.
- Use `wgpu::PresentMode::AutoVsync` for frame pacing.
- Prefer `Backends::DX12 | Backends::VULKAN` on Windows. Do NOT request GL backend.
- All render state (GpuContext, TextAtlas, Viewport) owned by the App, not by individual panes.
- Resize: reconfigure surface + update viewport + request redraw. Never skip.

## glyphon 0.10 Patterns (CRITICAL — version-specific)
- `TextAtlas::new()` requires a `&Cache` parameter — create `Cache::new(device)` first.
- `Viewport::new()` takes `(device, &cache)` — NOT `(device, &Resolution)`.
- `Viewport::update()` takes `(&queue, Resolution)` — NOT `(&cache, Resolution)`.
- `Buffer::set_text()` takes 5 args: `(&mut font_system, text, &attrs, shaping, Option<Align>)` — attrs is a reference `&Attrs`, not owned.
- Use `Shaping::Advanced` for correct Unicode rendering.
- Monospace fonts only for terminal grid. Cache glyph atlas across frames.
