# ADR-0003: Text Rendering — glyphon 0.10

> **Status**: Accepted
> **Date**: 2026-03-19
> **Confidence**: Medium
> **Deciders**: wmux team

## Context

The terminal grid requires fast, GPU-accelerated monospace text rendering with support for Unicode, bold/italic variants, colored glyphs, and efficient atlas management. The text rendering crate must integrate with wgpu 28 and handle thousands of glyphs per frame without per-frame allocation.

## Decision Drivers

- Must integrate with wgpu 28 (our GPU backend) — no separate rendering context
- Glyph atlas management should be automatic (pack, evict, resize)
- Need advanced text shaping for Unicode correctness (ligatures optional but nice)
- Used by other Rust terminal/editor projects — proven in production
- Active maintenance (wgpu version coupling means we need timely updates)

## Decision

**glyphon 0.10** for all text rendering in wmux. glyphon provides:
- `TextAtlas`: GPU texture atlas with automatic glyph packing (etagere)
- `TextRenderer`: wgpu render pass integration
- `Buffer`: Text layout with font shaping (cosmic-text → swash)
- `Viewport`: Resolution-aware coordinate mapping

Font rasterization is handled by cosmic-text/swash internally — **not** by DirectWrite/dwrote directly.

## Alternatives Considered

### Manual glyph atlas (wgpu + dwrote directly)
- **Pros**: Full control over hinting, ClearType, glyph placement. DirectWrite gives best Windows font quality
- **Cons**: Massive implementation effort: atlas packing, LRU eviction, GPU texture management, font fallback chains, Unicode shaping. Months of work for what glyphon provides out of the box
- **Why rejected**: Engineering cost is too high for v1. glyphon's atlas management is production-proven. If font quality becomes an issue, we can investigate a DirectWrite-backed font source for glyphon (cosmic-text supports custom font sources)

### wgpu_glyph (older wgpu text crate)
- **Pros**: Simpler API. Used by some older projects
- **Cons**: Deprecated in favor of glyphon. Last release 0.23.0 (2023). Does not support wgpu 28. Based on glyph_brush which lacks advanced shaping
- **Why rejected**: Unmaintained. Incompatible with wgpu 28. glyphon is its spiritual successor

### cosmic-text directly (without glyphon)
- **Pros**: Text shaping and layout engine. More control over layout. Used by COSMIC desktop
- **Cons**: cosmic-text handles *layout*, not *rendering*. Would still need manual GPU atlas, texture upload, and render pass integration — effectively reimplementing glyphon
- **Why rejected**: cosmic-text is a *dependency* of glyphon, not an alternative. Using it directly just means writing the GPU integration layer ourselves

## Consequences

### Positive
- Production-proven GPU text rendering (used by COSMIC Terminal, other wgpu projects)
- Automatic glyph atlas management (packing, eviction, resize)
- Advanced Unicode shaping via cosmic-text/swash (correct rendering of complex scripts)
- Clean wgpu integration (TextRenderer drops into any render pass)

### Negative (acknowledged trade-offs)
- Tightly coupled to wgpu version — cannot upgrade wgpu without a matching glyphon release (currently blocked at wgpu 28)
- Font rendering quality may differ from Windows Terminal's DirectWrite ClearType hinting — cosmic-text/swash uses its own rasterizer
- Less control over glyph placement than a manual atlas — may need workarounds for pixel-perfect monospace alignment

### Mandatory impact dimensions
- **Security**: glyphon only handles rendering — no file I/O, no network. Minimal attack surface
- **Cost**: $0. MIT licensed
- **Latency**: Glyph atlas is retained across frames. Prepare phase only uploads changed glyphs. Measured sub-1ms for typical terminal updates in benchmarks

## Revisit Triggers

- If users report noticeably worse font rendering quality compared to Windows Terminal, investigate using DirectWrite as a custom font source for cosmic-text (swash has a pluggable rasterizer API)
- If glyphon does not publish a wgpu 29-compatible release within 3 months of wgpu 29 stable, evaluate forking or switching to manual atlas with the wgpu migration
- If terminal grid rendering exceeds 8ms per frame at 200 columns × 50 rows, profile and consider batch optimization or manual atlas for the terminal grid (keep glyphon for UI text)
