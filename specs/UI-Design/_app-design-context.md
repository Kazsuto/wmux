---
name: wmux App-Wide Design Context
created: 2026-03-22
stack: native-gpu
technology: wgpu 28 + glyphon 0.10 + winit 0.30
---

# wmux — App-Wide Design Context

## Stack

```yaml
type: native-gpu
technology: "wgpu 28 (Direct3D 12) + glyphon 0.10 (cosmic-text) + winit 0.30"
token_format: rust-const
shader_language: WGSL
blending: premultiplied-alpha
color_format: non-sRGB (hex values already in sRGB, avoids double gamma)
```

## Fonts

```yaml
heading: System monospace (no dedicated heading font)
body: System monospace
mono: System monospace (primary — terminal content)
terminal_size: 14.0px
terminal_line_height: 18.0px
sidebar_size: 13.0px
sidebar_line_height: 18.0px
```

## Colors (wmux-default theme — GitHub Dark inspired)

### Terminal Palette

```yaml
background: "#0d1117"  # (13, 17, 23) — very dark anthracite
foreground: "#e6edf3"  # (230, 237, 243) — near white
cursor: "#e6edf3"
selection_bg: "#264f78" # (38, 79, 120) — dark blue
```

### ANSI 16

| Index | Name | Hex |
|-------|------|-----|
| 0 | Black | #484f58 |
| 1 | Red | #ff7b72 |
| 2 | Green | #3fb950 |
| 3 | Yellow | #d29922 |
| 4 | Blue | #58a6ff |
| 5 | Magenta | #bc8cff |
| 6 | Cyan | #56d4dd |
| 7 | White | #b1bac4 |
| 8 | Bright Black | #6e7681 |
| 9 | Bright Red | #ffa198 |
| 10 | Bright Green | #56d364 |
| 11 | Bright Yellow | #e3b341 |
| 12 | Bright Blue | #79c0ff |
| 13 | Bright Magenta | #d2a8ff |
| 14 | Bright Cyan | #a5d6ff |
| 15 | Bright White | #f0f6fc |

### UI Chrome (derived from terminal palette via HSL)

```yaml
surface_base: "matches background — #0d1117"
surface_0: "~#161b22 — sidebar bg, tab bar bg (+8L from base)"
surface_1: "~#1c2128 — hover, active, selection (+16L)"
surface_2: "~#252d35 — borders, dividers (+24L)"

accent: "#58a6ff — derived from ANSI blue, saturation boosted 80%+"
accent_muted: "#58a6ff at 40% alpha"

text_primary: "#e6edf3 — full alpha"
text_secondary: "#e6edf3 at 55% alpha"
text_muted: "#e6edf3 at 30% alpha"

border: "surface_2 at 60% alpha"
error: "#ff7b72 — from ANSI red"
success: "#3fb950 — from ANSI green"
warning: "#d29922 — from ANSI yellow"
```

## Effects

```yaml
neumorphism: false
glassmorphism: false
gradients: none
shadow_style: minimal  # only command palette has a shadow (black 25%, 2px offset)
transparency: true  # 95% alpha on overlay panels, 40% dimming overlays
```

## Radius

```yaml
style: mixed
ui_panels: 12px  # command palette, notification panel
interactive: 6-8px  # tabs, inputs, results
subtle: 0-1px  # accent bars, borders
```

## Spacing & Dimensions

```yaml
density: compact
sidebar_row_height: 52px
sidebar_padding_x: 12px
sidebar_padding_y: 8px
accent_bar_width: 3px
tab_bar_height: 36px
tab_gap: 4px
tab_radius: 6px
command_palette_width: 600px
input_height: 40px
result_height: 36px
notification_panel_width: 350px
notification_item_height: 72px
focus_stripe_width: 2px
```

## Motion

```yaml
easing: CubicOut (Fluent Design standard)
transition_duration: 200-300ms
notification_pulse: 2s sine wave (alpha 0.3→0.5)
cursor_blink: 500ms
```

## Established Patterns

- All UI chrome colors derived automatically from terminal theme palette via HSL transforms
- Surface elevation system: base → +8L → +16L → +24L lightness steps
- Text hierarchy via alpha: 100% → 55% → 30%
- Accent color always from ANSI blue (palette[4])
- Semantic colors from ANSI red/green/yellow
- Overlays use semi-transparent backgrounds (95% alpha) with dimming backdrop (40% alpha)
- Active items get accent-colored left stripe (2-3px)
- Hover states elevate to surface_1
- Command palette is centered modal with 12px radius + shadow
- Notification panel slides from right with 8px radius
- Tab pills with 6px radius and bottom accent indicator
- Dark mode integrated with Windows DWM (title bar, caption, border colors)
- Theme-agnostic: UI adapts to any terminal theme (catppuccin, dracula, nord, etc.)

## Constraints

- Font rendering via glyphon/cosmic-text — no CSS, no HTML
- All geometry via wgpu QuadPipeline with instancing (max 8192 quads/frame)
- Rounded corners via SDF in WGSL shader
- Premultiplied alpha blending — colors must be premultiplied before GPU submission
- No widget framework — every UI element hand-rendered
- Color values as [f32; 4] RGBA tuples (0.0..=1.0)
