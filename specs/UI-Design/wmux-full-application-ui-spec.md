---
title: "UI Specification: wmux — Full Application Interface"
task_id: "wmux-full-application"
product_type: "devtool"
primary_style: "refined-dark"
secondary_style: "futuristic-ai (glow only)"
density: "compact"
atmosphere: "bold-contrast (hierarchy) + futuristic-ai (technique)"
inspirations: ["Linear", "Vercel"]
app_context_used: true
design_mode: "redesign"
stack: "native-gpu: wgpu 28 + glyphon 0.10"
concept: "Luminous Void"
guardian_review: "passed (rev2)"
created: "2026-03-22"
stepsCompleted: [0, 1, 2, 3, 4, 5, 6]
---

# UI Specification: wmux — "Luminous Void"

Native Windows terminal multiplexer. GPU-accelerated (wgpu + glyphon), split panes, workspaces, command palette, sidebar, notifications, status bar. Dark mode default, keyboard-first, targets developers and AI agents.

---

## 1. UI Audit Summary

### What Works Well (PRESERVE)

| Element | Why |
|---------|-----|
| HSL-derived surface elevation | All surface levels auto-computed from theme — guarantees coherence across 5+ bundled themes |
| Text hierarchy via alpha | 3-tier alpha system creates clear information layers without extra colors |
| Accent from ANSI blue | Accent always harmonizes with terminal palette — derived, not hardcoded |
| Active item left stripe | 2-3px accent bar is a clear, minimal focus indicator |
| Theme-agnostic architecture | catppuccin, dracula, nord all auto-generate coherent UI chrome |
| Command palette centered modal | Follows devtool conventions (VS Code, Raycast) |
| CubicOut easing | Fluent Design standard — native Windows feel |

### Issues Addressed in This Spec

| # | Issue | Solution |
|---|-------|----------|
| 1 | No sans-serif font for UI chrome | Dual font system: Segoe UI Variable (chrome) + monospace (terminal) |
| 2 | No status bar | New 28px status bar with workspace, connection, branch info |
| 3 | Sidebar lacks visual richness | Section headers, workspace color dots, icons, hover transitions |
| 4 | No visual distinction between pane types | Type indicators on tabs (>_ terminal, globe browser) |
| 5 | Shadow only on command palette | Expanded shadow system with glow variant |
| 6 | No empty states | Designed for sidebar, notifications, terminal |
| 7 | Notification severity not styled | Left stripe + muted background tint per severity |
| 8 | Tab bar lacks affordances | Close button on hover, unsaved dot indicator |

---

## 2. Visual Direction

- **Product type:** devtool (terminal multiplexer)
- **UI objective:** Immersion + orientation instantanee — identify active pane, current workspace, and system state in < 1 second, even with 8+ splits
- **Density:** Compact (4px grid, 36px tabs, 28px status bar)
- **Primary style:** refined-dark — visual discipline (single accent, fine typography, subtle shadows) without the generous spacing of canonical luxury-dark
- **Secondary influence:** futuristic-ai — ONLY luminous glow effects (Focus Glow, border-glow). Excluded: neon, hexagons, grids, backdrop blur, data animations
- **Atmosphere:** bold-contrast provides hierarchy (active = bright, inactive = recedes). futuristic-ai provides the technique (glow rendering)
- **Inspirations:** Linear (density, command palette, opacity hierarchy), Vercel (monochromatic discipline, terminal as first-class)
- **Signature element:** **Focus Glow** — luminous halo around the active terminal pane

---

## 3. Visual System (Design Tokens)

All values are concrete, implementable as Rust `[f32; 4]` RGBA constants or pixel dimensions.

### 3.1 Colors

#### Surface Elevation (HSL-derived, 5L steps)

| Token | Derivation | wmux-default Hex | Usage |
|-------|------------|------------------|-------|
| `surface-base` | theme.background | #0d1117 | Terminal pane bg |
| `surface-0` | base + 5L | ~#13181f | Subtle lift |
| `surface-1` | base + 10L | ~#1a2029 | Sidebar bg, tab bar bg |
| `surface-2` | base + 15L | ~#212830 | Hover, active, selections |
| `surface-3` | base + 20L | ~#283038 | Borders, dividers |
| `surface-overlay` | surface-1 at 95% alpha | ~#1a2029/95% | Command palette bg, notification panel bg |

#### Accent System

