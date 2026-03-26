# Backlog wmux

Référence visuelle : `docs/stitch/` (6 maquettes Google Stitch).
Règle : chaque tâche est **complète** ou **pas commencée**.
Dernière mise à jour : 2026-03-26 (audit `code-explorer`).

## Déjà implémenté

| Changement | Vérification |
|------------|-------------|
| Thème `stitch-blue` (accent `#0979d5`, warning `#dd8b00`) | Config `theme = stitch-blue` → couleurs changent |
| Focus glow paramètres (radius 18px, alpha boost) | **Dead code** tant que #1 n'est pas fait |
| Notification stripe 4px + items 82px | **Dead code** tant que #2 n'est pas fait |
| Filter tabs quads (PaletteFilter enum + pills) | **Dead code** tant que #2 n'est pas fait |
| SystemHandler IPC | Déjà enregistré via `Router::new()` — `wmux-cli system ping` fonctionne |
| Port scanner → sidebar texte | Complet — ports affichés en texte dans l'info workspace |
| Search overlay | Complet — Ctrl+F ouvre la recherche in-pane |
| Session restore | Complet — les workspaces/panes sont restaurés au redémarrage |

---

## Séquence d'implémentation

### #1 — Focus Glow : câbler l'appel dans le render loop

**Réf.** `wmux_terminal_dashboard_main_view.png` (halo bleu autour du pane actif)
**Prérequis :** aucun
**Effort :** faible — la fonction existe, il manque l'appel

`render_focus_glow()` est implémentée (`pane.rs:125-163`) mais **jamais appelée** depuis `render.rs`. L'animation `focus_glow_anim` tourne dans le vide — son alpha n'est jamais lu.

**Scope :**
- Dans `render.rs`, après le rendu des sidebar/pane backgrounds et avant les tab bars : appeler `PaneRenderer::render_focus_glow()` avec le rect du pane focusé
- Lire `animation.get(focus_glow_anim)` pour obtenir l'alpha de cross-fade
- Passer `accent_glow_core` et `accent_glow` depuis `UiChrome`

**Vérification :**
1. `cargo run -p wmux-app`
2. Créer un split (`Ctrl+D`) → 2 panes
3. Cliquer sur un pane → un halo bleu lumineux apparaît autour du pane actif
4. Cliquer sur l'autre pane → le halo se déplace avec une animation de transition
5. Le pane inactif n'a pas de halo

**Fichiers :** `wmux-ui/src/window/render.rs`

---

### #2 — Command Palette complète

**Réf.** `wmux_command_palette_overlay.png`
**Prérequis :** aucun
**Inclut :** câblage UiState + rendu quads + texte COMPLET (input, filter tabs, résultats, raccourcis) + interactions clavier

`CommandPalette` et `CommandRegistry` existent mais ne sont pas dans `UiState`. Les filter tabs ont des quads mais pas de texte. Aucun glyphon `Buffer` n'existe pour le contenu de la palette.

**Scope :**
- Ajouter `CommandPalette` + `CommandRegistry` dans `UiState`
- Câbler `Ctrl+Shift+P` → ouvre/ferme la palette (remplacer le placeholder `handlers.rs:671`)
- Intercepter le clavier quand ouvert (saisie query, flèches, Enter, Escape, Tab pour filter)
- Créer les glyphon `Buffer` pour : placeholder "Type a command...", query saisie, labels filter tabs ("All", "Commands", "Workspaces", "Surfaces"), noms de commandes, badges raccourcis
- Appeler `render_quads()` et les text areas dans le render loop
- Filtrer les résultats selon le tab actif
- Exécuter la commande sélectionnée (mapping → ShortcutAction)

**Vérification :**
1. `cargo run -p wmux-app`
2. `Ctrl+Shift+P` → overlay sombre + palette centrée
3. On voit "Type a command..." dans le champ de saisie
4. Les 4 filter tabs affichent leur texte ("All" surligné bleu, les autres gris)
5. Taper "split" → résultats filtrés avec noms + raccourcis
6. Flèche bas → sélection descend
7. Enter → commande exécutée
8. Escape → palette fermée

**Fichiers :** `wmux-ui/src/command_palette.rs`, `wmux-ui/src/window/mod.rs`, `wmux-ui/src/window/handlers.rs`, `wmux-ui/src/window/render.rs`

---

### #3 — Notification Panel complet

**Réf.** `wmux_notification_panel_slide_out.png`
**Prérequis :** aucun
**Inclut :** câblage UiState + header + texte items + badge count

`NotificationPanel` existe mais pas dans `UiState`. `text_areas()` retourne un Vec vide quand il y a des notifications (`notification_panel.rs:219`). Le badge sidebar rend un cercle mais pas le chiffre.

**Scope :**
- Ajouter `NotificationPanel` dans `UiState`
- Câbler `Ctrl+Shift+I` → toggle (remplacer placeholder `handlers.rs:687`)
- Câbler `Ctrl+Shift+U` → jump to last unread (remplacer placeholder `handlers.rs:691`)
- Alimenter avec `NotificationStore` via le snapshot de l'actor
- Créer les glyphon `Buffer` pour :
  - Header : titre "Notifications" (title font bold), "Clear all" (text_muted), icône X
  - Par item : label catégorie coloré ("Build success" vert, "Warning" jaune), titre bold, description secondary, timestamp faint
- Corriger `text_areas()` : ne plus retourner vide quand `!notifications.is_empty()`
- Ajouter texte du chiffre dans le badge sidebar (`sidebar.rs:387-398`)
- Scroll molette, hover highlight, clic notification → focus workspace source

