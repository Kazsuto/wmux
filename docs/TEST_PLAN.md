# wmux — Test Plan

Essential tests only — high ROI, hard to debug visually, breaks silently.

## Status Legend

- [ ] Not implemented
- [x] Implemented

---

## Tier 0 — Existing Code (4 tests)

> Target: `wmux-render/src/quad.rs`, `wmux-core/src/error.rs`

- [ ] `push_quad_skips_non_finite` — NaN/Infinity on x/y/w/h silently skipped
- [ ] `push_quad_respects_max_capacity` — >4096 quads silently dropped, no panic
- [ ] `clear_resets_count` — `clear()` → `quad_count() == 0`
- [ ] `core_error_messages` — All CoreError variants produce readable messages

## Tier 1 — Layer 1: Single-Pane Terminal (18 tests)

### Grid (5 tests) — `wmux-core/src/grid.rs`

- [ ] `grid_new_dimensions` — Grid(80,24) → 24 rows × 80 cols of Cell::default()
- [ ] `grid_resize` — Grow and shrink preserve existing data, fill with default
- [ ] `grid_scroll_up` — Rows shift up within scroll region, new row at bottom
- [ ] `grid_scroll_down` — Rows shift down within scroll region, new row at top
- [ ] `grid_out_of_bounds_no_panic` — Invalid coords → CoreError::OutOfBounds, never panic

### VTE Parser (8 tests) — `wmux-core/src/vte_handler.rs`

- [ ] `vte_print_ascii` — "Hello" → 5 cells with correct graphemes
- [ ] `vte_print_cjk_wide` — "世" → wide cell + WIDE_SPACER
- [ ] `vte_cursor_absolute` — ESC[5;10H → cursor at (4,9) (1-based to 0-based)
- [ ] `vte_erase_line` — ESC[0K / ESC[1K / ESC[2K clear correct regions
- [ ] `vte_sgr_color_rgb` — ESC[38;2;255;128;0m → fg = Rgb(255,128,0)
- [ ] `vte_sgr_reset` — ESC[0m → default attributes restored
- [ ] `vte_wraparound` — Write past right margin → wraps to next line
- [ ] `vte_malformed_no_panic` — Invalid escape sequences silently discarded

### Scrollback (3 tests) — `wmux-core/src/scrollback.rs`

- [ ] `scrollback_push_and_read` — Push rows → read_text() returns content
- [ ] `scrollback_capacity_eviction` — >4096 lines → oldest evicted (FIFO)
- [ ] `scrollback_alternate_screen_swap` — Enter alt screen → main grid preserved

### PTY (2 tests) — `wmux-pty/src/manager.rs` — `#[ignore]`

- [ ] `pty_spawn_and_echo` — Spawn shell, write "echo hello\n", read "hello" back
- [ ] `shell_detection_order` — Detection logic: pwsh → powershell → cmd

## Tier 2 — Layer 2: Multiplexer + IPC (10 tests)

### PaneTree Layout (3 tests) — `wmux-ui/src/pane_tree.rs`

- [ ] `split_equal` — Horizontal/vertical split → two panes at 50% each
- [ ] `remove_pane_reparent` — Remove pane → sibling takes 100%
- [ ] `directional_navigation` — Focus left/right/up/down targets correct pane

### IPC Protocol (5 tests) — `wmux-ipc/src/protocol.rs`

- [ ] `jsonrpc_parse_valid` — Valid JSON-RPC 2.0 request parsed correctly
- [ ] `jsonrpc_invalid_version_error` — jsonrpc != "2.0" → error -32600
- [ ] `jsonrpc_newline_delimited` — Messages separated by \n
- [ ] `hmac_auth_valid` — Correct HMAC token → request accepted
- [ ] `hmac_auth_invalid` — Wrong token → rejected

### IPC Handlers (2 tests) — `wmux-ipc/src/handlers/`

- [ ] `method_names_match_cmux` — Exact names: workspace.list, surface.send_text, etc.
- [ ] `workspace_create_and_list` — Create workspace via IPC → appears in list

## Tier 3 — Layer 3: Integration (3 tests)

### Session Persistence — `wmux-core/src/session.rs`

- [ ] `session_serialize_roundtrip` — Save → Load → identical state
- [ ] `session_restore_corrupt_no_crash` — Invalid JSON → clean start + warning

### Config Parser — `wmux-config/src/parser.rs`

- [ ] `config_parse_key_value` — `font-size = 16` parsed correctly, comments ignored
