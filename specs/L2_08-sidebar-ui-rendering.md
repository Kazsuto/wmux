---
task_id: L2_08
title: "Implement Sidebar UI Rendering"
status: pending
priority: P0
estimated_hours: 2.5
wave: 9
prd_features: [F-02, F-05]
archi_sections: [ADR-0001, ADR-0002, ADR-0003]
depends_on: [L2_07, L0_03]
blocks: [L2_14, L3_09, L3_14, L4_01, L4_05]
---

# Task L2_08: Implement Sidebar UI Rendering

> **Phase**: Core
> **Priority**: P0-Critical
> **Estimated effort**: 2.5 hours
> **Wave**: 9

## Context
The sidebar is a vertical panel on the left side of the window showing the list of workspaces with metadata. It's a key differentiator from Windows Terminal and tmux. Each workspace entry shows: name, git branch, CWD, status badges, progress bars, and log entries. Architecture §4 Component Diagram shows the UI Layer with sidebar. PRD §5 describes the Sidebar Metadata System.

## Prerequisites
- [ ] Task L2_07: Workspace Lifecycle — provides workspace list and metadata
- [ ] Task L0_03: QuadPipeline — provides colored rectangles for sidebar UI

## Scope
### Deliverables
- Sidebar panel rendering: fixed-width panel on left (default 220px, configurable)
- Workspace list: render each workspace as a row (name, active indicator)
- Metadata display per workspace: git branch, CWD path, status badges, progress bar, log entries
- Active workspace highlight
- Sidebar toggle (show/hide)
- Notification badge count per workspace
- Terminal viewport adjustment: terminal area starts after sidebar width

### Explicitly Out of Scope
- Sidebar metadata IPC (Task L2_14 — this task renders, not mutates)
- Drag-and-drop workspace reorder (post-MVP polish)
- Sidebar resize by dragging edge (post-MVP)
- Mica/Acrylic backdrop (Task L4_05)

## Implementation Details
### Files to Create/Modify
| Action | Path | Purpose |
|--------|------|---------|
| Create | `wmux-ui/src/sidebar.rs` | Sidebar renderer |
| Modify | `wmux-ui/src/app.rs` | Reserve sidebar space in layout, render sidebar |
| Modify | `wmux-render/src/lib.rs` | Sidebar uses GlyphonRenderer + QuadPipeline |

### Key Decisions
- **Custom wgpu rendering** (ADR-0002): Sidebar rendered via QuadPipeline (backgrounds, badges, progress bars) + GlyphonRenderer (text labels). No widget framework
- **Layout**: Window width = sidebar_width + terminal_area_width. PaneTree layout uses terminal_area as its viewport
- **Metadata rendering**: Status badges as colored rounded rectangles with icon + text. Progress bar as filled/unfilled rectangle. Logs as scrolling text list

### Patterns to Follow
- Architecture §4 Component Diagram: "UI Layer → Sidebar"
- Architecture §5 wmux-ui: "sidebar.rs — Sidebar rendering"
- PRD §5 Sidebar Metadata System: status badges, progress bars, logs

### Technical Notes
- Sidebar background: theme-specific color (slightly different from terminal background)
- Workspace row layout (top to bottom): name + git branch (line 1), cwd (line 2, truncated), status badges (line 3+), progress bar (if active), log entries (if any)
- Active workspace: highlighted background + accent left border
- Badge rendering: small colored circle/rectangle with text value (e.g., "🔵 Needs input")
- Progress bar: horizontal bar with fill percentage. Label text centered
- Notification badge: small number indicator on workspace row
- Sidebar width stored in AppState (for persistence)
- When sidebar is hidden, terminal area uses full window width
- Font: same monospace font as terminal, slightly smaller size for metadata

## Success Criteria
- [ ] Sidebar renders on left side with workspace list
- [ ] Active workspace is visually highlighted
- [ ] Git branch and CWD display (using placeholder data until Tasks L3_13/L3_14)
- [ ] Status badges render with icon, text, and color
- [ ] Progress bar renders with fill percentage
- [ ] Log entries render in sidebar area
- [ ] Sidebar toggle shows/hides sidebar
- [ ] Terminal area correctly adjusts width based on sidebar visibility
- [ ] `cargo clippy --workspace` zero warnings

## Validation Steps
### Automated Checks
```bash
cargo build --workspace
cargo clippy --workspace -- -W clippy::all
cargo test --workspace
cargo fmt --all -- --check
```
### Manual Verification
1. Run app — verify sidebar visible on left with workspace entries
2. Create new workspace — verify it appears in sidebar
3. Switch workspaces — verify active highlight moves
4. Verify terminal content starts after sidebar (no overlap)
### Edge Cases to Test
- Many workspaces (20+) — sidebar should scroll
- Very long workspace name — should truncate with ellipsis
- Very long CWD path — should truncate
- Window too narrow for sidebar + minimum pane — hide sidebar or enforce minimum

## Dependencies
**Blocks**:
- Task L2_14: Sidebar Metadata Store + IPC Handlers
- Task L3_09: Notification Visual Indicators
- Task L3_14: Git Branch Detection (populates git data)
- Task L4_01: Command Palette (overlay rendering infrastructure)
- Task L4_05: Mica/Acrylic Effects (sidebar backdrop)

## References
- **PRD**: §5 Sidebar Metadata System (badges, progress, logs), §2 Multiplexeur (sidebar)
- **Architecture**: §5 wmux-ui (sidebar.rs), §6 Sidebar Metadata Model
- **ADR**: ADR-0002 (custom wgpu rendering)