**Vérification :**
1. `cargo run -p wmux-app`
2. Générer une notification (via OSC escape sequence ou IPC)
3. La sidebar montre un badge cercle avec le **chiffre** "1" à l'intérieur
4. `Ctrl+Shift+I` → panel glisse depuis la droite
5. Header "Notifications" visible + "Clear all" + bouton X
6. Chaque notification : label catégorie coloré + titre + description + timestamp
7. Stripe gauche colorée par sévérité
8. Scroll molette fonctionne
9. `Ctrl+Shift+I` → panel se ferme

**Fichiers :** `wmux-ui/src/notification_panel.rs`, `wmux-ui/src/sidebar.rs`, `wmux-ui/src/window/mod.rs`, `wmux-ui/src/window/handlers.rs`, `wmux-ui/src/window/render.rs`

---

### #4 — Sidebar port badges colorés

**Réf.** `wmux_sidebar_workspace_details_view.png`
**Prérequis :** aucun

Ports actuellement en texte plain. Maquette = pills colorés avec fond + texte.

**Scope :**
- Retirer les ports de `build_info_text()`
- Ajouter des quads pill arrondis + créer glyphon `Buffer` par port visible **dans la même implémentation**
- Couleurs cyclées : accent, success, warning, dot_purple, dot_cyan (fond à 15% alpha)
- Texte du port (":3000") centré dans le pill

**Vérification :**
1. `cargo run -p wmux-app`
2. Lancer `python -m http.server 3000`
3. Attendre ~15s (scan ports)
4. La sidebar montre un badge pill coloré `:3000` avec fond bleu translucide et texte blanc

**Fichiers :** `wmux-ui/src/sidebar.rs`

---

### #5 — Sidebar mode collapsed (icon-only)

**Réf.** `wmux_multi_pane_layout_with_browser_preview.png`
**Prérequis :** #4 (badges doivent se masquer en collapsed)

**Scope :**
- Ajouter `collapsed: bool` à `SidebarState`
- Collapsed = 48px, icônes workspace centrées, aucun texte ni badge
- Raccourci `Ctrl+B` pour toggle
- Viewport s'adapte (sidebar width = 48)

**Vérification :**
1. `cargo run -p wmux-app`
2. `Ctrl+B` → sidebar réduite à une colonne d'icônes (~48px)
3. Noms, branches, ports disparaissent
4. Contenu des panes s'élargit
5. `Ctrl+B` → sidebar pleine largeur
6. Clic sur icône en collapsed → switch de workspace

**Fichiers :** `wmux-ui/src/sidebar.rs`, `wmux-ui/src/shortcuts.rs`, `wmux-ui/src/window/render.rs`

---

### #6 — Tab bar : style toggle shell/browser

**Réf.** `wmux_terminal_dashboard_main_view.png`
**Prérequis :** aucun

**Scope :**
- Toggle 2 segments "shell | browser" quand le pane a 1 terminal + 1 browser
- Sinon : pills individuelles avec icônes terminal/globe améliorées

**Vérification :**
1. `cargo run -p wmux-app`
2. Ouvrir un browser dans un pane (bouton globe)
3. Tab bar montre "shell | browser" en toggle
4. Cliquer "browser" → panel browser
5. Cliquer "shell" → retour terminal

**Fichiers :** `wmux-render/src/pane.rs`, `wmux-ui/src/window/render.rs`

---

### #7 — Custom title bar

**Réf.** `wmux_command_palette_overlay.png`
**Prérequis :** #1 à #6 terminés (stabiliser avant de toucher au window chrome)

**Scope :**
- `WM_NCCALCSIZE` pour supprimer la barre standard
- `WM_NCHITTEST` pour drag, close, min, max
- Rendu wgpu : fond + texte "wmux" centré + boutons Codicons
- Snap Windows (bords/haut)
- Fallback vers barre standard si erreur

**Vérification :**
1. `cargo run -p wmux-app`
2. Barre titre custom avec "wmux" centré
3. Drag → fenêtre se déplace
4. Boutons close/min/max fonctionnent
5. Double-clic → maximize/restore
6. Drag vers haut → snap maximize

**Fichiers :** `wmux-ui/src/effects.rs`, `wmux-ui/src/window/event_loop.rs`, `wmux-ui/src/window/render.rs`

---

### #8 — Typographie Inter (optionnel)

**Prérequis :** aucun

**Scope :** Bundler Inter Regular + Bold, charger pour UI chrome.

**Vérification :** Comparer visuellement avant/après.

**Fichiers :** `wmux-render/src/`, `wmux-ui/src/typography.rs`

---

## Non-Stitch (priorité basse)

| # | Feature | Effort | Vérification |
|---|---------|--------|-------------|
| 9 | Sidebar progress bar + activity log | Moyen | IPC `sidebar.set_progress` → barre visible |
| 10 | Auto-updater | Moyen | Au démarrage → notification si MAJ dispo |
| 11 | i18n dans l'UI | Haut | Config `language = fr` → UI en français |
| 12 | Config keybindings custom | Moyen | Config `keybind = ctrl+k=split_right` → raccourci fonctionne |
| 13 | CLI notify commands | Moyen | `wmux notify create` → notification dans le panel (requiert #3) |
| 14 | CLI SSH + daemon Go | Haut | `wmux ssh connect user@host` → workspace distant |
| 15 | Browser automation (25+ méthodes) | Haut | `wmux-cli browser click "#btn"` → clic WebView2 |
| 16 | ToggleDevTools (F12) | Faible | F12 → WebView2 devtools ou debug overlay |
