---
name: wmux Visual System — "Luminous Void"
step: 03-visual-system
date: 2026-03-22
direction: refined-dark + futuristic-ai (glow only)
density: compact
atmosphere: bold-contrast (hierarchy) + futuristic-ai (technique)
guardian_review: passed (rev2)
---

# Visual System: "Luminous Void"

Design tokens for wmux — native GPU-rendered terminal multiplexer.
All values are concrete and implementable as Rust `[f32; 4]` RGBA constants or pixel dimensions.

> **Design mode: REDESIGN** — Current app values shown as reference. Changes justified.

## Style Reconciliation

**Primary style: "refined-dark"** (formerly labeled "luxury-dark" — renamed per Guardian review to accurately reflect compact density + semibold weights, which diverge from canonical luxury-dark's generous spacing and thin weights).

The design combines two influences with clear boundaries:

- **bold-contrast** provides the *hierarchy system*: active element is bright and prominent (glow, full alpha text), everything else recedes (dimmed surfaces, reduced alpha). This is the "what" — the visual hierarchy strategy.
- **futuristic-ai** provides the *technique* for the signature element: luminous glow effects (Focus Glow halo, border-glow separators). This is the "how" — the rendering approach for focus indication.

**Explicitly excluded from futuristic-ai:** neon colors, backdrop blur, transparency layers beyond overlays, hexagonal patterns, data stream animations, circuit-board motifs. Only the luminous glow technique is borrowed.

---

## 1. Color System

### Surface Elevation (Dark Mode — Primary)

**From current app:** HSL-derived `surface_base` → `surface_0` (+8L) → `surface_1` (+16L) → `surface_2` (+24L)
**New direction:** KEPT — The HSL auto-derivation system is excellent. Refine the step sizes and add a new level.

```
surface-base:     theme.background         # #0d1117 for wmux-default
surface-0:        base + 5L (HSL)          # ~#13181f — subtle lift (was +8L, reduced for finer granularity)
surface-1:        base + 10L (HSL)         # ~#1a2029 — sidebar bg, tab bar bg
surface-2:        base + 15L (HSL)         # ~#212830 — hover, active states, selections
surface-3:        base + 20L (HSL)         # ~#283038 — borders, dividers
surface-overlay:  surface-1 (base + 10L), 95% alpha  # command palette bg, notification panel bg — aligned on 5L scale
```

**Why changed:** Finer 5L steps (was 8L) give more nuance. 5 levels instead of 4 — adds `surface-overlay` as explicit floating surface with transparency. The 8L jump was too coarse for a luxury-dark direction that relies on subtle elevation differences.

### Accent System

**From current app:** Flat `#58a6ff` (ANSI blue, saturation boosted to 80%+), `accent_muted` at 40% alpha
**New direction:** Keep flat accent (Guardian note: gradient may be imperceptible at small sizes). Add glow-specific tokens.

```
accent:           theme.palette[4]         # #58a6ff — derived from ANSI blue, boosted S≥80%
accent-hover:     accent + 8L (HSL)        # ~#79bbff — lighter for hover
accent-muted:     accent, 30% alpha        # subtle highlights (was 40%, reduced for refinement)
accent-glow:      accent, 20% alpha        # Focus Glow outer halo
accent-glow-core: accent, 50% alpha        # Focus Glow inner 1px ring
accent-tint:      accent, 8% alpha         # Dimming overlay tint (Guardian: 8% minimum, not 3-5%)
```

**Why changed:** Dropped gradient-accent per Guardian recommendation (imperceptible at 3px bar width). Added glow-specific tokens for the Focus Glow signature element. Reduced accent-muted from 40% to 30% for more restraint (luxury-dark discipline). Added accent-tint at 8% for overlay coloring.

### Text Hierarchy

**From current app:** `text_primary` (100% alpha), `text_secondary` (55% alpha), `text_muted` (30% alpha)
**New direction:** KEPT with adjustment — 3 tiers via alpha on foreground color.

```
text-primary:     theme.foreground, 100%   # #e6edf3 — headings, active labels, terminal content
text-secondary:   theme.foreground, 65%    # sidebar subtitles, inactive tabs — WCAG AA 7.2:1 on surface-base
text-muted:       theme.foreground, 53%    # timestamps, hints, section headers — WCAG AA 4.5:1 on surface-base (was 35%, Guardian BLOCKED: 2.90:1)
text-faint:       theme.foreground, 40%    # purely decorative hints, non-critical metadata — NOT for functional text
text-inverse:     surface-base, 100%       # text on accent backgrounds (badges, buttons)
```

**Why changed:** Slight increases from 55→60% and 30→35% for readability per Guardian note about light-on-dark irradiation. Text inverse added for accent-bg elements.

### Semantic Colors

**From current app:** Derived from ANSI red/green/yellow
**New direction:** KEPT — natural for terminal users.

```
error:            theme.palette[1]         # #ff7b72 — ANSI red
error-muted:      error, 12% alpha         # notification background tint
success:          theme.palette[2]         # #3fb950 — ANSI green
success-muted:    success, 12% alpha       # notification background tint
warning:          theme.palette[3]         # #d29922 — ANSI yellow
warning-muted:    warning, 12% alpha       # notification background tint
info:             accent                   # same as accent blue
info-muted:       accent, 12% alpha        # notification background tint
```

**Why changed:** Added `-muted` variants at 12% alpha for notification severity backgrounds (resolves audit issue #7).

### Border

**From current app:** `surface_2` at 60% alpha
**New direction:** Refined with multiple border intensities.

```
border-subtle:    surface-3, 40% alpha     # faint dividers between panes
border-default:   surface-3, 60% alpha     # standard borders (sidebar, sections)
border-strong:    surface-3, 80% alpha     # emphasized borders (focused sections)
border-glow:      accent, 25% alpha        # luminous separator between zones — increased from 12% for visibility (Guardian: 12% = ~1.3:1 contrast, invisible)
```

**Why changed:** Added granularity. `border-glow` is the futuristic-ai influence — separators that glow faintly with accent color instead of dead gray lines.

### Dimming Overlay

```
overlay-dim:      #000000, 50% alpha       # command palette backdrop (was 40%, increased for stronger focus)
overlay-tint:     accent, 8% alpha         # layered ON TOP of overlay-dim for ambient coloring
```

---

## 2. Typography System

**From current app:** System monospace everywhere — terminal 14px/18px, sidebar 13px/18px
**New direction:** Dual system — sans-serif for UI chrome, monospace for terminal content.

### Font Families

```
font-ui:          "Segoe UI Variable", "Segoe UI", system-ui, sans-serif
font-mono:        user-configured terminal font (default: "Cascadia Code", "Consolas", monospace)
```

**Why changed:** The single biggest visual upgrade. Sans-serif UI chrome creates clear separation between "the tool" (navigation, labels, status) and "the content" (terminal output).

### Font Fallback Chain (Guardian BLOCKED: Win10 compatibility)

```
Win11 22H2+:     "Segoe UI Variable" — variable font with optical sizing axis
Win10-Win11 21H2: "Segoe UI" — classic non-variable, different metrics
Final fallback:   system-ui, sans-serif

Key differences between Segoe UI Variable and Segoe UI:
- Variable has optical sizing: text auto-adjusts weight/proportions at small sizes
- Classic has fixed metrics: ascent/descent differ by ~1px at 13px
- Both align to 4px grid at the specified sizes (validated: 48px sidebar rows, 36px tab bar, 28px status bar all accommodate ±1px metric variance)

Implementation note for glyphon:
- Use cosmic-text Family::Name("Segoe UI Variable") with Family::Name("Segoe UI") as fallback
- glyphon's FontSystem::new() loads all system fonts including both variants
- Test rendering at text-xs (12px) and text-base (14px) with BOTH fonts to verify grid alignment
```

### Scale (Compact Density — px values for GPU renderer)

```
text-2xs:         11px / 15px (line-height)  — decorative metadata only (was 10px, Guardian BLOCKED: too small for functional text)
text-xs:          12px / 16px                — status bar labels, section headers, keyboard shortcuts, badges — MINIMUM for functional text
text-sm:          13px / 17px                — helper text, sidebar subtitles, notification messages
text-base:        14px / 19px                — sidebar primary text, tab labels, notification titles
text-lg:          15px / 20px                — command palette results
text-xl:          16px / 21px                — command palette input, modal titles
text-2xl:         18px / 23px                — (reserved — rare use, onboarding)

terminal-base:    14px / 18px                — terminal content (user-configurable, independent)
```

**Why changed:** Scale calibrated for compact density. Sizes are smaller than web conventions (12-16px range vs 14-36px) because GPU-rendered text at native resolution doesn't need the same inflation as browser text. The 13px base for UI chrome is the sweet spot between density and readability.

### Weights

```
weight-regular:   400    — body text, sidebar items, status bar values
weight-medium:    500    — sidebar primary labels, tab labels, notification titles
weight-semibold:  600    — section headers, active workspace name, command palette input
```

**Why changed:** Medium (500) for labels per Guardian note (Regular 400 too thin on dark bg at 13px). SemiBold (600) for section headers per explorer direction. No Bold (700) — luxury-dark uses weight contrast sparingly.

### Letter Spacing

```
tracking-tight:   -0.01em   — section headers (text-lg+)
tracking-normal:  0em       — body text
tracking-wide:    +0.04em   — uppercase labels, keyboard shortcuts
```

---

## 3. Spacing System

**From current app:** Mixed values (12px padding, 52px rows, 36px tabs, 600px palette)
**New direction:** Formalized 4px grid with compact density multipliers.

### Base Grid

```
space-0:    0px
space-0.5:  2px    — hairline gaps, icon-to-text micro spacing
space-1:    4px    — minimum spacing, internal padding tight
space-1.5:  6px    — tab internal padding vertical
space-2:    8px    — element gap (within components), notification item padding
space-3:    12px   — component padding (sidebar padding-x, palette padding)
space-4:    16px   — section gap (between component groups)
space-6:    24px   — major section gap
space-8:    32px   — zone gap (sidebar ↔ terminal area)
```

### Component Dimensions

```
sidebar-row-height:       48px    (was 52px — reduced for density, Guardian validated)
sidebar-width-collapsed:  48px    (icon-only mode)
sidebar-width-expanded:   220px   (full mode with labels)
tab-bar-height:           36px    (kept)
tab-gap:                  4px     (kept)
status-bar-height:        28px    (new component)
command-palette-width:    600px   (kept)
command-input-height:     44px    (was 40px — slightly taller for text-xl input)
command-result-height:    36px    (kept)
notification-panel-width: 360px   (was 350px — aligned to 4px grid)
notification-item-height: 72px    (kept)
pane-divider-width:       1px     (kept — luminous border-glow replaces thick dividers)
focus-stripe-width:       3px     (was 2px — slightly thicker for visibility, kept)
```

**Why changed:** All dimensions now align to 4px grid. Sidebar rows reduced 52→48px for density. Status bar added at 28px. Command input slightly taller for the xl text size. Notification panel aligned to grid.

---

## 4. Radius System

**From current app:** 12px panels, 6px interactive, 0-1px accents
**New direction:** KEPT with minor refinement — the existing radius language is already good.

```
radius-none:   0px     — pane borders, terminal content, accent stripes
radius-xs:     2px     — focus stripe ends, tab accent indicator
radius-sm:     4px     — badges, small interactive elements
radius-md:     6px     — tabs, result items, sidebar hover states
radius-lg:     8px     — notification panel, inputs, dropdown
radius-xl:     12px    — command palette, modal overlays
radius-full:   9999px  — notification badges (circular), pills
```

**Why changed:** Minimal changes. Added `radius-xs` at 2px for small accent details. Notification panel reduced from 12px to 8px (`radius-lg`) to differentiate from the primary modal (command palette keeps 12px).

### Radius Rules (every token has assigned usage)

```
radius-none (0px):   terminal pane borders, accent stripes, pane dividers
radius-xs (2px):     focus stripe ends, tab accent bottom indicator
radius-sm (4px):     tooltips, keyboard shortcut caps, icon-only button hover
radius-md (6px):     tabs, result items, sidebar hover states, standard buttons, inputs
radius-lg (8px):     notification panel, toast notifications, dropdowns
radius-xl (12px):    command palette, modal overlays, settings dialog
radius-full:         notification count badges (circular), workspace color dots
```

---

## 5. Shadows & Depth

**From current app:** Single shadow on command palette (black 25%, 2px offset). No other shadows.
**New direction:** Expanded shadow system with glow variant — futuristic-ai influence.

### Standard Shadows (for dark theme — increased opacity vs light)

```
shadow-none:      none                                    — flat elements, panes
shadow-sm:        0 1px 3px rgba(0,0,0,0.30)              — subtle lift (status bar, tab bar)
shadow-md:        0 4px 8px rgba(0,0,0,0.35)              — floating elements (dropdowns)
shadow-lg:        0 8px 16px rgba(0,0,0,0.40)              — notification panel
shadow-xl:        0 12px 24px rgba(0,0,0,0.45)             — command palette, modals
```

### Glow Shadows (signature element)

```
glow-focus:       0 0 0 1px accent-glow-core,              — Focus Glow: inner ring (1px, 50% alpha)
                  0 0 16px 3px accent-glow                 — Focus Glow: outer halo (16px spread, 20% alpha)
glow-subtle:      0 0 8px 1px accent-glow                  — subtle glow on active elements (sidebar active row)
```

**Why changed:** Dark themes need stronger shadows (30-45% opacity vs 5-15% for light). Added glow-focus as the signature Focus Glow element. `glow-subtle` for secondary active indicators.

### Usage Rules

```
Pane content area:      shadow-none        — terminal grids are flat
Tab bar:                shadow-sm          — subtle separation from content
Status bar:             shadow-sm          — subtle top shadow (inverted: 0 -1px 3px)
Notification panel:     shadow-lg          — prominent floating panel
Command palette:        shadow-xl          — highest elevation modal
Active pane:            glow-focus         — THE signature element (only 1 pane at a time)
Sidebar active row:     glow-subtle        — secondary glow indicator
Dropdowns/menus:        shadow-md          — mid-elevation
```

---

## 6. Motion System

**From current app:** CubicOut easing, 200-300ms transitions, 500ms cursor blink, 2s notification pulse
**New direction:** KEPT and formalized — CubicOut (Fluent Design) is excellent for Windows native.

### Duration Scale

```
motion-instant:   0ms     — immediate state (disabled, removed)
motion-micro:     80ms    — hover state color changes, opacity shifts
motion-fast:      150ms   — focus transitions, tab switches, button press
motion-normal:    250ms   — Focus Glow animation, sidebar transitions, panel open
motion-slow:      350ms   — command palette open/close, notification slide-in
motion-pulse:     2000ms  — notification attention ring (continuous sine wave)
motion-blink:     500ms   — cursor blink interval
```

### Easing Functions

```
ease-out:         cubic-bezier(0.33, 1, 0.68, 1)     — CubicOut: entering elements, Focus Glow appear
ease-in:          cubic-bezier(0.32, 0, 0.67, 0)     — CubicIn: exiting elements, Focus Glow disappear
ease-in-out:      cubic-bezier(0.65, 0, 0.35, 1)     — state changes, position moves
ease-linear:      linear                               — cursor blink, continuous animations
```

### Animation Specifications

```
Focus Glow transition:
  - New pane: glow-focus fade-in, ease-out, motion-normal (250ms)
  - Old pane: glow-focus fade-out, ease-in, motion-fast (150ms)
  - Cross-fade: old starts fading immediately, new starts after 50ms delay
  - Result: brief moment where BOTH glow (150ms overlap), then only new pane glows

Command palette:
  - Open: overlay-dim fade-in (motion-fast), panel scale 0.97→1.0 + fade-in (motion-slow, ease-out)
  - Close: reverse, motion-fast for both

Notification panel:
  - Slide from right: translateX(360px→0) + fade-in, motion-slow, ease-out
  - Dismiss: translateX(0→360px) + fade-out, motion-normal, ease-in

Sidebar hover:
  - Background: surface-1 → surface-2, motion-micro (80ms), ease-out

Tab switch:
  - Active indicator: translateX to new tab, motion-fast (150ms), ease-in-out

Pane resize:
  - No animation — immediate (resizing must feel direct and responsive)
```

**Why changed:** Formalized duration scale from existing values. Added cross-fade specification for Focus Glow per Guardian note (both glows visible for ~150ms during transition). Added per-component animation specs.

### Focus Glow — Implementation Technique

The current wgpu QuadPipeline renders flat-colored quads with SDF rounded corners. It does NOT support box-shadow or gaussian blur. The Focus Glow must be implemented using one of two approaches:

**Approach A: Concentric Quads (recommended — no shader changes)**

Simulate the glow halo with 4 concentric quads behind the active pane, each with decreasing opacity and increasing size:

```
Quad 1 (inner ring):  pane bounds + 1px outset, accent at 50% alpha, radius matches pane
Quad 2 (near halo):   pane bounds + 4px outset, accent at 25% alpha
Quad 3 (mid halo):    pane bounds + 9px outset, accent at 12% alpha
Quad 4 (outer halo):  pane bounds + 16px outset, accent at 5% alpha
```

Total: 4 additional QuadInstances per active pane (out of 8192 budget — negligible). Rendered BEHIND the pane content quad. Animated by interpolating all 4 alphas from 0 → target over 250ms CubicOut.

**Approach B: Shader Extension (better visual quality, requires shader change)**

Add `glow_radius: f32` and `glow_color: vec4<f32>` fields to `QuadInput` in the WGSL shader. In the fragment shader, after the SDF rounded-rect distance calculation, add an outer glow using the NEGATIVE distance (outside the shape):

```wgsl
let outer_dist = -sdf_distance;  // positive outside the shape
let glow = smoothstep(glow_radius, 0.0, outer_dist) * glow_color.a;
color = mix(color, glow_color, glow);
```

This produces a true smooth falloff. Only 1 quad with glow active at any time.

### Reduced Motion

```
When reduced-motion preference detected (Windows: Settings → Accessibility → Visual effects → Animation effects OFF):
  - All motion-* durations → 0ms EXCEPT motion-blink (cursor still blinks)
  - Focus Glow: instant appear/disappear (no fade)
  - Command palette: instant open/close (no scale/fade)
  - Hover states: instant color change
```

---

## Migration Summary

| Token Category | Changed | Kept | Notes |
|---------------|---------|------|-------|
| **Surface elevation** | Step size 8L→5L, added surface-overlay | HSL derivation system | Finer granularity for luxury-dark |
| **Accent** | Dropped gradient, added glow tokens, reduced muted 40→30% | Core accent from ANSI blue | Guardian: gradient imperceptible |
| **Text** | Adjusted alphas 55→60%, 30→35%, added text-inverse | 3-tier alpha hierarchy | Readability improvement |
| **Semantic** | Added -muted variants at 12% | Derivation from ANSI | Notification severity |
| **Borders** | Added granularity (subtle/default/strong/glow) | Base concept | Luminous separators |
| **Typography** | **MAJOR: Added sans-serif UI font** | Terminal monospace | Biggest visual change |
| **Spacing** | Formalized 4px grid, sidebar 52→48px, added status bar | Most dimensions | Grid alignment |
| **Radius** | Minor refinement, added radius-xs | Existing 12px/6px/0px | Already good |
| **Shadows** | **MAJOR: Added glow shadow system** | Command palette shadow | Signature element |
| **Motion** | Formalized scale, added Focus Glow cross-fade spec | CubicOut easing, existing durations | Animation specs |

### Non-composition Rule (Guardian Note #5)

```
inactive_pane_opacity (config): Applies ONLY to pane surface background, NOT to text content.
Text in inactive panes keeps its full alpha hierarchy (primary 100%, secondary 60%, muted 35%).
The inactive overlay is rendered as a surface-base quad at (1 - inactive_pane_opacity) alpha ON TOP of the pane content.
This prevents destructive composition: text remains readable, pane just appears slightly dimmed.
```