| Token | Value | Usage |
|-------|-------|-------|
| `accent` | theme.palette[4], S>=80% | #58a6ff — Primary action color |
| `accent-hover` | accent + 8L | ~#79bbff — Hover state |
| `accent-muted` | accent at 30% alpha | Subtle highlights |
| `accent-glow` | accent at 20% alpha | Focus Glow outer halo |
| `accent-glow-core` | accent at 50% alpha | Focus Glow inner 1px ring |
| `accent-tint` | accent at 8% alpha | Overlay ambient coloring |

#### Text Hierarchy

| Token | Alpha | WCAG on surface-base | Usage |
|-------|-------|---------------------|-------|
| `text-primary` | 100% | 16.14:1 | Headings, active labels, terminal |
| `text-secondary` | 65% | ~8.5:1 | Sidebar subtitles, inactive tabs |
| `text-muted` | 53% | ~4.5:1 | Timestamps, hints, section headers |
| `text-faint` | 40% | ~3.0:1 | Decorative-only metadata (NOT functional text) |
| `text-inverse` | surface-base 100% | 7.58:1 on accent | Text on accent backgrounds |

#### Semantic Colors (derived from ANSI)

| Token | Hex | Muted (12% alpha) | Usage |
|-------|-----|-------------------|-------|
| `error` | #ff7b72 (ANSI red) | error-muted | Errors, connection failures |
| `success` | #3fb950 (ANSI green) | success-muted | Connected, completed |
| `warning` | #d29922 (ANSI yellow) | warning-muted | Reconnecting, unsaved |
| `info` | = accent | info-muted | Informational notifications |

#### Borders

| Token | Value | Usage |
|-------|-------|-------|
| `border-subtle` | surface-3 at 40% alpha | Faint dividers |
| `border-default` | surface-3 at 60% alpha | Standard borders |
| `border-strong` | surface-3 at 80% alpha | Emphasized borders |
| `border-glow` | accent at 25% alpha | Luminous separators (sidebar right edge, pane dividers) |

#### Overlays

| Token | Value | Usage |
|-------|-------|-------|
| `overlay-dim` | #000000 at 50% alpha | Command palette backdrop |
| `overlay-tint` | accent at 8% alpha | Layered on overlay-dim for ambient coloring |

### 3.2 Typography

#### Font Families

```
font-ui:   "Segoe UI Variable" (Win11), "Segoe UI" (Win10), system-ui, sans-serif
font-mono: user terminal font (default: "Cascadia Code", "Consolas", monospace)
```

Win10 1809+ uses "Segoe UI" (non-variable). Metrics differ by ~1px at 13px. All component heights (48px sidebar, 36px tabs, 28px status bar) accommodate this variance.

#### Scale (Compact)

| Token | Size / Line-height | Usage |
|-------|-------------------|-------|
| `text-2xs` | 11px / 15px | Decorative metadata only |
| `text-xs` | 12px / 16px | Status bar, section headers, shortcuts, badges — MINIMUM for functional text |
| `text-sm` | 13px / 17px | Helper text, sidebar subtitles, notification messages |
| `text-base` | 14px / 19px | Sidebar primary text, tab labels, notification titles |
| `text-lg` | 15px / 20px | Command palette results |
| `text-xl` | 16px / 21px | Command palette input, modal titles |
| `text-2xl` | 18px / 23px | Reserved (onboarding) |
| `terminal` | 14px / 18px | Terminal content (user-configurable, independent) |

#### Weights

| Token | Value | Usage |
|-------|-------|-------|
| `weight-regular` | 400 | Body text, status bar values |
| `weight-medium` | 500 | Sidebar labels, tab labels, notification titles |
| `weight-semibold` | 600 | Section headers, active workspace name, palette input |

#### Letter Spacing

| Token | Value | Usage |
|-------|-------|-------|
| `tracking-tight` | -0.01em | text-lg+ headers |
| `tracking-normal` | 0em | Body text |
| `tracking-wide` | +0.04em | Uppercase labels, keyboard shortcuts |

### 3.3 Spacing (4px Grid)

| Token | Value | Usage |
|-------|-------|-------|
| `space-0.5` | 2px | Hairline gaps, icon-text micro spacing |
| `space-1` | 4px | Minimum spacing, tight internal padding |
| `space-1.5` | 6px | Tab internal padding vertical |
| `space-2` | 8px | Element gap, notification item padding |
| `space-3` | 12px | Component padding (sidebar, palette) |
| `space-4` | 16px | Section gap |
| `space-6` | 24px | Major section gap |
| `space-8` | 32px | Zone gap |

