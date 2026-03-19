# ADR-0010: Config Format — Ghostty-compatible Key-Value

> **Status**: Accepted
> **Date**: 2026-03-19
> **Confidence**: Medium
> **Deciders**: wmux team

## Context

wmux needs a configuration format for terminal appearance (colors, fonts, opacity), keybindings, application behavior (scrollback size, shell path, locale), and theme selection. cmux uses Ghostty's configuration format, which is a simple `key = value` text format (not standard TOML despite visual similarity). Reusing this format would allow wmux to import existing Ghostty theme files (~50+ themes available).

## Decision Drivers

- cmux users already have Ghostty config files — reusable themes reduce migration friction
- ~50+ Ghostty themes available as community resources (catppuccin, dracula, nord, etc.)
- Configuration must be human-editable with a text editor
- Must support comments for documentation
- Must be parseable without external tools
- Simplicity — terminal users expect simple `key = value`, not nested YAML/JSON

## Decision

**Ghostty-compatible `key = value` format** for wmux configuration, stored at `%APPDATA%\wmux\config`. A custom parser layer handles Ghostty's format quirks on top of the `toml 0.8` crate for value parsing.

Configuration priority (highest to lowest):
1. `%APPDATA%\wmux\config` (wmux-specific)
2. `%APPDATA%\ghostty\config` (imported from existing Ghostty install)
3. Built-in defaults

Format example:
```
# wmux configuration
theme = catppuccin-mocha
font-family = JetBrains Mono
font-size = 14
scrollback-limit = 4000
shell = pwsh
locale = auto
```

## Alternatives Considered

### Standard TOML with sections
- **Pros**: Well-defined spec (toml.io). Native Rust support (toml crate). Sections for organization (`[terminal]`, `[keybindings]`, `[theme]`). Strong typing
- **Cons**: Not compatible with Ghostty theme files (Ghostty uses flat `key = value` without sections). Users would need to convert existing Ghostty configs. More complex for simple terminal configuration
- **Why rejected**: Ghostty compatibility is a key differentiator — importing 50+ existing themes requires matching the format. Standard TOML sections add complexity without benefit for a flat config. If wmux needs sections in the future, the parser can be extended while keeping backward compatibility

### JSON
- **Pros**: Universal format. Easy to parse (serde_json). Machine-readable. Schema validation possible
- **Cons**: Not human-friendly for editing (requires quotes, commas, braces). No comments. Not convention for terminal configs. Hostile to version control diffs
- **Why rejected**: Terminal configuration files are edited by hand in text editors. JSON's syntax noise (braces, quotes, commas) and lack of comments make it a poor choice. Every terminal emulator (Alacritty YAML->TOML, WezTerm Lua, Ghostty key=value, Kitty key=value) avoids JSON

### YAML
- **Pros**: Human-readable. Comments supported. Nested structure. Used by Alacritty (historically)
- **Cons**: Whitespace-sensitive (indentation errors cause silent bugs). Complex spec with many gotchas (`yes`/`no` as booleans, Norway problem). Alacritty itself migrated away from YAML to TOML. Requires `serde_yaml` dependency
- **Why rejected**: YAML's gotchas are well-documented — even Alacritty abandoned it. The terminal community has moved to simpler formats. YAML adds a dependency (`serde_yaml`) and complexity for no benefit over `key = value`

### Lua (like WezTerm)
- **Pros**: Full programming language for config. Conditional logic, functions, computed values. WezTerm proves it works for terminal config
- **Cons**: Requires embedding a Lua runtime (mlua/rlua — adds ~500KB). Overkill for simple key=value config. Security concern (config file executes arbitrary code). Learning curve for non-programmers
- **Why rejected**: wmux configuration is declarative (set values, pick themes) — no need for conditional logic or computation. Embedding Lua adds binary size, security surface area, and complexity. If programmatic config is needed in v2, Lua can be added as an optional layer

## Consequences

### Positive
- Direct import of ~50+ Ghostty community themes (catppuccin, dracula, nord, gruvbox, etc.)
- Familiar format for cmux users migrating to Windows
- Simple to edit — no syntax to learn beyond `key = value` and `#` comments
- No external dependency beyond `toml 0.8` (already used for locale files)

### Negative (acknowledged trade-offs)
- Custom parser needed — Ghostty's format is not standard TOML (no sections, no inline tables). Parser must handle the differences
- No nested configuration — flat key space may require prefixed keys (`keybind-ctrl-n = new-workspace`) instead of sections
- Less tooling support than standard TOML (no schema validation, no editor auto-complete)
- Format is Ghostty-specific — if Ghostty changes its format, wmux must track

### Mandatory impact dimensions
- **Security**: Config files are read-only (parsed, not executed). No code execution from config. File permissions: user-readable only. Config paths are canonicalized to prevent directory traversal
- **Cost**: $0. toml 0.8 is already a dependency. Custom parser is ~200 lines
- **Latency**: Config parsing at startup: < 1ms for typical config file. Theme loading: < 5ms (read theme file + build color palette). Hot-reload on file change: < 10ms

## Revisit Triggers

- If Ghostty's config format changes significantly (adds sections, changes syntax), evaluate whether to track the changes or fork the format
- If users request complex configuration (conditional themes, per-workspace settings), evaluate adding a TOML override layer (`config.toml`) alongside the Ghostty-compat base
- If the flat key space becomes unwieldy (> 100 config keys), introduce optional TOML sections while keeping backward compatibility
