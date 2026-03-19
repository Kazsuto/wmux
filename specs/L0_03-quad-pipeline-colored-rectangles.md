# Task L0_03: Build QuadPipeline for Colored Rectangles

> **Phase**: Scaffold
> **Priority**: P0-Critical
> **Estimated effort**: 2.5 hours

## Context

The terminal multiplexer needs to render colored rectangles everywhere: cell backgrounds, cursor block, text selection highlights, pane dividers, sidebar background, notification badges, and progress bars. The existing wmux-render crate has GpuContext (wgpu surface/device) and GlyphonRenderer (text) but lacks a pipeline for colored quads. Architecture §5 (wmux-render) specifies QuadPipeline as a core component. ADR-0002 mandates custom wgpu rendering.

## Prerequisites

- [ ] Task L0_01: Error Types and Tracing Infrastructure — wmux-render already has this, but ensures workspace compiles

## Scope

### Deliverables
- `QuadPipeline` struct in `wmux-render/src/quad.rs`
- WGSL vertex + fragment shader in `wmux-render/src/shader.wgsl`
- Quad vertex format (position, color, with optional per-instance data)
- Batch rendering API: `push_quad(rect, color)` → `flush(render_pass)`
- Integration with existing `GpuContext` (shares device/queue)

### Explicitly Out of Scope
- Rounded corners (post-MVP Mica/Acrylic task)
- Texture-mapped quads (not needed for terminal)
- 3D transforms (2D only)
- Anti-aliased edges (pixel-aligned quads are sufficient)

## Implementation Details

### Files to Create/Modify
| Action | Path | Purpose |
|--------|------|---------|
| Create | `wmux-render/src/quad.rs` | QuadPipeline struct, vertex format, batch API |
| Create | `wmux-render/src/shader.wgsl` | WGSL vertex + fragment shaders for colored quads |
| Modify | `wmux-render/src/lib.rs` | Export quad module |
| Modify | `wmux-render/Cargo.toml` | Add `bytemuck` dependency (if not already) |

### Key Decisions
- **Instanced rendering**: Each quad is an instance with (x, y, width, height, r, g, b, a). Vertex shader generates corners from instance data. Minimizes vertex buffer uploads
- **Dynamic buffer**: Re-upload quad buffer each frame (quads change frequently with cursor blink, selection, dirty rows). Use `write_buffer` with pre-allocated capacity
- **Coordinate system**: Pixel coordinates (0,0 top-left), transformed to NDC in vertex shader using viewport dimensions uniform

### Patterns to Follow
- Architecture §5 wmux-render: "QuadPipeline (colored rectangles for UI)"
- `.claude/rules/rendering.md`: wgpu 28 patterns — `RenderPassColorAttachment` needs `depth_slice: None`, `RenderPassDescriptor` needs `multiview_mask: None`
- Retained-mode rendering: QuadPipeline retains the pipeline/bind group across frames, only the instance buffer changes

### Technical Notes
- **wgpu 28 specifics**: `DeviceDescriptor` uses `memory_hints: MemoryHints::default()`. Use `Backends::DX12 | Backends::VULKAN`
- Vertex format: `QuadInstance { x: f32, y: f32, w: f32, h: f32, color: [f32; 4] }` — must derive `bytemuck::Pod, bytemuck::Zeroable`
- Pre-allocate instance buffer for ~4096 quads (enough for full-screen terminal backgrounds + UI)
- Shader uniform: viewport size (width, height) for pixel→NDC conversion
- Render order: backgrounds first, then text (glyphon) on top. QuadPipeline renders before GlyphonRenderer in the render pass

## Success Criteria

- [ ] QuadPipeline compiles and integrates with existing GpuContext
- [ ] Can render a colored rectangle at arbitrary pixel position and size
- [ ] Can batch-render 1000+ quads in a single draw call
- [ ] WGSL shader compiles without warnings on D3D12 backend
- [ ] Quad colors support full RGBA (including alpha for future transparency)
- [ ] `cargo clippy --workspace` zero warnings
- [ ] `cargo test -p wmux-render` passes

## Validation Steps

### Automated Checks
```bash
cargo build --workspace
cargo clippy --workspace -- -W clippy::all
cargo test -p wmux-render
cargo fmt --all -- --check
```

### Manual Verification
1. Modify wmux-app temporarily to render a few colored quads (red, green, blue rectangles at different positions)
2. Verify quads appear at correct positions with correct colors
3. Verify window resize correctly re-maps quad positions
4. Verify alpha blending works (semi-transparent quad over background)

### Edge Cases to Test
- Zero-size quad (should not crash, just skip)
- Quad extending beyond viewport (should clip naturally)
- Flush with zero quads (no-op, no crash)
- Very large batch (4000+ quads) — verify no buffer overflow

## Dependencies

**Blocks**:
- Task L1_07: Terminal Grid GPU Rendering Pipeline
- Task L2_04: Multi-Pane GPU Rendering
- Task L2_08: Sidebar UI Rendering
- Task L3_09: Notification Visual Indicators

## References
- **PRD**: §1 Terminal GPU-Accéléré (cell backgrounds, cursor), §2 Multiplexeur (dividers), §5 Sidebar Metadata (progress bars, badges)
- **Architecture**: §5 wmux-render ("QuadPipeline — colored rectangles for UI"), §4 Component Diagram
- **ADR**: ADR-0002 (Custom wgpu pipeline, not iced/egui)