#### Component Dimensions

| Component | Value |
|-----------|-------|
| Sidebar row height | 48px |
| Sidebar collapsed width | 48px |
| Sidebar expanded width | 220px |
| Tab bar height | 36px |
| Tab gap | 4px |
| Status bar height | 28px |
| Command palette width | 600px |
| Command input height | 44px |
| Command result height | 36px |
| Notification panel width | 360px |
| Notification item height | 72px |
| Pane divider width | 1px |
| Focus stripe width | 3px |

### 3.4 Radius

| Token | Value | Assigned Components |
|-------|-------|-------------------|
| `radius-none` | 0px | Terminal panes, accent stripes, pane dividers |
| `radius-xs` | 2px | Focus stripe ends, tab accent indicator |
| `radius-sm` | 4px | Tooltips, keyboard shortcut caps, icon-only buttons |
| `radius-md` | 6px | Tabs, results, sidebar hovers, buttons, inputs |
| `radius-lg` | 8px | Notification panel, toast, dropdowns |
| `radius-xl` | 12px | Command palette, modal overlays |
| `radius-full` | 9999px | Notification badges, workspace color dots |

### 3.5 Shadows & Depth

#### Standard Shadows (dark theme — 30-45% opacity)

| Token | Value | Usage |
|-------|-------|-------|
| `shadow-none` | none | Flat elements, panes |
| `shadow-sm` | 0 1px 3px rgba(0,0,0,0.30) | Tab bar, status bar |
| `shadow-md` | 0 4px 8px rgba(0,0,0,0.35) | Dropdowns |
| `shadow-lg` | 0 8px 16px rgba(0,0,0,0.40) | Notification panel |
| `shadow-xl` | 0 12px 24px rgba(0,0,0,0.45) | Command palette, modals |

#### Glow Shadows (Signature)

| Token | Value | Usage |
|-------|-------|-------|
| `glow-focus` | 1px ring accent@50% + 16px halo accent@20% | Active pane Focus Glow (max 1 at a time) |
| `glow-subtle` | 8px halo accent@20% | Active sidebar row |

#### Focus Glow Implementation

**Approach A (no shader change):** 4 concentric quads behind active pane:
- Quad 1: bounds +1px, accent@50% (inner ring)
- Quad 2: bounds +4px, accent@25%
- Quad 3: bounds +9px, accent@12%
- Quad 4: bounds +16px, accent@5% (outer halo)

**Approach B (shader extension):** Add `glow_radius: f32` + `glow_color: vec4` to QuadInput. Fragment shader outer glow via `smoothstep(glow_radius, 0.0, -sdf_distance)`.

### 3.6 Motion

#### Duration Scale

| Token | Value | Usage |
|-------|-------|-------|
| `motion-micro` | 80ms | Hover color changes |
| `motion-fast` | 150ms | Focus transitions, tab switch |
| `motion-normal` | 250ms | Focus Glow appear, sidebar transitions |
| `motion-slow` | 350ms | Command palette open, notification slide |
| `motion-pulse` | 2000ms | Notification attention ring |
| `motion-blink` | 500ms | Cursor blink |

#### Easing

| Token | Value | Usage |
|-------|-------|-------|
| `ease-out` | cubic-bezier(0.33, 1, 0.68, 1) | CubicOut: entering elements |
| `ease-in` | cubic-bezier(0.32, 0, 0.67, 0) | CubicIn: exiting elements |
| `ease-in-out` | cubic-bezier(0.65, 0, 0.35, 1) | State changes, position moves |
| `ease-linear` | linear | Cursor blink, continuous |

#### Focus Glow Cross-Fade

```
Old pane: glow fades out, ease-in, 150ms (starts immediately)
New pane: glow fades in, ease-out, 250ms (starts after 50ms delay)
Overlap: ~150ms where both panes glow (intentional cross-fade)
```

#### Reduced Motion

When Windows animation effects disabled (`SPI_GETCLIENTAREAANIMATION`): all durations → 0ms except cursor blink.

### 3.7 Inactive Pane Rule

`inactive_pane_opacity` applies ONLY to a surface-base overlay on top of the pane, NOT to text. Text keeps its full alpha hierarchy in inactive panes, ensuring terminal content remains readable.

---

## 4. Layout

### Structure

