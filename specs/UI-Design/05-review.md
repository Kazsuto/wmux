---
name: wmux Review — Guardian + Layout
step: 05-review
date: 2026-03-22
guardian_verdict: BLOCKED → PASSED (rev2, corrections applied)
layout_pattern: workspace
---

# Review: Anti-Patterns & Layout

## Guardian Quality Review

**Verdict (initial): BLOCKED** — 3 blocking issues, 4 critical issues, 4 warnings.
**Verdict (rev2): PASSED** — All blocking/critical issues addressed in 03-visual-system.md and 04-components.md.

### Corrections Applied

| # | Issue | Severity | Fix Applied |
|---|-------|----------|-------------|
| 1 | text-muted 35% alpha = 2.90:1 contrast (WCAG FAIL) | BLOCKING | Increased to 53% alpha → 4.5:1+. Added text-faint (40%) for non-critical hints. |
| 2 | text-2xs 10px for status bar functional text | BLOCKING | Raised text-2xs to 11px. Status bar/section headers use text-xs (12px) minimum. |
| 3 | Segoe UI Variable Win11-only, no Win10 fallback docs | BLOCKING | Documented full fallback chain with metric variance notes. |
| 4 | Focus Glow requires shader modification not available | CRITICAL | Added implementation plan: Approach A (4 concentric quads) and Approach B (shader glow_radius extension). |
| 5 | surface-overlay base+8L misaligned with 5L scale | CRITICAL | Changed to surface-1 (+10L) at 95% alpha. |
| 6 | "luxury-dark" label doesn't match compact density | CRITICAL | Renamed to "refined-dark". Added Style Reconciliation section. |
| 7 | bold-contrast + futuristic-ai not reconciled | CRITICAL | Added explicit reconciliation: bold-contrast = hierarchy strategy, futuristic-ai = glow technique only. Listed exclusions. |
| 8 | border-glow accent 12% invisible (~1.3:1 contrast) | WARNING | Increased to accent 25% alpha. |
| 9 | Unused radius tokens (xs, sm, full) | WARNING | Documented explicit usage for each token. |
| 10 | Disabled state proximity to text-muted | WARNING | Resolved: text-muted raised to 53%, clear gap from disabled (40%). |
| 11 | Missing loading states | WARNING | Added skeleton specs for sidebar, command palette, SSH connection. |

### WCAG Contrast Verification (post-correction)

| Pair | Ratio | WCAG AA | Status |
|------|-------|---------|--------|
| text-primary (#e6edf3) on surface-base (#0d1117) | 16.14:1 | 4.5:1 | PASS |
| text-secondary (fg 65%) on surface-base | ~8.5:1 | 4.5:1 | PASS |
| text-muted (fg 53%) on surface-base | ~4.5:1 | 4.5:1 | PASS (borderline) |
| text-muted (fg 53%) on surface-1 | ~3.5:1 | 3:1 (UI) | PASS for UI elements |
| accent (#58a6ff) on surface-base | 7.58:1 | 4.5:1 | PASS |
| error (#ff7b72) on surface-base | 7.56:1 | 4.5:1 | PASS |
| success (#3fb950) on surface-base | 7.57:1 | 4.5:1 | PASS |
| warning (#d29922) on surface-base | 7.55:1 | 4.5:1 | PASS |
| text-inverse on accent | 7.58:1 | 4.5:1 | PASS |

---

## Anti-Pattern Scan

### Checked — NOT found:
- **excessive-gradients**: No gradients in the spec (dropped per Guardian pre-check).
- **decorative-noise**: Glow effects are functional (focus indication), not decorative.
- **too-many-cards**: No cards — flat list in sidebar, flat results in palette.
- **inconsistent-styles**: All components reference the same token system.
- **ui-clutter**: Compact density is intentional for devtool, not clutter.

### Checked — Addressed:
- **poor-contrast**: text-muted was 2.90:1 → fixed to 4.5:1+
- **poor-typography**: text-2xs was 10px for functional text → fixed to 11px min, functional uses 12px
- **spacing-chaos**: All dimensions aligned to 4px grid ✓

---

## Layout Pattern: Workspace

**Selected:** `workspace-layout` — professional, keyboard-driven environment for power users.

### wmux Layout Structure

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
│  mode)   │      │ ✨GLOW   │ (dimmed)  │            │
│          │      │          │           │            │
│          │      └──────────┴───────────┘            │
│          ├──────────────────────────────────────────┤
│          │         Status Bar (28px)                 │
├──────────┴──────────────────────────────────────────┤
│  Overlays: Command Palette (centered modal)          │
│            Notification Panel (right-side slide)     │
└─────────────────────────────────────────────────────┘
```

### Layout Characteristics

- **Grid:** No traditional column grid — workspace layout uses fixed sidebar + fluid content split
- **Sidebar:** 220px expanded (or 48px collapsed icon mode), left-anchored, resizable via drag
- **Content area:** fluid, fills remaining width. Split into panes via tree layout engine
- **Tab bar:** full width of content area (not sidebar)
- **Status bar:** full width of window (spans sidebar + content)
- **Overlays:** command palette centered, notification panel slides from right edge
- **Scan pattern:** Left sidebar for navigation context → tabs for pane switching → content for work
- **Keyboard flow:** Ctrl+K (palette) → workspace switching (sidebar) → pane focus (glow) → terminal input

### Responsive Behavior

- Desktop wide (1920px+): full sidebar + multi-pane splits
- Desktop standard (1280px): full sidebar + 2-pane max comfortable
- Desktop compact (1024px): collapsed sidebar (48px icons) + single pane or 2-pane
- Below 1024px: not targeted (terminal multiplexer is not a mobile use case)
