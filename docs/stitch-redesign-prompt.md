# Stitch Redesign Prompt — wmux

> **Usage** : Copier le prompt ci-dessous dans Google Stitch (stitch.withgoogle.com).
> **Mode recommandé** : Standard (Gemini 3) pour la qualité maximale.
> **Astuce** : Joindre un screenshot de l'application actuelle en plus du prompt pour que Stitch comprenne le layout existant.
> **Stratégie** : Commencer par le prompt principal (écran 1), puis itérer écran par écran avec les prompts de refinement.

---

## Prompt principal (écran 1 — Vue principale)

```
Design a modern desktop application UI for "wmux", a GPU-accelerated terminal multiplexer for Windows — similar to Warp, iTerm2, or VS Code's integrated terminal, but as a standalone app.

Platform: Desktop (1920x1080 viewport, not mobile).

Layout structure:
— Left sidebar (collapsible, ~220px): Vertical list of "workspaces" (like VS Code's Explorer but for terminal sessions). Each workspace card shows: workspace name, git branch badge, working directory path, listening ports, and a small colored accent bar on the left edge. Section header "WORKSPACES" at top. A "+" button to create new workspace. Support drag-and-drop reordering visual cue.
— Main content area (right of sidebar): Split into 2 panes (one top-left, one right), each with its own horizontal tab bar at the top. Each tab bar has pill-shaped tabs showing tab name + close button, plus action buttons: "+" (new tab), split icon, globe icon (browser tab).
— Bottom status bar (full width, slim ~32px): Shows workspace name, pane count, git branch, shell type, and a small connection status dot (green = connected).

Content inside panes: Dark terminal with monospace text showing typical CLI output (git status, npm commands, or cargo build output). One pane shows a terminal, the other shows a browser panel (embedded web preview with URL bar).

Visual style:
— Dark theme inspired by "Luminous Void" aesthetic — deep charcoal/near-black backgrounds (#131313 base), NOT pure black.
— Electric blue accent color for focus states, active tabs, and workspace selection indicators.
— Elevation system with subtle surface layering: sidebar slightly lighter than base, tab bars slightly lighter than sidebar, hover states lighter still. No hard borders — use subtle elevation and shadow depth instead.
— Focus glow effect: the active/focused pane has a subtle blue glow halo around its border (like a neon outline, 20% opacity).
— Typography: clean sans-serif (Inter or Segoe UI style) for UI chrome. Monospace font in terminal areas only.
— Rounded corners on cards, tabs (4-8px radius), buttons. Pill-shaped tabs with gaps between them.
— Subtle shadows: small shadow under tab bars, medium shadow on sidebar, large shadow on overlays.
— Fluent Design / Material Design 3 hybrid — depth, translucency hints, refined spacing.
— Premium, professional feel — think Stripe dashboard meets VS Code meets Warp terminal.

Color palette:
— Backgrounds: #131313 (base), #1a1a1a (surface 0), #212121 (surface 1), #2a2a2a (surface 2)
— Accent: #4a9eff (electric blue), hover: +8% lighter, pressed: -10% darker
— Text: #e0e0e0 (primary), rgba(255,255,255,0.88) (secondary), rgba(255,255,255,0.75) (muted)
— Semantic: green for success/connected, red for errors/close buttons, yellow for warnings/search matches
— Borders: subtle, mostly transparent (rgba white at 10-15%)

Spacing: generous whitespace, 12-16px padding in cards, 8px gaps between tabs, breathable layout. Not cramped.

This should look like a premium developer tool — polished, modern, and visually distinctive. NOT generic. The glow effect and deep dark theme should be the signature visual identity.
```

---

## Prompt refinement — Écran 2 : Command Palette

```
Design the Command Palette overlay for wmux.

Same dark theme and visual style as previous screen. The command palette appears as a centered floating modal (600px wide) with:
— Semi-transparent dark backdrop dimming the entire app behind it (50% black overlay)
— Search input at top (44px tall, rounded corners, subtle border, placeholder "Type a command...")
— Filter tabs below the input: "All", "Commands", "Workspaces", "Surfaces" — pill-shaped, subtle borders
— Results list below: each item shows an icon, command name, and keyboard shortcut (right-aligned, in a subtle badge). Items are 36px tall with hover highlight.
— Maximum 12 visible results with scroll indicator
— Large shadow (shadow-lg) around the palette
— Border-radius 12px on the modal

This should feel like VS Code's command palette or Raycast — snappy, clean, keyboard-first. Blue accent on the selected/highlighted result row.
```

