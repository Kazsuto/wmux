# Task L2_01: Refactor to AppState Actor and Multi-Pane Architecture

> **Phase**: Core
> **Priority**: P0-Critical
> **Estimated effort**: 2.5 hours

## Context
The single-pane terminal (Task L1_10) has a direct coupling between the event loop and terminal state. To support multiple panes, workspaces, and IPC, the application needs an actor-based architecture where AppState owns all mutable state and communicates via bounded channels. ADR-0008 mandates the actor pattern. Architecture §5 (wmux-ipc) describes the AppState actor.

## Prerequisites
- [ ] Task L1_10: Single-Pane Terminal Integration — working single-pane terminal to refactor

## Scope
### Deliverables
- `AppState` actor struct (owns workspace tree, pane registry, metadata store)
- `AppCommand` enum (all operations that mutate state)
- `AppResponse` enum (results returned to callers)
- Bounded channel pair for command/response communication
- Bridge: winit event loop sends commands, receives responses
- PaneRegistry: HashMap<PaneId, PaneState> tracking all active panes
- Each pane owns: Terminal + PtyActor channels + TerminalRenderer state
- tokio task running AppState actor loop

### Explicitly Out of Scope
- PaneTree layout engine (Task L2_02)
- IPC server integration (Task L2_09)
- Workspace model (Task L2_07)

## Implementation Details
### Files to Create/Modify
| Action | Path | Purpose |
|--------|------|---------|
| Create | `wmux-core/src/app_state.rs` | AppState actor, AppCommand, AppResponse |
| Create | `wmux-core/src/pane_registry.rs` | PaneRegistry (HashMap<PaneId, PaneState>) |
| Modify | `wmux-ui/src/app.rs` | Refactor App to use AppState via channels |
| Modify | `wmux-app/src/main.rs` | Spawn AppState actor task |
| Modify | `wmux-core/src/lib.rs` | Export new modules |

### Key Decisions
- **Actor pattern via bounded channels** (ADR-0008): AppState runs in a dedicated tokio task. All mutations go through `AppCommand` messages. No `Arc<Mutex>` on any state
- **Channel capacity 256** (ADR-0008): Provides backpressure without excessive memory use
- **PaneState bundles per-pane data**: Terminal instance, PtyActor channel handles, dirty state, viewport offset, selection state

### Patterns to Follow
- ADR-0008: "AppState actor owns workspace tree/pane registry"
- `.claude/rules/rust-architecture.md`: Actor pattern (channel + task) over Arc<Mutex<T>>
- Architecture §5 wmux-ipc: AppState actor with bounded channel

### Technical Notes
- AppCommand variants: CreatePane, ClosePane, ResizePane, FocusPane, SendInput, ProcessPtyOutput, etc.
- Render path: App (winit) requests render data from AppState. AppState returns dirty pane info + grid snapshots. This avoids GPU state in the actor
- EventLoopProxy wakes winit when AppState has render-relevant changes
- For the initial refactor, keep single-pane behavior — just route through actor. Multi-pane logic comes in Task L2_02

## Success Criteria
- [ ] Application still works as a single-pane terminal after refactoring
- [ ] All terminal state is owned by AppState actor (no direct state access from event loop)
- [ ] Commands flow through bounded channels
- [ ] No `Arc<Mutex>` usage on terminal/pane state
- [ ] Performance regression < 1ms per frame compared to pre-refactor
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
1. Run app — verify terminal still works identically to Task L1_10 output
2. Type commands, verify output, scrollback, copy/paste all still functional
3. Check tracing logs for actor message flow
### Edge Cases to Test
- Rapid input while PTY is producing output (concurrent messages)
- Window resize during heavy output (command + resize race)
- AppState channel full (verify backpressure, not panic)

## Dependencies
**Blocks**:
- Task L2_02: PaneTree Binary Split Layout Engine
- Task L2_09: Named Pipes Server + JSON-RPC v2

## References
- **PRD**: §2 Multiplexeur (multi-pane architecture)
- **Architecture**: §5 wmux-ipc (AppState actor), §4 Component Diagram
- **ADR**: ADR-0008 (Actor pattern via bounded channels)
