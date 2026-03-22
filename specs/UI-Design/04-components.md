---
name: wmux Component Direction — "Luminous Void"
step: 04-components
date: 2026-03-22
---

# Component Direction: "Luminous Void"

Visual treatments for every wmux component, referencing tokens from `03-visual-system.md`.
All descriptions are visual — no implementation code.

---

## 1. Navigation Components

### 1a. Sidebar (Workspace Navigation)

**Role:** Primary navigation — workspace list, session management, quick-switch.

**Structure (top to bottom):**
- App logo/icon (16×16px) + "wmux" label (`text-sm`, `weight-semibold`, `font-ui`) — top area, `space-3` padding
- Section header: "WORKSPACES" (`text-xs`, `weight-medium`, `font-ui`, `tracking-wide`, `text-muted`) — uppercase, 12px minimum for functional text
- Workspace rows (flat list, not cards — compact density, high volume)
- Bottom: collapse toggle icon

**Workspace Row (48px height):**
- Left: accent stripe (`focus-stripe-width` 3px, `radius-xs`) — visible only on active workspace
- Icon area: workspace icon or colored dot (8px circle, workspace-assigned color from ANSI palette) — `space-3` from left
- Primary text: workspace name (`text-base`, `weight-medium`, `font-ui`, `text-primary`) — single line, truncate with ellipsis
- Secondary text: pane count ("3 panes") (`text-sm`, `weight-regular`, `font-ui`, `text-secondary`) — below name
- Right: notification badge (if pending) — circular (`radius-full`, 16px diameter, `accent` bg, `text-inverse` text, `text-2xs`)
- Padding: `space-3` horizontal, `space-1` vertical

**Visual Treatment:**
- Background: `surface-1` (not surface-base — sidebar is elevated)
- Right border: 1px `border-glow` (accent 25% alpha — luminous accent separator, signature futuristic-ai element)
- Active row: `surface-2` background + left accent stripe (`accent`) + `glow-subtle`
- Hover row: `surface-2` at 50% alpha, `motion-micro` (80ms) transition
- Drag reorder: row lifts with `shadow-md`, 3% scale increase, `motion-fast` (150ms)

### 1b. Tab Bar (Pane Tabs)

**Role:** Switch between panes within a workspace. Pill-style tabs in a horizontal bar.

**Bar:** 36px height, `surface-1` background, `shadow-sm` bottom separation. Full width of content area.

**Tab Pill:**
- Height: 28px (within 36px bar, centered vertically with `space-1` top/bottom)
- Padding: `space-1.5` vertical, `space-3` horizontal
- Radius: `radius-md` (6px)
- Gap between pills: `tab-gap` (4px)
- Text: pane title (`text-sm`, `weight-medium`, `font-ui`, `text-secondary` for inactive)
- Max width: 160px, truncate with ellipsis

**Tab States:**
- Default: `surface-1` background (matches bar), `text-secondary`
- Active: `surface-2` background, `text-primary`, bottom accent indicator (2px rounded bar, `accent`, `radius-xs`)
- Hover (inactive): `surface-2` at 50% alpha, `motion-micro`
- Close button: appears on hover, 16px × 16px, `text-muted` → `text-secondary` on hover, `radius-sm`
- Unsaved indicator: small dot (6px, `warning`) left of title