```
┌─────────────────────────────────────────────────────┐
│              Window Title Bar (DWM native)           │
├──────────┬──────────────────────────────────────────┤
│          │            Tab Bar (36px)                 │
│ Sidebar  ├──────────────────────────────────────────┤
│ (220px   │                                          │
│  or      │      Terminal Pane Area                   │
│  48px    │      ┌──────────┬───────────┐            │
│  icon    │      │ Active   │ Inactive  │            │
│  mode)   │      │ GLOW     │ (dimmed)  │            │
│          │      │          │           │            │
│          │      └──────────┴───────────┘            │
│          ├──────────────────────────────────────────┤
│          │         Status Bar (28px)                 │
├──────────┴──────────────────────────────────────────┤
│  Overlays: Command Palette (centered modal)          │
│            Notification Panel (right-side slide)     │
└─────────────────────────────────────────────────────┘
```

### Layout Rules

- Sidebar: left-anchored, 220px expanded / 48px collapsed, resizable
- Content area: fluid, fills remaining width, split into panes via tree layout
- Tab bar: full width of content area (not sidebar)
- Status bar: full width of window (spans sidebar + content)
- Overlays: command palette centered at 20% from top, notification panel slides from right
- Scan pattern: sidebar (context) → tabs (switching) → content (work) → status bar (info)

### Responsive

- 1920px+: full sidebar + multi-pane splits
- 1280px: full sidebar + 2-pane comfortable
- 1024px: collapsed sidebar (48px icons) + single/2-pane
- <1024px: not targeted

---

## 5. Components

### 5.1 Sidebar

| Property | Value |
|----------|-------|
| Width | 220px expanded, 48px collapsed |
| Background | surface-1 |
| Right border | 1px border-glow (accent 25%) |
| Row height | 48px |
| Section header | "WORKSPACES" — text-xs, weight-medium, font-ui, tracking-wide, text-muted, uppercase |
| Primary text | text-base, weight-medium, font-ui, text-primary |
| Secondary text | text-sm, weight-regular, font-ui, text-secondary |
| Active row | surface-2 bg + 3px accent stripe left + glow-subtle |
| Hover row | surface-2 at 50% alpha, motion-micro |
| Workspace dot | 8px circle, ANSI palette color per workspace |
| Notification badge | 16px circle, radius-full, accent bg, text-2xs text-inverse |

### 5.2 Tab Bar

| Property | Value |
|----------|-------|
| Height | 36px |
| Background | surface-1 |
| Shadow | shadow-sm (bottom) |
| Pill height | 28px, radius-md |
| Active tab | surface-2 bg + 2px accent bottom indicator |
| Inactive tab | surface-1, text-secondary |
| Hover | surface-2 at 50%, motion-micro |
| Close button | hover-reveal, 16px, text-muted → text-secondary |
| Type indicator | >_ (terminal), globe (browser) — 8px, text-muted |
| Unsaved dot | 6px, warning color |
| Max tab width | 160px, truncate with ellipsis |

### 5.3 Command Palette

| Property | Value |
|----------|-------|
| Width | 600px centered, 20% from top |
| Background | surface-overlay (surface-1 at 95%) |
| Radius | radius-xl (12px) |
| Shadow | shadow-xl |
| Border | 1px border-subtle |
| Backdrop | overlay-dim + overlay-tint layered |
| Input | 44px height, surface-0 bg, radius-lg, text-xl, weight-regular |
| Result | 36px height, selected: surface-2 + radius-md |
| Shortcut hint | text-xs, font-mono, text-muted, tracking-wide |
| Open anim | backdrop motion-fast fade, panel scale 0.97→1.0 + fade motion-slow ease-out |
| Close anim | reverse, motion-fast |

### 5.4 Status Bar

| Property | Value |
|----------|-------|
| Height | 28px, full window width |
| Background | surface-1 |
| Shadow | shadow-sm inverted (upward) |
| Labels | text-xs, weight-regular, font-ui, text-secondary |
| Values | text-xs, weight-regular, font-mono, text-primary |
| Separator | " · " in text-muted |
| Sections | workspace name · pane count · connection dot · branch · encoding · shell |
| Connection dot | 6px circle: success (connected), warning (reconnecting), error (disconnected) |

### 5.5 Notification Panel