---

## Prompt refinement — Écran 3 : Notification Panel

```
Design the Notification Panel for wmux.

Same dark theme. The panel slides in from the right side (360px wide), overlaying the main content:
— Header: "Notifications" title with a close button and "Clear all" text link
— List of notification cards (72px each): each card has an icon (info/success/warning/error with matching semantic color), title text, description text, and a timestamp
— Cards have subtle hover state (lighter surface)
— Unread indicator: small blue dot on unread notifications
— Empty state: centered icon + "No notifications" message when empty
— Subtle shadow on the left edge of the panel
— Smooth slide-in animation suggestion (show it partially visible)

Keep the overall premium dark aesthetic. Notifications should feel clean and scannable.
```

---

## Prompt refinement — Écran 4 : Sidebar détaillée

```
Design an expanded view of the wmux sidebar with rich workspace metadata.

Same dark theme. The sidebar shows 5 workspaces in the list, each as a card with:
— Workspace name (bold, 15px)
— Git branch with a branch icon (muted text)
— Working directory path (truncated, caption size, very muted)
— Listening ports shown as small badges (e.g., ":3000", ":8080") with subtle colored backgrounds
— Notification badge count (small red/blue circle with number) on workspaces that have unread items
— Active workspace: highlighted with accent color left bar (3px), slightly elevated background
— Hover state on inactive workspaces: subtle surface elevation change

At the top: "WORKSPACES" section header in all-caps, caption size, very muted color.
At the bottom of sidebar: a "New Workspace" button (subtle, outlined style).

The sidebar should feel like a refined file explorer panel — information-dense but not cluttered. Good visual hierarchy through typography scale and opacity levels.
```

---

## Prompt refinement — Écran 5 : Search Overlay

```
Design the in-pane Search bar overlay for wmux.

Same dark theme. A floating search bar appears at the top-right of the active terminal pane:
— Compact horizontal bar (38px tall, 320px wide)
— Search input with magnifying glass icon
— Match count display: "3 of 12" in muted text
— Navigation arrows (up/down) to jump between matches
— Regex toggle button (subtle icon toggle)
— Case-sensitive toggle button
— Close button (X)
— Rounded corners (8px), subtle border, small shadow

In the terminal content behind it, show highlighted search matches: yellow background highlight on matching text (30% opacity), with the current/active match in a brighter yellow (50% opacity).

Clean, minimal, non-intrusive — should not block too much terminal content.
```

---

## Prompt refinement — Écran 6 : Multi-pane avec Browser

```
Design a wmux view with 3 split panes showing diverse content.

Same dark theme. Layout:
— Left pane (50% width): Terminal showing cargo build output with colored ANSI text (green for success, yellow for warnings, red for errors)
— Top-right pane (50% width, 60% height): Embedded browser panel showing a web application preview, with a minimal URL bar at the top of the tab bar area showing "localhost:3000"
— Bottom-right pane (50% width, 40% height): Terminal showing Claude Code AI agent output with structured text

Each pane has its own tab bar. The focused pane (left) has the blue glow border effect. Inactive panes are slightly dimmed (70% opacity overlay on content).

Dividers between panes are subtle (1px line), with a slightly wider hit zone suggested by a subtle hover state.

Show the sidebar in collapsed state (just workspace icons, no text) to demonstrate the responsive behavior.

This screen demonstrates the power-user multi-tasking layout — terminals + browser side by side.
```

---

## Notes pour l'adaptation à la stack Rust/wgpu

Ce que Stitch va produire (HTML/CSS) sert de **référence visuelle**, pas de code production. Pour adapter à wmux :

1. **Couleurs et spacing** → Extraire les valeurs exactes du CSS généré et les injecter dans `wmux-config/src/theme/chrome.rs`
2. **Composants** → Chaque div/section du HTML correspond à un quad dans le `QuadPipeline` de wmux-render
3. **Typographie** → Les font-size/line-height CSS mappent directement aux métriques glyphon
4. **Ombres** → Les box-shadow CSS se traduisent en paramètres du `ShadowPipeline`
5. **Layout** → Les flexbox/grid CSS informent les calculs de viewport dans `wmux-ui`
6. **Glow effects** → Les border/box-shadow bleus → paramètres du SDF shader dans le focus glow renderer
7. **Export Figma** → Permet de mesurer précisément chaque élément avant implémentation

Le workflow idéal : **Stitch → Export Figma → Mesurer → Implémenter dans wgpu**
