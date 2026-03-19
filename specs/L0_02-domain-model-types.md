# Task L0_02: Create Domain Model Types in wmux-core

> **Phase**: Scaffold
> **Priority**: P0-Critical
> **Estimated effort**: 2 hours

## Context

wmux's conceptual hierarchy (Window > Workspace > Pane > Surface > Panel) requires strongly typed identifiers and core data structures shared across all crates. These types form the vocabulary of the entire application. PRD §Modèle Conceptuel defines the 5-level hierarchy. Architecture §5 (wmux-core) specifies the domain model responsibility.

## Prerequisites

- [ ] Task L0_01: Error Types and Tracing Infrastructure — provides error module and dependencies

## Scope

### Deliverables
- ID types: `WindowId`, `WorkspaceId`, `PaneId`, `SurfaceId` (newtype UUIDs)
- `Cell` struct with grapheme cluster, foreground/background colors, attribute flags
- `Row` type (Vec<Cell>)
- `CursorState` (position, shape, visible, blinking)
- `CursorShape` enum (Block, Underline, Bar)
- `TerminalMode` bitflags (origin, wraparound, bracketed_paste, application_cursor, mouse_reporting modes)
- `SplitDirection` enum (Horizontal, Vertical)
- `Color` enum (Named ANSI 0-15, Indexed 0-255, Rgb)
- `CellFlags` bitflags (Bold, Italic, Underline, Strikethrough, Inverse, Hidden, DimBold)
- `PanelKind` enum (Terminal, Browser)
- `SurfaceInfo` struct (id, kind, title)

### Explicitly Out of Scope
- Grid implementation (Task L1_01)
- Scrollback buffer (Task L1_03)
- VTE parsing (Task L1_02)
- Workspace/Pane tree logic (Tasks L2_02, L2_07)

## Implementation Details

### Files to Create/Modify
| Action | Path | Purpose |
|--------|------|---------|
| Create | `wmux-core/src/types.rs` | ID newtypes (WindowId, WorkspaceId, PaneId, SurfaceId) |
| Create | `wmux-core/src/cell.rs` | Cell struct, CellFlags bitflags, Row type |
| Create | `wmux-core/src/color.rs` | Color enum (Named, Indexed, Rgb) |
| Create | `wmux-core/src/cursor.rs` | CursorState, CursorShape |
| Create | `wmux-core/src/mode.rs` | TerminalMode bitflags |
| Create | `wmux-core/src/surface.rs` | PanelKind, SurfaceInfo |
| Modify | `wmux-core/src/lib.rs` | Re-export all public types |
| Modify | `wmux-core/Cargo.toml` | Add `uuid`, `bitflags`, `serde` dependencies |

### Key Decisions
- **UUID for all IDs**: Globally unique, no collision across windows/workspaces. Use `uuid` crate with `v4` feature
- **Grapheme cluster cell model** (`.claude/rules/terminal-vte.md`): Cell stores `String` (not `char`) for multi-codepoint graphemes. Wide chars span 2 cells (first has content, second is `WIDE_SPACER`)
- **bitflags for CellFlags and TerminalMode**: Efficient bitwise operations, derive Serialize/Deserialize for persistence
- **Color::Named uses indices 0-15**: Maps to theme palette. Color::Indexed for 256-color. Color::Rgb for truecolor

### Patterns to Follow
- Architecture §5 wmux-core: "Pure Rust library. Consumed by wmux-render, wmux-ui, wmux-ipc, wmux-app"
- All types derive `Debug, Clone, PartialEq` minimum
- ID types derive `Eq, Hash, Copy` for use as HashMap keys
- Serialize/Deserialize on types needed for session persistence (Cell, Color, CursorState, IDs)

### Technical Notes
- Cell default: space character, white foreground, default background, no flags
- TerminalMode defaults: wraparound ON, all others OFF
- WIDE_SPACER flag on Cell marks the trailing cell of a wide character
- Named colors 0-7 are normal, 8-15 are bright variants

## Success Criteria

- [ ] All ID types are `Copy + Clone + Debug + Display + Eq + Hash + Serialize + Deserialize`
- [ ] Cell struct correctly represents a grapheme cluster with all terminal attributes
- [ ] CellFlags supports all standard terminal attributes (bold, italic, underline, etc.)
- [ ] TerminalMode covers all required modes (origin, wraparound, bracketed paste, app cursor, mouse)
- [ ] Color enum handles Named(0-15), Indexed(0-255), Rgb(u8,u8,u8)
- [ ] `cargo build --workspace` succeeds
- [ ] `cargo clippy --workspace` zero warnings

## Validation Steps

### Automated Checks
```bash
cargo build --workspace
cargo clippy --workspace -- -W clippy::all
cargo test -p wmux-core
cargo fmt --all -- --check
```

### Manual Verification
1. Write a unit test creating a Cell with various attributes and verify Display/Debug output
2. Write a unit test verifying ID generation produces unique values
3. Verify CellFlags bitwise operations (combine flags, test individual flags)

### Edge Cases to Test
- Cell with empty grapheme (should default to space)
- Cell with multi-byte UTF-8 grapheme (emoji, CJK)
- Color::Named boundary values (0 and 15)
- Color::Indexed boundary values (0 and 255)
- TerminalMode: set and clear individual bits without affecting others

## Dependencies

**Blocks**:
- Task L1_01: Terminal Cell Grid Data Structure
- Task L2_02: PaneTree Binary Split Layout Engine

## References
- **PRD**: §Modèle Conceptuel (5-level hierarchy)
- **Architecture**: §5 wmux-core (domain model), §6 Data Architecture (session schema uses these types)
- **ADR**: ADR-0009 (Session Persistence — types must be serializable)
