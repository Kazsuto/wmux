---
name: wmux Team Synthesis
step: 02-team-synthesis
date: 2026-03-22
chosen_direction: audacieux
concept: "Luminous Void"
primary_style: luxury-dark
secondary_style: futuristic-ai
density: compact
atmosphere: bold-contrast
---

# Team Synthesis — Direction Comparison

## Chosen Direction: Audacieux — "Luminous Void"

### State Variables Set

- **ui_objective**: Immersion + orientation instantanée — identifier la pane active, le workspace courant et l'état système en < 1 seconde
- **density**: compact (36px tabs, 48px sidebar rows, 28px status bar, 4px base grid)
- **primary_style**: luxury-dark
- **secondary_style**: futuristic-ai (lueurs colorées, bordure lumineuse 1px, transparence par couches)
- **atmosphere**: bold-contrast (hiérarchie par contraste sévère — actif éclatant, reste en retrait)
- **inspirations**: Linear (densité, command palette, hiérarchie par opacité), Vercel (discipline monochromatique, terminal comme citoyen première classe)

### Explorer Reasoning (for Guardian Review)

**Why luxury-dark as primary:**
The discipline of luxury-dark (single accent, fine typography, multi-layer subtle shadows) prevents visual chaos in a tool where users already have lots to look at (code, logs, splits). It's not about commercial luxury — it's about visual discipline.

**Why futuristic-ai as secondary influence:**
Only borrowing: luminous 1px border with colored box-shadow, and animated transitions on state changes. NOT taking: hexagons, visible grids, data animations.

**Why bold-contrast atmosphere:**
Not brutal black-white, but focus-first hierarchy. Active element is bright (accent glow), everything else recedes (desaturated surfaces, 55% alpha text). Amplifies keyboard-driven focus readability.

**Signature element — Focus Glow:**
Luminous halo (box-shadow: 0 0 0 1px accent, 0 0 12px 2px accent@20%) around active terminal pane. Animates on pane switch (250ms CubicOut). Implementable in WGSL shader with glow_radius + glow_color parameters and gaussian falloff. Only 1 quad with glow active at any time (performance safe).

**Trade-offs acknowledged:**
- Gradient-accent may age poorly (mitigated: only 3 elements, easily swapped to flat)
- Sans-serif may alienate terminal purists (mitigated: `ui-font = "monospace"` config toggle)
- Tinted dimming overlay may be invisible on low-saturation themes (mitigated: minimum saturation 80%)

**Competitive positioning:**
Market is saturated with flat/generic dark terminals. Focus Glow creates a "screenshottable" identity (developer shares on Twitter/X). No terminal multiplexer has luminous focus as a visual identity.

## Competitive Analysis Summary

### Top Competitors
| Product | Distinctive Element | Weakness |
|---------|-------------------|----------|
| Warp | Command blocks, glassmorphism | Heavy chrome, login wall |
| Windows Terminal | Mica/Acrylic native | Generic tabs, no sidebar |
| WezTerm | Lua-scriptable powerline tabs | Brutally basic default |
| Ghostty | Best font rendering, GLSL shaders | No Windows, no sidebar |
| Alacritty | Zero chrome | No tabs, no splits |
| Zellij | TUI status bar with shortcuts | ASCII-only, no GPU |

### Key Differentiation Opportunities
1. Sidebar verticale persistante (aucun concurrent)
2. Notification panel contextuel
3. Windows-native premium (Mica as identity)
4. GPU-rendered pane dividers with contextual color
5. Focus Glow as unique visual signature

### Anti-patterns to Avoid
1. Delegating all design to user config (WezTerm/tmux)
2. Horizontal-only tabs (doesn't scale past 6-8)
3. Inconsistent chrome vs terminal tokens
4. Generic copy-paste command palette
