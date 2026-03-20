---
task_id: L3_06
title: "Implement Browser DOM Automation"
status: pending
priority: P1
estimated_hours: 3
wave: 3
prd_features: [F-04]
archi_sections: [ADR-0001, ADR-0006]
depends_on: [L3_05]
blocks: [L3_07]
---

# Task L3_06: Implement Browser DOM Automation

> **Phase**: Integration
> **Priority**: P1-High
> **Estimated effort**: 3 hours
> **Wave**: 3

## Context
AI agents need full DOM interaction: click, fill forms, type text, check/uncheck, scroll, take screenshots, and inspect the accessibility tree. This is what makes wmux's browser programmable. PRD §4 lists 8 categories of browser commands.

## Prerequisites
- [ ] Task L3_05: Browser Navigation and JavaScript Eval — provides JS eval for DOM operations

## Scope
### Deliverables
- DOM interaction: click, dblclick, hover, focus, check, uncheck, scroll-into-view
- Form input: type, fill, press, keydown, keyup, select
- Scroll: scroll page or element
- Inspection: snapshot (accessibility tree), screenshot (CapturePreview), get, is, find, highlight
- Frame support: target operations to specific iframe
- Console log capture: read browser console messages
- Error capture: read JavaScript errors

### Explicitly Out of Scope
- Dialog handling (alert, confirm, prompt) — warn user these block the browser
- Download management (complex, post-MVP)
- IPC handler wiring (Task L3_07)

## Implementation Details
### Files to Create/Modify
| Action | Path | Purpose |
|--------|------|---------|
| Modify | `wmux-browser/src/automation.rs` | Add DOM interaction, screenshot, snapshot methods |
| Modify | `wmux-browser/src/panel.rs` | Add console log tracking |

### Key Decisions
- **DOM operations via JS eval**: Most DOM operations (click, fill, type) are implemented by injecting JavaScript that performs the action. This avoids complex COM interop for each operation
- **Accessibility tree via JS**: `snapshot` command builds a simplified accessibility tree by traversing DOM with `TreeWalker` and ARIA attributes
- **Screenshot via COM**: WebView2's `CapturePreviewAsync` provides a PNG screenshot natively

### Patterns to Follow
- PRD §4: Full command table (Interaction DOM, Inspection categories)
- Architecture §5 wmux-browser: "click, fill, eval, screenshot"

### Technical Notes
- click(selector): `document.querySelector(selector).click()`
- fill(selector, value): find input → clear → set value → dispatch input/change events
- type(selector, text): simulate keydown/keyup events for each character
- press(key): dispatch KeyboardEvent for key name
- snapshot: traverse DOM, extract tag names, roles, accessible names, states. Return as JSON tree
- screenshot: `CapturePreviewAsync(stream)` → PNG bytes → base64 encode for IPC response
- get(selector, attribute): `element.getAttribute(name)` or computed properties
- is(selector, state): check `checked`, `disabled`, `visible`, `editable`
- find(selector): return all matching elements as simplified objects
- highlight(selector): inject temporary CSS outline around element
- Console capture: WebView2 `DevToolsProtocolEventReceived` for "Runtime.consoleAPICalled"
- Error capture: listen for `Runtime.exceptionThrown` DevTools protocol event

## Success Criteria
- [ ] click/dblclick/hover work on page elements
- [ ] fill correctly populates form inputs
- [ ] type simulates character-by-character input
- [ ] snapshot returns accessibility tree as JSON
- [ ] screenshot returns PNG image (base64 encoded)
- [ ] get retrieves element attributes
- [ ] is checks element states (checked, visible, etc.)
- [ ] find returns matching elements
- [ ] highlight visually outlines elements
- [ ] Console and error capture works
- [ ] Frame targeting works for iframes
- [ ] `cargo clippy --workspace` zero warnings

## Validation Steps
### Automated Checks
```bash
cargo build --workspace
cargo clippy --workspace -- -W clippy::all
cargo test -p wmux-browser
cargo fmt --all -- --check
```
### Manual Verification
1. Navigate to a form page → fill input → verify value set
2. Click a button → verify action triggered
3. Call snapshot → verify tree structure returned
4. Call screenshot → verify PNG data returned
5. Trigger console.log → read console → verify message captured
### Edge Cases to Test
- Click on element that doesn't exist (should return error)
- Fill non-input element (should return error)
- Screenshot of blank page (should return valid PNG)
- Very large accessibility tree (limit depth to prevent timeout)
- Selector matching multiple elements (click first one)

## Dependencies
**Blocks**:
- Task L3_07: Browser IPC Handlers

## References
- **PRD**: §4 Navigateur Intégré (Interaction DOM, Inspection categories)
- **Architecture**: §5 wmux-browser (automation.rs)
- **ADR**: ADR-0006 (WebView2 automation)
