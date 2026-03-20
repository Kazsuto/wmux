---
task_id: L3_13
title: "Implement Shell Integration Hooks"
status: pending
priority: P1
estimated_hours: 2
wave: 3
prd_features: [F-13]
archi_sections: [ADR-0001]
depends_on: [L1_06]
blocks: [L4_07]
---

# Task L3_13: Implement Shell Integration Hooks

> **Phase**: Integration
> **Priority**: P1-High
> **Estimated effort**: 2 hours
> **Wave**: 3

## Context
Shell integration hooks are scripts sourced in the user's shell that emit OSC escape sequences for CWD tracking (OSC 7), prompt marks (OSC 133), and notifications. These enable the sidebar to show git branch, CWD, and prompt state. PRD §13 describes shell integration. Architecture §12 shows resources/shell-integration/.

## Prerequisites
- [ ] Task L1_06: PTY Async I/O — provides PTY environment variable injection

## Scope
### Deliverables
- PowerShell hook script (prompt function emitting OSC 7 + OSC 133)
- Bash hook script (PROMPT_COMMAND emitting OSC 7 + OSC 133)
- Zsh hook script (precmd/preexec emitting OSC 7 + OSC 133)
- Auto-injection: detect shell type, source appropriate hook on PTY spawn
- Hook scripts stored in `resources/shell-integration/`
- PowerShell execution policy handling (bypass for hook injection)

### Explicitly Out of Scope
- Fish shell (deferred per architecture audit)
- WSL shell hooks (deferred per architecture audit)
- Git detection from CWD (Task L3_14)

## Implementation Details
### Files to Create/Modify
| Action | Path | Purpose |
|--------|------|---------|
| Create | `resources/shell-integration/wmux.ps1` | PowerShell hook |
| Create | `resources/shell-integration/wmux.bash` | Bash hook |
| Create | `resources/shell-integration/wmux.zsh` | Zsh hook |
| Modify | `wmux-pty/src/manager.rs` | Auto-inject hook on shell spawn |

### Key Decisions
- **Auto-injection**: Detect shell type from executable name. For PowerShell: set `-Command` arg to source the hook. For bash: set `--rcfile` or `BASH_ENV`. For zsh: set `ZDOTDIR` to temp dir with hook-sourcing .zshrc
- **PowerShell execution policy**: Use `-ExecutionPolicy Bypass` for the hook injection command only
- **Minimal hooks**: Only emit OSC sequences. Don't modify prompt appearance or behavior beyond adding sequences

### Patterns to Follow
- Architecture §12: resources/shell-integration/ directory
- `.claude/rules/terminal-vte.md`: "OSC 7 (CWD), OSC 133 (prompt marks)"

### Technical Notes
- PowerShell hook: override `prompt` function to emit `\x1b]7;file://hostname/path\x07` before normal prompt, and OSC 133;A before prompt, 133;B after
- Bash hook: set PROMPT_COMMAND to emit OSC 7 with `\e]7;file://$(hostname)/$(pwd)\a` and OSC 133 marks
- Zsh hook: use `precmd` for OSC 7 + 133;A/133;D, `preexec` for 133;B/133;C
- Shell detection in PtyManager: check executable name (pwsh/powershell → PowerShell, bash → Bash, zsh → Zsh)
- Hook injection must not break if user's own profile has errors
- Bundled as string resources in the binary (or embedded via `include_str!`)

## Success Criteria
- [ ] PowerShell hook emits OSC 7 on directory change
- [ ] Bash hook emits OSC 7 on directory change
- [ ] Zsh hook emits OSC 7 on directory change
- [ ] OSC 133 prompt marks emitted correctly
- [ ] Hook auto-injected on shell spawn
- [ ] Shell still works normally with hook (no prompt breakage)
- [ ] PowerShell execution policy doesn't block hook
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
1. Open wmux terminal → `cd /tmp` → verify sidebar CWD updates
2. Verify prompt appears normal (no extra characters)
3. Test in bash (Git Bash) → verify OSC 7 works
### Edge Cases to Test
- Shell with custom prompt (should not break)
- Shell without hook support (should fall back gracefully)
- Very deep directory path (OSC 7 should handle long paths)
- Directory with spaces in name (should be properly encoded in URI)

## Dependencies
**Blocks**:
- Task L3_14: Git Branch Detection

## References
- **PRD**: §13 Shell Integration & Détection Git
- **Architecture**: §12 Project Structure (shell-integration/)
