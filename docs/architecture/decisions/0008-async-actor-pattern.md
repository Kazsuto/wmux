# ADR-0008: Async Architecture — Actor Pattern via Bounded Channels

> **Status**: Accepted
> **Date**: 2026-03-19
> **Confidence**: High
> **Deciders**: wmux team

## Context

wmux has multiple concurrent subsystems that must coordinate safely: the GPU render loop (winit event loop, main thread), PTY I/O readers (one per terminal pane, blocking), the IPC server (async, handles multiple concurrent clients), session persistence (periodic writes), and notification processing. These subsystems need to read and mutate shared application state (workspace tree, terminal grids, sidebar metadata). The concurrency model directly affects correctness (data races), performance (contention), and code complexity.

## Decision Drivers

- Terminal multiplexer state is inherently mutable — workspaces, panes, and grids change constantly
- Multiple producers (IPC commands, PTY output, keyboard input) modify the same state
- The render loop must read state every frame (60fps) without blocking on locks
- tokio runtime is already required for async IPC and PTY I/O
- Deadlocks are notoriously hard to debug in GUI applications

## Decision

**Actor pattern via bounded tokio channels**. Each stateful subsystem runs as a dedicated tokio task that owns its state exclusively. Communication happens through bounded `mpsc` channels (command → actor) and `oneshot` channels (actor → caller for request/response). No `Arc<Mutex<T>>` for shared mutable state.

Key actors:
- **AppState actor**: Owns workspace tree, pane registry, focus state. Receives commands from IPC and UI
- **PTY actors** (one per pane): Own PTY handle and read loop. Forward output to terminal grid
- **IPC Server actor**: Owns Named Pipe listener. Dispatches commands to AppState actor
- **Persistence actor**: Periodically snapshots state from AppState actor

Channel sizing: Bounded at 256 messages. Backpressure if a consumer falls behind (IPC commands queue up rather than being dropped).

## Alternatives Considered

### Arc<Mutex<T>> shared state
- **Pros**: Simple, familiar Rust pattern. No channel overhead. Direct access to state
- **Cons**: Lock contention between render loop (reads every 16ms) and IPC/PTY writers. Deadlock risk when multiple locks are held. Mutex poisoning on panic leaves state inconsistent. Hard to reason about lock ordering across 5+ subsystems
- **Why rejected**: The render loop reads state at 60fps — a contended Mutex would cause frame drops. Lock ordering bugs in a multiplexer with N panes are inevitable. Actors eliminate this entire class of bugs by design

### tokio::sync::broadcast for event bus
- **Pros**: Pub/sub model, decoupled producers and consumers. Good for fan-out
- **Cons**: Broadcast channels clone every message to every subscriber — expensive for large state updates (terminal grid rows). Lagging receivers get `RecvError::Lagged` and lose messages. No request/response pattern
- **Why rejected**: Terminal grid updates are hot-path data (thousands of cells/frame). Broadcast cloning is too expensive. The command/response pattern (IPC → execute → return result) doesn't fit pub/sub

### Single-threaded with polling (event loop only)
- **Pros**: No concurrency bugs at all. Simple mental model. Works for simple terminals (Alacritty)
- **Cons**: Cannot do blocking ConPTY reads on the main thread (would freeze the UI). IPC server needs async accept/read. Session persistence writes block. Everything serialized through one thread limits throughput
- **Why rejected**: ConPTY reads are blocking — must be on separate threads. IPC server must handle concurrent clients. A single-threaded model cannot support N terminal panes + IPC without introducing manual polling complexity worse than actors

## Consequences

### Positive
- No data races or deadlocks by construction — state is never shared, only messages
- Render loop never blocks — reads state snapshots sent via channels
- IPC commands have natural backpressure (bounded channel fills up → client waits)
- Each actor can be tested in isolation by injecting messages
- Clean shutdown via channel close semantics (all senders drop → receiver loop ends)

### Negative (acknowledged trade-offs)
- Channel overhead: ~50ns per send/recv (negligible for IPC command frequency, but adds up for high-frequency PTY output)
- More boilerplate: each actor needs a command enum, message types, and dispatch loop
- Debugging: actor message flow is harder to trace than direct function calls (mitigated by `tracing` spans)
- State snapshots for rendering may be slightly stale (one frame behind) — acceptable for terminal UI

### Mandatory impact dimensions
- **Security**: Actors isolate state — an IPC command cannot corrupt terminal grid state because it goes through the actor's command handler. Input validation happens at the actor boundary
- **Cost**: $0. tokio channels are part of tokio's standard library. No additional dependencies
- **Latency**: Channel send/recv adds ~50ns. For IPC commands (< 1000/s), this is negligible. For PTY output bytes, batching into row-sized chunks amortizes the overhead

## Revisit Triggers

- If channel overhead for PTY output exceeds 1ms per frame (measurable with tracing spans), investigate lock-free ring buffers for the terminal grid specifically (keep actors for everything else)
- If the actor boilerplate becomes excessive (> 10 actors), evaluate a lightweight actor framework crate (e.g., `kameo`, `xactor`)
- If debugging actor message flows becomes too difficult, add structured tracing for all channel sends with correlation IDs
