# Task L4_01: Implement Command Palette with Fuzzy Search

> **Phase**: Polish
> **Priority**: P2-Medium
> **Estimated effort**: 2.5 hours

## Context
The command palette (Ctrl+Shift+P) provides quick access to all wmux commands via fuzzy search. Architecture §5 (wmux-ui) lists overlay.rs for command palette. PRD §11 describes the palette with cross-surface search (Ctrl+P).

## Prerequisites
- [ ] Task L2_08: Sidebar UI Rendering — provides overlay rendering infrastructure patterns

## Scope
### Deliverables
- Command palette overlay (centered, full-width popup)
- `CommandRegistry`: list of all available commands with names, descriptions, shortcuts
- Fuzzy search: type to filter commands by name
- Keyboard navigation: Up/Down to select, Enter to execute, Escape to close
- Command execution: dispatch selected command via AppCommand
- Cross-surface search (Ctrl+P): search terminal content across all surfaces

### Explicitly Out of Scope
- Custom command registration (post-MVP)
- Recently used commands ranking (post-MVP)

## Implementation Details
### Files to Create/Modify
| Action | Path | Purpose |
|--------|------|---------|
| Create | `wmux-ui/src/command_palette.rs` | Palette overlay, fuzzy search |
| Create | `wmux-core/src/command_registry.rs` | CommandEntry list |
| Modify | `wmux-ui/src/shortcuts.rs` | Add Ctrl+Shift+P and Ctrl+P |
| Modify | `wmux-ui/src/app.rs` | Integrate palette overlay |
| Modify | `wmux-core/src/lib.rs` | Export command_registry |

### Key Decisions
- **Fuzzy search algorithm**: Simple substring matching with scoring (prefer prefix match, then substring). Add fuzzy-matcher crate for smarter matching if needed
- **CommandEntry struct**: `{ name: String, description: String, shortcut: Option<String>, action: AppCommand }`
- **Overlay rendering**: Full-width box at top ~40% of window. Input field at top, results list below. Rendered via QuadPipeline + GlyphonRenderer

### Patterns to Follow
- PRD §11: "Overlay (Ctrl+Shift+P) avec recherche fuzzy"
- Architecture §5 wmux-ui: overlay.rs

### Technical Notes
- Palette input captures all keyboard input while open (overrides terminal input)
- Results update on every keystroke
- Max 20 visible results (scrollable if more match)
- Cross-surface search (Ctrl+P): search all surfaces' read_text output for query string. Results show surface name + matching line
- Palette closes on Escape, Enter (execute), or clicking outside
- Commands include: all shortcuts (new workspace, split, etc.), workspace switch by name, surface switch

## Success Criteria
- [ ] Ctrl+Shift+P opens command palette
- [ ] Typing filters commands by fuzzy match
- [ ] Up/Down navigates results, Enter executes
- [ ] Escape closes palette
- [ ] All major commands listed in palette
- [ ] Search results appear in < 100ms
- [ ] Ctrl+P searches across surface content
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
1. Ctrl+Shift+P → type "split" → verify split commands appear → Enter → verify split happens
2. Ctrl+P → type search term → verify results from multiple surfaces
3. Arrow keys → verify selection moves → Escape → verify closes
### Edge Cases to Test
- Very long command list (should scroll)
- Search with no matches (should show "No results")
- Palette open → Ctrl+Shift+P again → should close (toggle)
- Execute command that fails (palette closes, error handled normally)

## Dependencies
**Blocks**: None — leaf polish task

## References
- **PRD**: §11 Palette de Commandes
- **Architecture**: §5 wmux-ui (overlay.rs)