**Tab Type Indicators (resolves audit issue #4):**
- Terminal pane: `>_` icon prefix (8px, `text-muted`, `font-mono`)
- Browser pane: globe icon prefix (8px, `text-muted`)
- Split pane: split-view icon prefix (8px, `text-muted`)

### 1c. Command Palette

**Role:** Universal command search. Primary interaction pattern (keyboard-first).

**Overlay:**
- Backdrop: `overlay-dim` (50% black) + `overlay-tint` (8% accent) layered on top
- Centered horizontally, positioned at 20% from top of viewport

**Panel:**
- Width: `command-palette-width` (600px)
- Radius: `radius-xl` (12px)
- Background: `surface-overlay` (surface-base + 8L at 95% alpha)
- Shadow: `shadow-xl`
- Border: 1px `border-subtle` (40% alpha)
- Max height: 480px (20 results × 36px + input + padding)

**Search Input:**
- Height: `command-input-height` (44px)
- Background: `surface-0`
- Radius: `radius-lg` (8px)
- Text: `text-xl`, `weight-regular`, `font-ui`, `text-primary`
- Placeholder: `text-muted`, "Type a command..."
- Left icon: search magnifier (16px, `text-muted`)
- Padding: `space-3` horizontal
- Bottom border: 1px `border-subtle` separating from results

**Result Item:**
- Height: `command-result-height` (36px)
- Padding: `space-3` horizontal, `space-1` vertical
- Default: transparent background
- Hover/selected: `surface-2`, `radius-md` (6px)
- Left: command icon (16px, `text-secondary`)
- Center: command name (`text-base`, `weight-regular`, `font-ui`, `text-primary`)
- Right: keyboard shortcut (`text-xs`, `weight-regular`, `font-mono`, `text-muted`, `tracking-wide`)

**Animation:**
- Open: backdrop fades in (`motion-fast`), panel scales 0.97→1.0 + fades in (`motion-slow`, `ease-out`)
- Close: reverse, `motion-fast` for both
- Result filtering: instant (no animation on list changes)

---

## 2. Surface Components

### 2a. Terminal Pane (Core Content)

**Role:** The primary content area. Terminal grid rendering.

**Visual Treatment:**
- Background: `surface-base` (matches theme background exactly)
- No padding (terminal content fills the entire pane area)
- Content: rendered via glyphon text atlas with ANSI 16 colors
- Cursor: theme cursor color at 85% alpha, blinks at `motion-blink` (500ms)
- Selection: theme selection color (`#264f78` for default theme)

**Focus State (THE Signature Element):**
- Active pane: `glow-focus` — 1px `accent-glow-core` inner ring + 16px `accent-glow` outer halo
- Glow extends outward from pane edges, overlapping into the gap between panes
- Animation: see Motion System Focus Glow specification (250ms ease-out in, 150ms ease-in out, 50ms overlap)
- Inactive panes: no glow, dimming overlay (surface-base at `1 - inactive_pane_opacity` alpha) on top — text remains readable

**Pane Dividers:**
- Width: `pane-divider-width` (1px)
- Color: `border-glow` (accent at 12% alpha) — luminous rather than dead gray
- Drag handle on hover: divider area expands to 5px, color shifts to `border-default`, cursor changes to resize

### 2b. Status Bar (NEW)

**Role:** Persistent bottom information bar — workspace context, connection status, mode indicators.

**Dimensions:**
- Height: `status-bar-height` (28px)
- Full width of window
- Shadow: `shadow-sm` (inverted: 0 -1px 3px, upward)

**Visual Treatment:**
- Background: `surface-1`
- Border top: 1px `border-subtle`
- Text: `text-2xs` (10px), `weight-regular`, `font-ui`, `text-secondary`
- Values: `text-2xs`, `weight-regular`, `font-mono`, `text-primary` (monospace for data values)
- Sections separated by `·` dot divider (`text-muted`)

**Sections (left to right):**
- Left group: workspace name (clickable, `text-primary`) · pane count · connection status icon (colored dot: `success` = connected, `warning` = reconnecting, `error` = disconnected)
- Center: (reserved for mode/status messages)
- Right group: branch name (if git detected) · encoding · shell type · timestamp
- All status bar text uses `text-xs` (12px) minimum — NOT text-2xs (Guardian: functional text must be 12px+)

**Hover:** section highlights with `surface-2` at 50% alpha on hover, some sections clickable (workspace name opens command palette)

### 2c. Notification Panel

**Role:** Right-side sliding panel for notifications, alerts, background process updates.

**Panel:**
- Width: `notification-panel-width` (360px)
- Radius: `radius-lg` (8px) on left corners only (slides from right edge)
- Background: `surface-overlay` (95% alpha)
- Shadow: `shadow-lg`
- Border left: 1px `border-subtle`

**Header:**
- "Notifications" title (`text-lg`, `weight-semibold`, `font-ui`, `text-primary`)
- Clear all button (ghost, `text-sm`, `text-secondary`)
- Close button (icon-only, 16px, `text-muted`)
- Padding: `space-3`
- Border bottom: 1px `border-subtle`

**Notification Item (72px height):**
- Left accent stripe: 2px, colored by severity (`error`/`warning`/`success`/`info`)
- Background tint: severity-muted color (12% alpha) — resolves audit issue #7
- Content: title (`text-base`, `weight-medium`, `font-ui`) + message (`text-sm`, `weight-regular`, `font-ui`, `text-secondary`) + timestamp (`text-2xs`, `text-muted`)
- Padding: `space-2` all sides
- Bottom separator: 1px `border-subtle`
- Hover: `surface-2`, `radius-md`, `motion-micro`
- Dismiss: swipe right or click X → slide out `motion-normal`, `ease-in`

**Animation:** Panel slides from right (`motion-slow`, `ease-out`). Individual notifications can fade in from top.

---

## 3. Button System

wmux has minimal button needs (keyboard-first). Buttons appear in command palette, notifications, status bar, and dialogs.

### Primary Button
- Background: `accent`
- Text: `text-inverse`, `text-sm`, `weight-medium`, `font-ui`
- Padding: `space-1.5` vertical, `space-3` horizontal
- Radius: `radius-md` (6px)
- Height: 32px
- Hover: `accent-hover` background, `motion-micro`
- Press: slight scale down (0.97), `motion-micro`
- Focus: 2px `accent-glow-core` ring, 2px offset

### Secondary Button
- Background: `surface-2`
- Border: 1px `border-default`
- Text: `text-primary`, `text-sm`, `weight-medium`, `font-ui`
- Same dimensions as primary
- Hover: `surface-3` background, `motion-micro`

### Ghost Button
- Background: transparent
- Text: `text-secondary`, `text-sm`, `weight-regular`, `font-ui`
- Same dimensions, no border
- Hover: `surface-2` at 50% alpha, `motion-micro`

### Destructive Button
- Background: `error`
- Text: `text-inverse`, `text-sm`, `weight-medium`, `font-ui`
- Same dimensions as primary
- Hover: `error` + 8L (lighter red)

### Icon-Only Button
- Size: 28px × 28px
- Background: transparent
- Icon: 16px, `text-secondary`
- Radius: `radius-sm` (4px)
- Hover: `surface-2`, icon → `text-primary`, `motion-micro`

---

## 4. Input Components

### Search Input (Command Palette)
- Covered in Command Palette section above

### Config/Settings Input (future dialogs)
- Height: 32px (compact density)
- Background: `surface-0`
- Border: 1px `border-default`
- Radius: `radius-md` (6px)
- Text: `text-sm`, `weight-regular`, `font-ui`, `text-primary`
- Placeholder: `text-muted`
- Focus: border → `accent`, 2px outer ring `accent-glow`
- Error: border → `error`, helper text below in `error` color, `text-xs`
- Padding: `space-3` horizontal, `space-1.5` vertical

---

## 5. Feedback Components

### Tooltips
- Background: `surface-3`
- Text: `text-xs`, `weight-regular`, `font-ui`, `text-primary`
- Radius: `radius-sm` (4px)
- Shadow: `shadow-md`
- Padding: `space-1` vertical, `space-2` horizontal
- Arrow: 4px triangle matching background
- Delay: 400ms before showing, `motion-fast` fade-in
- Max width: 240px

### Toast Notifications (Inline)
- Positioned bottom-right, above status bar
- Width: 320px
- Radius: `radius-lg` (8px)
- Shadow: `shadow-lg`
- Background: `surface-overlay` with severity-muted tint
- Left accent stripe: 3px, severity color
- Text: `text-sm`, `font-ui`
- Auto-dismiss: 4 seconds (error: manual dismiss only)
- Enter: slide up from bottom + fade in, `motion-slow`, `ease-out`
- Exit: fade out + slide down, `motion-normal`, `ease-in`

### Keyboard Shortcut Hints
- Background: `surface-2`
- Text: `text-xs`, `weight-regular`, `font-mono`, `text-muted`, `tracking-wide`
- Radius: `radius-sm` (4px)
- Padding: `space-0.5` vertical, `space-1` horizontal
- Border: 1px `border-subtle`
- Example: `Ctrl+K` rendered as individual key caps

---

## 6. UI States

### Hover
- Surface elevation: current level → next level (e.g., `surface-1` → `surface-2` at 50% alpha)
- Timing: `motion-micro` (80ms), `ease-out`
- Text: `text-secondary` → `text-primary` (on interactive labels)
- Icon-only buttons: icon color shift + background appear
- Pane dividers: expand 1px → 5px, color shift to `border-default`
- Sidebar rows: background lifts, subtle reveal of action icons

### Focus (Keyboard Navigation)
- **Pane focus (signature):** `glow-focus` — 1px inner ring + 16px outer halo in accent
- **Element focus:** 2px `accent` outline, 2px offset — appears instantly on keyboard navigation
- **Command palette focus:** selected result gets `surface-2` background + left accent indicator (2px)
- **Tab focus:** tab pill gets 2px `accent` ring
- Focus indicators: visible ONLY on keyboard navigation (not mouse clicks)
- Contrast: accent ring has 3:1+ contrast ratio against all surface levels

### Loading States
- **Terminal pane loading:** cursor blinks with "connecting..." text in `text-muted`, centered
- **Command palette search:** subtle inline spinner (12px, `accent`, 800ms rotation, `ease-linear`)
- **Notification fetch:** skeleton pulse on notification items — `surface-2` → `surface-0` shimmer, 1.5s cycle
- **Sidebar workspace loading:** skeleton rows (48px, `surface-2` shimmer, 3 placeholder rows)
- **SSH connection pending:** status bar connection dot pulses `warning` color (motion-pulse 2s), pane shows "Connecting to {host}..." centered in `text-muted` with inline spinner
- **Command palette results loading:** 3 skeleton result rows (36px each, `surface-2` → `surface-0` shimmer)
- No full-screen loading spinners — wmux loads incrementally
- Skeleton shimmer: linear gradient sweep left-to-right, 1.5s cycle, `ease-in-out`

### Empty States
- **No workspaces:** centered in sidebar area
  - Icon: terminal icon outline (32px, `text-muted`)
  - Headline: "No workspaces" (`text-base`, `weight-medium`, `font-ui`, `text-primary`)
  - Description: "Create a workspace to get started" (`text-sm`, `font-ui`, `text-secondary`)
  - CTA: primary button "New Workspace" or keyboard hint `Ctrl+N`
  - Spacing: `space-4` between elements
- **No notifications:** centered in notification panel
  - Icon: bell-off icon (24px, `text-muted`)
  - Text: "All caught up" (`text-sm`, `font-ui`, `text-muted`)
- **Empty terminal:** blank `surface-base` with blinking cursor (normal terminal behavior, no special empty state)

### Error States
- **Connection error:** status bar dot turns `error`, tooltip shows error message
- **Command error:** toast notification with `error` severity (left stripe, red-muted background)
- **Terminal process crash:** pane shows error message in `error` color, centered, with "Restart" primary button
- **Input validation:** border → `error`, helper text below in `error`, `text-xs`
- All errors include: specific icon (warning triangle), descriptive text, actionable suggestion

### Success States
- **Command executed:** brief flash of `success-muted` background on the command palette result (200ms), then close
- **Workspace created:** toast notification with `success` severity, auto-dismiss 3s
- **Connection established:** status bar dot transitions to `success`, brief `glow-subtle` pulse (1 cycle)
- Feedback is always brief and non-blocking

### Disabled States
- Opacity: 40% overall (text + background + borders all at 40%)
- Cursor: not-allowed (or no cursor change in GPU renderer — element simply doesn't respond)
- No hover effects on disabled elements
- Text: `text-muted` (regardless of normal state)
- Interactive elements: `surface-1` background, no border emphasis
- Example: "Delete Workspace" button disabled when only one workspace exists

---

## 7. Special Components

### Pane Resize Handle
- Default: invisible (1px `border-glow`)
- Hover: expands to 5px, `border-default` color, resize cursor
- Active (dragging): `accent` color, 3px width
- Orientation: horizontal or vertical depending on split direction

### Window Title Bar (DWM Integration)
- Uses Windows DWM attributes: `DWMWA_USE_IMMERSIVE_DARK_MODE`, `DWMWA_CAPTION_COLOR` (matches `surface-1`), `DWMWA_TEXT_COLOR`, `DWMWA_BORDER_COLOR`
- Close/Minimize/Maximize: native Windows buttons (theme-adapted via DWM)
- No custom title bar — leverage native Windows 11 integration

### Notification Badge (Sidebar)
- Circle: 16px diameter, `radius-full`
- Background: `accent`
- Text: `text-2xs`, `weight-medium`, `font-ui`, `text-inverse`
- Content: count number (1-9) or "9+" for overflow
- Pulse animation when new notification arrives: `accent-glow` ring pulses 0.3→0.5 alpha, `motion-pulse` (2s), 3 cycles then stops

### Workspace Color Dot (Sidebar)
- 8px circle, `radius-full`
- Color: derived from workspace's assigned ANSI palette color (user-configurable)
- Colors cycle: ANSI blue, green, magenta, cyan, yellow, red (one per workspace)
- Subtle glow on active workspace dot: `glow-subtle` with workspace color instead of accent