| Property | Value |
|----------|-------|
| Width | 360px, slides from right |
| Background | surface-overlay |
| Radius | radius-lg left corners only |
| Shadow | shadow-lg |
| Header | "Notifications" text-lg weight-semibold, clear all (ghost), close (icon-only) |
| Item height | 72px |
| Severity stripe | 2px left, colored by error/warning/success/info |
| Severity bg | severity-muted (12% alpha tint) |
| Content | title (text-base weight-medium), message (text-sm text-secondary), timestamp (text-2xs text-muted) |
| Slide anim | translateX(360→0) + fade, motion-slow, ease-out |

### 5.6 Terminal Pane

| Property | Value |
|----------|-------|
| Background | surface-base |
| Content | glyphon text atlas, ANSI 16 colors |
| Cursor | theme cursor color at 85% alpha, blinks motion-blink |
| **Active pane** | **glow-focus** (1px inner ring + 16px outer halo) — THE SIGNATURE |
| Inactive pane | surface-base overlay at (1 - inactive_pane_opacity), text unaffected |
| Dividers | 1px border-glow, expand to 5px on hover (border-default), resize cursor |

### 5.7 Buttons

| Variant | Background | Text | Border | Height |
|---------|-----------|------|--------|--------|
| Primary | accent | text-inverse | none | 32px |
| Secondary | surface-2 | text-primary | 1px border-default | 32px |
| Ghost | transparent | text-secondary | none | 32px |
| Destructive | error | text-inverse | none | 32px |
| Icon-only | transparent | text-secondary (16px icon) | none | 28x28px |

All: radius-md, text-sm weight-medium font-ui. Hover: motion-micro. Press: scale 0.97. Focus: 2px accent ring.

---

## 6. UI States

### Hover
Surface elevation +1 level at 50% alpha, motion-micro (80ms), ease-out. Text: secondary → primary on interactive labels.

### Focus (Keyboard)
- **Pane:** glow-focus (signature)
- **Elements:** 2px accent outline, 2px offset
- **Palette result:** surface-2 + left accent indicator
- Focus-visible only (not on mouse click). 3:1+ contrast ratio.

### Loading
- Terminal connecting: "Connecting..." text-muted centered + cursor blink
- Palette search: 12px accent spinner, 800ms linear
- Notifications/sidebar: skeleton shimmer surface-2 → surface-0, 1.5s ease-in-out
- SSH pending: status bar dot pulses warning, pane shows "Connecting to {host}..."

### Empty
- No workspaces: terminal icon 32px + "No workspaces" text-base + CTA
- No notifications: "All caught up" text-muted
- Empty terminal: normal cursor blink

### Error
- Connection: status bar dot → error, tooltip with message
- Command: toast notification error severity
- Process crash: error message centered in pane + "Restart" button

### Success
- Command: brief success-muted flash (200ms) on palette result
- Workspace created: toast success, auto-dismiss 3s
- Connection: status bar dot → success, brief glow-subtle pulse

### Disabled
40% overall opacity, no hover effects, text-muted. Cursor: not-allowed.

---

## 7. Anti-Patterns Avoided

- [x] No color-only information — all semantic states have icon + color
- [x] No text smaller than 12px for functional content (text-xs minimum)
- [x] No contrast violations — all text pairs pass WCAG AA 4.5:1
- [x] No hover states without visual feedback
- [x] No decorative glow without functional purpose (Focus Glow = orientation)
- [x] No gradient-accent (dropped: imperceptible at small sizes)
- [x] No neon/hexagon/backdrop-blur imports from futuristic-ai
- [x] prefers-reduced-motion respected

---

## 8. App-Wide Compatibility (Redesign Mode)

### Tokens Preserved from Existing App

- HSL-derived surface elevation (refined from 8L to 5L steps)
- Accent from ANSI blue palette[4] with saturation boost
- Semantic colors from ANSI red/green/yellow
- CubicOut easing (Fluent Design)
- Theme-agnostic architecture (all 5 bundled themes auto-generate coherent UI)
- Premultiplied alpha blending in wgpu pipeline

### Intentional Changes (Redesign)

| Change | Why |
|--------|-----|
| Added sans-serif font-ui (Segoe UI Variable) | Separates UI chrome from terminal content — biggest visual upgrade |
| Surface steps 8L → 5L | Finer granularity for refined-dark |
| Added Focus Glow | Signature element — solves orientation in multi-split |
| Added border-glow luminous separators | futuristic-ai influence — alive separators vs dead gray lines |
| Added status bar | Devtool convention, missing from current app |
| Text alphas adjusted (55/30 → 65/53) | WCAG compliance |

