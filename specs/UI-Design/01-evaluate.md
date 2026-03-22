---
name: wmux UI Evaluation
step: 01-evaluate
date: 2026-03-22
product_type: devtool
design_mode: redesign
---

# UI Audit: wmux — Full Application Interface

## Current State

**Screen name**: Full application (sidebar + terminal panes + command palette + notifications)
**Current primary style**: Devtools Dark (GitHub Dark palette)
**Current density**: Compact
**Audit date**: 2026-03-22
**Platform**: Windows 10/11 native (wgpu GPU-rendered)
**Audience**: Developers, power users, AI agents (via IPC)

**Overall assessment**: Strong technical foundation with coherent theme-derived color system. The auto-derived UiChrome approach ensures consistency. However, the interface currently reads as functional-first with minimal visual personality — closer to a raw terminal than a polished devtool like Warp, WezTerm, or Windows Terminal.

---

## What Works Well (PRESERVE)

| Element | Why It Works |
|---------|-------------|
| **HSL-derived surface elevation** | Elegant system: all surface levels computed from theme background. Guarantees visual coherence across any theme. This is architecturally superior to hardcoded values. |
| **Text hierarchy via alpha** | 100% → 55% → 30% is clean and effective. Creates clear information layers without additional colors. |
| **Accent from ANSI blue** | Smart: accent color always harmonizes with the terminal palette since it's derived from it. |
| **Active item left stripe** | 2-3px accent bar is a clear, minimal focus indicator. Effective devtool pattern (VS Code, JetBrains). |
| **Theme-agnostic architecture** | Any terminal theme (catppuccin, dracula, nord) automatically generates a coherent UI. This is a major strength. |
| **Command palette centered modal** | 600px wide, 12px radius, dimming overlay — follows established devtool conventions (VS Code, Raycast). |
| **Semantic colors from ANSI palette** | Red/green/yellow for error/success/warning — natural for terminal users. |
| **CubicOut easing** | Fluent Design standard — native Windows feel. |

## Issues to Address

| # | Issue | Severity | Category | Current State | Recommendation |
|---|-------|----------|----------|---------------|----------------|
| 1 | **No sans-serif font for UI chrome** | High | Typography | Everything is monospace — sidebar labels, command palette, notifications all use terminal font. UI chrome text should use a proportional font for readability and visual distinction. | Add a system sans-serif (Segoe UI on Windows) for non-terminal UI elements. Keep monospace for terminal content only. |
| 2 | **No status bar** | High | Component | No bottom status bar showing workspace info, connection status, or mode indicators. Devtools convention (VS Code, terminals). | Add a compact status bar (24-28px) at bottom with workspace name, pane count, connection status. |
| 3 | **Sidebar lacks visual richness** | Medium | Visual | Sidebar rows are flat rectangles with text only. No icons, no visual hierarchy beyond active stripe. Compares poorly to Windows Terminal or Warp sidebar. | Add workspace icons/avatars, subtle hover transitions, section headers. |
| 4 | **No visual distinction between pane types** | Medium | Visual | Terminal panes, browser panes (WebView2), and future split panes all look identical. No visual cue for pane type. | Add subtle type indicators — icon or colored top-border per pane type. |
| 5 | **Shadow only on command palette** | Low | Visual | Only one element has a shadow. Notification panel and other overlays lack depth cues. | Add consistent soft shadows to all overlay/floating elements. |
| 6 | **No empty states** | Medium | Component | No visual treatment when sidebar is empty, no workspaces exist, or terminal has no output. | Design empty states with illustration or guidance text. |
| 7 | **Notification panel lacks severity styling** | Medium | Visual | All notifications look the same regardless of severity (error vs info vs success). | Use semantic color accent (left stripe or icon tint) per notification level. |
| 8 | **Tab bar lacks close/action affordances** | Medium | Interaction | Tab pills show text only. No close button, no context menu indicator, no unsaved indicator. | Add hover-reveal close button, unsaved dot indicator. |

## Missed Opportunities

1. **Window backdrop effects**: Mica/Acrylic support is already detected in `effects.rs` but not applied to any surface. Using Mica on the sidebar or Acrylic on overlays would give wmux a native Windows 11 premium feel that competitors lack. (impact: high)

2. **Micro-animations on state changes**: The animation engine exists (`animation.rs` with CubicOut easing) but is underutilized. Pane focus transitions, sidebar item reordering, notification slide-in — these would add polish. (impact: medium)

3. **Brand identity through accent treatment**: The accent is always flat blue. A subtle gradient on the accent stripe, or a glow effect on focus, would differentiate wmux from generic terminal apps. (impact: medium)

## App Context Alignment

- **Patterns followed**: Surface elevation, text alpha hierarchy, accent from ANSI blue, CubicOut easing, dark mode DWM integration — all consistent and well-implemented.
- **Deviations**: None detected — the design system is internally consistent but limited in scope.

---

## Priority Fixes (Top 3)

### Priority 1: Sans-serif UI font (#1)
- **Why**: Monospace for everything makes the UI feel like a raw terminal, not a polished app. This is the single highest-impact change.
- **Effort**: Low (glyphon supports multiple font families)

### Priority 2: Status bar (#2)
- **Why**: Every major devtool has one. Missing it makes wmux feel incomplete. Critical for showing workspace context.
- **Effort**: Medium (new component, but follows existing patterns)

### Priority 3: Sidebar enrichment (#3)
- **Why**: Sidebar is the primary navigation. Flat text-only rows feel unfinished compared to competitors.
- **Effort**: Medium (icons, transitions, section headers)

---

## Design Debt Summary

**Total issues**: 8
**Critical/High**: 2 — Sans-serif font, status bar
**Medium**: 4 — Sidebar richness, pane types, empty states, notification severity, tab affordances
**Low**: 1 — Consistent shadows

**Overall design health**: 5/10 — Solid technical architecture with a coherent color system, but the visual execution is functional-first. The app looks like it was built by engineers (coherent, logical) rather than designed (polished, personality). The theme derivation system is excellent — the challenge is layering visual richness on top of it.

---

## Product Type

**Identified**: `devtool` — Developer-oriented terminal multiplexer with keyboard-first interaction, dark mode default, code/terminal as primary content, command palette as core navigation paradigm.