### Implementation Constraint

Use the existing `UiChrome` struct in `wmux-config/src/theme.rs` and extend it. Do NOT create a parallel color system. All new tokens must be derived from the same `derive_ui_chrome()` pipeline.

---

## 9. Implementation Handoff

### Visual Priorities (implement in this order)

1. **Focus Glow on active pane** — THE differentiator. Without this, the app is just another dark terminal. Use Approach A (concentric quads) first, upgrade to Approach B (shader) later.

2. **Dual font system** — Add sans-serif rendering for UI chrome via glyphon. Second font atlas. Largest visual impact after Focus Glow.

3. **Status bar** — New component. 28px, surface-1, workspace/connection/branch info. Devtool convention.

4. **Sidebar enrichment** — Section headers, workspace color dots, notification badges, hover transitions. Transform flat text list into rich navigation.

5. **Tab bar affordances** — Type indicators, close button on hover, unsaved dot.

6. **Notification panel severity** — Left stripe + muted bg tint per severity level.

7. **Border-glow luminous separators** — accent at 25% alpha on sidebar right edge and pane dividers.

8. **Empty states** — No workspaces, no notifications.

### Acceptance Criteria

- [ ] Focus Glow visible and animated on active pane (250ms CubicOut)
- [ ] Sans-serif renders correctly for all UI chrome at text-xs through text-xl
- [ ] Fallback to Segoe UI on Windows 10 without visual breakage
- [ ] Status bar shows workspace, pane count, connection status, branch
- [ ] All text pairs pass WCAG AA 4.5:1 contrast
- [ ] All 5 bundled themes auto-generate coherent UI chrome
- [ ] Hover states on all interactive elements with motion-micro timing
- [ ] Focus-visible ring on keyboard navigation
- [ ] Command palette open/close animation smooth at 60fps
- [ ] Reduced motion preference disables all animations except cursor blink
- [ ] Inactive pane dimming does NOT reduce text readability

### Quality Checkpoints

- [ ] Contrast ratios validated with WCAG calculator for all text/surface pairs
- [ ] Focus Glow renders correctly with 1, 2, 4, 6, and 8+ pane splits
- [ ] Sans-serif + monospace mixed rendering (sidebar label next to keyboard shortcut) aligned vertically
- [ ] All animations within 16ms frame budget (60fps)
- [ ] Theme switching (runtime) regenerates all derived tokens correctly
- [ ] DWM title bar colors match surface-1 on all themes

### Do NOT

- Do NOT use hardcoded hex values — always derive from theme palette
- Do NOT apply inactive_pane_opacity to text content — surface overlay only
- Do NOT add backdrop blur (Mica/Acrylic) in v1 — save for future enhancement
- Do NOT use Bold (700) weight — refined-dark uses Regular/Medium/SemiBold only
- Do NOT animate pane resize — must feel instant and direct
- Do NOT create a parallel color system — extend UiChrome

---

## 10. Stack Implementation Notes (native-gpu)

### Token Mapping

| Design Token | Implementation |
|---|---|
| Colors | `[f32; 4]` RGBA constants. Extend `UiChrome` struct in `theme.rs`. HSL manipulation for surface elevation. |
| Typography | glyphon `FontSystem` with two `Family` entries: `Family::Name("Segoe UI Variable")` for UI, terminal font for content. |
| Spacing | `f32` logical pixels. Multiply by `scale_factor` at render time. |
| Radius | SDF rounded-rect in `shader.wgsl` — already implemented. |
| Shadows | Expanded quads with alpha gradient (Approach A) or SDF outer glow (Approach B). |
| Transitions | `AnimationEngine` with `CubicOut`/`CubicIn` easing. Interpolate `[f32; 4]` color values per frame. |

### Windows-Specific

- DWM: `DWMWA_CAPTION_COLOR` = surface-1, `DWMWA_TEXT_COLOR` = text-primary, `DWMWA_BORDER_COLOR` = border-default
- Dark mode: `DWMWA_USE_IMMERSIVE_DARK_MODE` = 1
- Win10 fallback: Segoe UI (non-variable) — verify metrics at 12-14px
- Reduced motion: check `SystemParametersInfoW(SPI_GETCLIENTAREAANIMATION)`
- DPI: all measurements in logical pixels, apply `winit::window::Window::scale_factor()`
