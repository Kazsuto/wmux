# Backlog wmux

Référence visuelle : `docs/stitch/` (6 maquettes Google Stitch).
Règle : chaque tâche est **complète** ou **pas commencée**.
Dernière mise à jour : 2026-04-19 (audit post v3.3 de l'architecture).

Progression : **48 / 50 specs livrées (96 %)**. Il reste **2 specs** (`L2_16`, `L4_07`) et **11 items de dette / finition** identifiés en comparant `docs/architecture/feature-files.md` au code réel.

## Déjà implémenté

| Changement | Preuve |
|------------|--------|
| Thème `stitch-blue` (accent `#0979d5`, warning `#dd8b00`) | Config `theme = stitch-blue` → couleurs changent |
| Focus glow câblé au render loop | `wmux-ui/src/window/render.rs:781` appelle `PaneRenderer::render_focus_glow()` |
| Command Palette complète | UiState : `palette_query_buffer`, `palette_filter_buffers[4]`, `palette_result_buffers`, `palette_shortcut_buffers` (`window/mod.rs:78-86`), Ctrl+Shift+P câblé, open/close/exec fonctionnels |
| Notification Panel complet | `NotificationPanel` dans UiState (`mod.rs:93`), Ctrl+Shift+I toggle + Ctrl+Shift+U jump-to-unread (`handlers.rs:1013-1040`), buffers header/clear_all/categories/titles/bodies/timestamps (`notification_panel.rs:37-44`), badge chiffre rendu (`sidebar.rs:361-365, 845-852`) |
| Sidebar port badges colorés (pills) | `port_pill_width()` + quads pill à alpha 15 % + texte centré (`sidebar.rs:36-42, 388-934`) |
| Sidebar mode collapsed (icon-only) | `SidebarState.collapsed: bool` + `toggle_collapsed()` + width 48px (`sidebar.rs:86-174`) |
| Tab bar : toggle shell/browser | `toggle_segment_rect`, `toggle_segment_to_tab`, `active_toggle_segment` (`wmux-render/src/pane.rs:482-558`), close button X ajouté |
| Custom title bar | `wmux-ui/src/titlebar.rs` (583 lignes), `WM_NCCALCSIZE`/`WM_NCHITTEST` via `SetWindowSubclass`, boutons Codicons (minimize/maximize/restore/close), theme-driven |
| Auto-updater durci | `wmux-app/src/updater.rs`, SHA-256 obligatoire, HTTPS + host allowlist, limite 200 MB, path containment, recovery interrompu |
| i18n dans l'UI (fr/en) | `wmux-config/src/locale.rs` + 12+ strings câblées via `locale.t()` dans notification panel, tab menus, severity labels, toggle labels, time-ago |
| SystemHandler IPC | `system.ping` fonctionne via `Router::new()` |
| Port scanner → sidebar | Ports affichés en pills dans la sidebar |
| Search overlay | Ctrl+F ouvre la recherche in-pane |
| Session restore | Workspaces/panes restaurés au redémarrage |
| Browser automation (IPC) | 30+ méthodes `browser.*` exposées dans `wmux-ipc/src/handlers/browser.rs` (click, fill, snapshot, screenshot, cookies, storage...) |
| CLI foundation + sous-commandes workspace/surface/sidebar/system | Fichiers `wmux-cli/src/commands/{workspace,surface,sidebar,system}.rs` existent et sont câblés à l'IPC, client `client.rs` fonctionnel, formatter `output.rs` (humain + `--json`) |

---

## Séquence d'implémentation

Structurée en **4 phases**. Chaque phase est un bloc dont les items peuvent s'implémenter en parallèle sans interférence. Les phases s'enchaînent séquentiellement : ne pas commencer la phase `N+1` avant que `N` soit stable, sauf mention contraire.

### Légende dépendances

- **Backend prêt** : toute la plomberie existe, il ne manque que le wiring ou le rendu final.
- **Prérequis externe** : dépend d'un composant hors du code Rust (font, binaire Go, toolchain packaging).
- **Bloquant release** : `L4_07` ne peut pas livrer tant que ça traîne.

---

## Phase A — Finition UI + handlers câblés (aucun risque, 4 items indépendants)

Objectif : rendre visibles ou fonctionnels les éléments dont le backend est déjà en place. Aucune modification d'architecture, purement du wiring et du rendu. Tous les items de cette phase peuvent se faire en parallèle, aucun ne bloque les autres.

### #A1 — Typographie Inter

**Prérequis :** aucun
**Effort :** faible
**Dépendances :** aucune

`wmux-ui/src/typography.rs` ne définit que les tokens de taille (TITLE / BODY / CAPTION / BADGE). Aucune police custom n'est chargée, l'UI chrome utilise la police système par défaut. Le dossier `resources/fonts/` n'existe pas encore.

**Scope :**
- Créer `resources/fonts/` et y placer Inter Regular + Bold (`.ttf`)
- Charger les `.ttf` via `FontSystem::db_mut().load_font_source()` au démarrage de `wmux-render`
- Référencer `Family::Name("Inter")` dans les `Attrs` du chrome (sidebar, tab bar, palette, notifications, titlebar, status bar)
- Fallback sur Segoe UI si le chargement échoue

**Vérification :**
1. `cargo run -p wmux-app`
2. Comparer avant / après : texte sidebar, tabs, palette, titlebar en Inter, pas Segoe UI
3. Tester avec `resources/fonts/` supprimé : fallback Segoe UI, pas de crash

**Fichiers :** `resources/fonts/Inter-Regular.ttf`, `resources/fonts/Inter-Bold.ttf` (nouveaux), `wmux-render/src/text.rs` ou `gpu.rs` (font loading), `wmux-ui/src/typography.rs`

---

### #A2 — ToggleDevTools (F12) — câblage handler

**Prérequis :** aucun
**Effort :** faible
**Dépendances :** aucune, fonction `OpenDevToolsWindow` COM existe déjà

F12 est câblé à `ShortcutAction::ToggleDevTools` (`shortcuts.rs:198`), la fonction `OpenDevToolsWindow()` existe dans `wmux-browser/src/panel/layout.rs:134-136`. Le handler dans `wmux-ui/src/window/handlers.rs:1010-1012` est un placeholder : `tracing::debug!("ToggleDevTools shortcut (placeholder)")`. Le pont entre le raccourci et la fonction COM n'est pas fait.

**Scope :**
- Dans `handlers.rs:1010`, remplacer le placeholder par un appel au browser panel focusé
- Router vers `BrowserPanel::open_devtools()` si le pane actif contient un browser
- Sinon : tracer `no browser in focused pane` sans erreur
- Release builds : désactivé (déjà géré côté WebView2)

**Vérification :**
1. `cargo run -p wmux-app`
2. Ouvrir un browser dans un pane
3. F12 → fenêtre DevTools WebView2 s'ouvre
4. F12 à nouveau → DevTools se ferme
5. F12 sur un pane terminal → pas de DevTools, pas de crash

**Fichiers :** `wmux-ui/src/window/handlers.rs`, `wmux-browser/src/panel/layout.rs` (exposer la fonction publiquement si besoin)

---

### #A3 — Sidebar progress bar (rendu UI)

**Prérequis :** aucun
**Effort :** faible
**Dépendances :** backend prêt (`MetadataStore.set_progress()` + `ProgressState` câblés, CLI `wmux sidebar set-progress` fonctionnel via IPC)

Le backend existe : `MetadataStore.set_progress(value, label)` + `ProgressState` (`wmux-core/src/metadata_store.rs:113-131`). Aucune référence à `progress_bar`, `render_progress` ou `ProgressState` dans `wmux-ui/src/sidebar.rs` : la donnée est stockée et jamais dessinée.

**Scope :**
- Lire `ProgressState` du workspace dans `sidebar.rs`
- Dessiner un quad barre fond + quad barre remplissage (alpha basé sur `value 0.0-1.0`)
- Afficher le label optionnel en texte caption
- Position : sous le nom du workspace, au-dessus des ports
- Respecter le mode collapsed (masquer ou n'afficher que l'anneau de progression)

**Vérification :**
1. `cargo run -p wmux-app`
2. Depuis un shell intégré : `wmux-cli sidebar set-progress 0.5 --label "Building"`
3. Barre visible à 50 % dans la ligne du workspace courant avec le label `Building`
4. `wmux-cli sidebar set-progress 1.0` → barre pleine
5. Basculer en mode collapsed → vérifier le comportement choisi

**Fichiers :** `wmux-ui/src/sidebar.rs`

---

### #A4 — Config keybindings custom (wiring)

**Prérequis :** aucun
**Effort :** moyen
**Dépendances :** parser config stocke déjà la HashMap

Le parser config stocke `keybind = ctrl+n=new_workspace` dans `Config.keybindings: HashMap<String, String>` (`wmux-config/src/config.rs:211`). Aucun code n'utilise `config.keybindings` dans `wmux-ui/src/shortcuts.rs`, le shortcut system reste 100 % hardcodé. La HashMap est parsée puis ignorée.

**Scope :**
- Ajouter `apply_custom_keybindings(&Config)` dans `wmux-ui/src/shortcuts.rs`
- Mapper les strings (`ctrl+n`, `split_right`) vers `(KeyCombo, ShortcutAction)`
- Override la table par défaut au démarrage, étendre si action custom
- Validation : rejeter les actions inconnues avec `tracing::warn!`, rejeter les combos malformés
- Passer la `Config` depuis `wmux-app/src/main.rs` vers `UiState::new()`

**Vérification :**
1. Ajouter `keybind = ctrl+k=split_right` dans la config
2. `cargo run -p wmux-app`
3. Ctrl+K → split right fonctionne (override du raccourci par défaut)
4. Ajouter `keybind = ctrl+shift+z=unknown_action` → warn dans les logs, pas de crash

**Fichiers :** `wmux-ui/src/shortcuts.rs`, `wmux-app/src/main.rs` (propagation de la config)

---

## Phase B — Complétion des surfaces CLI (4 items indépendants)

Objectif : finir la couverture CLI pour que chaque feature IPC ait un équivalent en ligne de commande. C'est la **clé pour les agents IA** (Claude Code, Codex) : aujourd'hui beaucoup de méthodes IPC sont inaccessibles depuis `wmux-cli`. Tous les items de cette phase peuvent se faire en parallèle. Cette phase **finit la spec `L2_16`** et ajoute 2 items hors spec (theme, update).

### #B1 — CLI browser automation (compléter les sous-commandes)

**Prérequis :** aucun
**Effort :** moyen
**Dépendances :** IPC `browser.*` (30+ méthodes) opérationnel

IPC expose **30+ méthodes** (`browser.click`, `browser.fill`, `browser.snapshot`, `browser.screenshot`, `browser.cookies`, `browser.storage`, `browser.hover`, `browser.focus`, `browser.select`, `browser.check`, `browser.uncheck`, `browser.scroll`, `browser.get`, `browser.is`, `browser.find`, `browser.highlight`, `browser.wait`, `browser.console`, `browser.errors`, `browser.state`, `browser.tab`, `browser.add_init_script`, etc.). Le CLI `wmux-cli/src/commands/browser.rs` n'expose que **7 sous-commandes** (Open, Navigate, Back, Forward, Reload, Url, Eval). L'automation complète est accessible par JSON-RPC direct mais pas via `wmux-cli`.

**Scope :**
- Ajouter les variantes manquantes à `BrowserCommands` : Click, DblClick, Hover, Focus, Fill, Type, Press, Select, Check, Uncheck, Scroll, Snapshot, Screenshot, Get, Is, Find, Highlight, Wait, Console, Errors, Cookies, Storage, State, Tab, AddInitScript
- Chaque variante mappée vers la méthode IPC correspondante
- Output JSON pour les résultats structurés (snapshot, screenshot en base64, cookies, storage)
- Ajouter `--surface-id` en option globale quand il manque déjà

**Vérification :**
1. `cargo run -p wmux-app`
2. Ouvrir un browser vers une page avec un bouton
3. `wmux-cli browser click "#submit" --surface-id <id>` → clic déclenché
4. `wmux-cli browser snapshot --surface-id <id>` → JSON de l'arbre accessibility
5. `wmux-cli browser screenshot --surface-id <id>` → PNG base64 en JSON
6. `wmux-cli browser fill "#email" "test@example.com" --surface-id <id>` → champ rempli

**Fichiers :** `wmux-cli/src/commands/browser.rs`

---

### #B2 — CLI notify commands (déstubber)

**Prérequis :** aucun
**Effort :** moyen
**Dépendances :** IPC `notification.*` déjà routé via `handlers/sidebar.rs`

`wmux-cli/src/commands/notify.rs:31-37` retourne explicitement `RpcErrorCode::InternalError : "not yet implemented (pending Task L3_08)"` pour les 3 sous-commandes (Create / List / Clear). Aucun appel IPC réel. Pourtant `L3_08` (notification store) et `L3_10` (toast) sont livrés depuis longtemps.

**Scope :**
- Câbler Create → `notification.create` IPC avec `{ title, body }`
- Câbler List → `notification.list` IPC (tableau humain, ou JSON avec `--json`)
- Câbler Clear → `notification.clear_all` IPC
- Retirer complètement le stub d'erreur `InternalError`
- Vérifier que les handlers IPC existent : sinon ajouter `notification.create`, `notification.list`, `notification.clear_all` dans `wmux-ipc/src/handlers/sidebar.rs` (le metadata store possède déjà la `NotificationStore`)

**Vérification :**
1. `cargo run -p wmux-app`
2. `wmux-cli notify create "Build success" --body "Tests passed"` → notification visible dans le panel Ctrl+Shift+I + toast Windows
3. `wmux-cli notify list` → JSON des notifications actives avec id / title / body / timestamp / read
4. `wmux-cli notify clear` → panel vidé

**Fichiers :** `wmux-cli/src/commands/notify.rs`, éventuellement `wmux-ipc/src/handlers/sidebar.rs` si des méthodes IPC manquent

---

### #B3 — CLI theme commands (nouveau fichier)

**Prérequis :** aucun
**Effort :** faible
**Dépendances :** theme engine opérationnel (8 thèmes bundlés)

`wmux-cli/src/commands/theme.rs` n'existe pas. Le thème se change uniquement via le fichier de config. Les agents ne peuvent pas lister ni switcher le thème en runtime. Aucune méthode IPC `theme.*` n'existe non plus.

**Scope :**
- Créer `wmux-cli/src/commands/theme.rs` avec 3 sous-commandes : `list`, `set <name>`, `current`
- Ajouter 3 méthodes IPC dans `wmux-ipc/src/handlers/system.rs` (ou un nouveau `handlers/theme.rs`) : `theme.list`, `theme.set`, `theme.current`
- Côté UI : `theme.set` déclenche un re-render avec la nouvelle palette (theme engine gère déjà le live switch)
- Wirer la nouvelle subcommand dans `wmux-cli/src/commands/mod.rs` et `main.rs`

**Vérification :**
1. `wmux-cli theme list` → liste des 8 thèmes (catppuccin-mocha, digital-obsidian, dracula, gruvbox-dark, nord, one-dark, stitch-blue, wmux-default)
2. `wmux-cli theme current` → nom du thème actif
3. `wmux-cli theme set dracula` → UI bascule sur dracula sans redémarrer

**Fichiers :** `wmux-cli/src/commands/theme.rs` (nouveau), `wmux-cli/src/commands/mod.rs`, `wmux-cli/src/main.rs`, `wmux-ipc/src/handlers/` (nouveau handler ou extension)

---

### #B4 — CLI update commands (nouveau fichier)

**Prérequis :** aucun
**Effort :** faible
**Dépendances :** `UpdateChecker` dans `wmux-app/src/updater.rs` opérationnel

`wmux-cli/src/commands/update.rs` n'existe pas. L'auto-updater tourne en arrière-plan mais n'est pas pilotable depuis le CLI. Aucune méthode IPC `update.*` n'existe.

**Scope :**
- Créer `wmux-cli/src/commands/update.rs` avec 2 sous-commandes : `check`, `install`
- Ajouter 2 méthodes IPC : `update.check` (force un poll + retourne la version cible), `update.install` (applique la mise à jour stagée)
- Output JSON : `{current: "0.1.0", latest: "0.1.3", available: true, staged: false}`
- Wirer la nouvelle subcommand

**Vérification :**
1. `wmux-cli update check` → retourne la version courante + dernière dispo
2. `wmux-cli update install` → applique la mise à jour (prompt confirmation en mode humain, direct en `--json`)

**Fichiers :** `wmux-cli/src/commands/update.rs` (nouveau), `wmux-cli/src/commands/mod.rs`, `wmux-cli/src/main.rs`, `wmux-ipc/src/handlers/` (extension), `wmux-app/src/updater.rs` (exposer une API)

---

## Phase C — SSH remote complet (effort élevé, 3 sous-étapes ordonnées)

Objectif : livrer le feature SSH remote de bout en bout. **Phase optionnelle pour la v0.1 publique** : si l'objectif est de sortir vite, SSH peut être reporté à la v0.2 et `L4_07` peut packager sans le daemon Go (juste un binaire absent du bundle). Si on garde SSH dans la release, les 3 sous-étapes sont **strictement séquentielles**.

### #C1 — wmuxd-remote : daemon Go

**Prérequis :** aucun
**Effort :** haut
**Dépendances :** code existant dans cmux à réutiliser ou à recréer, le modèle `RemoteConfig` côté Rust est déjà là (`wmux-core/src/remote.rs`)

Le dossier `daemon/` n'existe pas dans le repo. `wmux-cli/src/commands/ssh.rs:25-29` retourne explicitement « SSH remote connection is not yet fully implemented », « Remote workspace model is ready, daemon integration pending ».

**Scope :**
- Adapter le daemon `cmuxd-remote` (Go) du projet cmux, ou le réécrire à partir de ses specs
- Placer dans `daemon/remote/cmd/wmuxd-remote/main.go`
- Fonctionnalités minimales : PTY relay, heartbeat, reconnect resumption
- Compilation cross-platform (Linux, macOS, Windows) dans le CI
- Binaire inclus dans le bundle release (sera utilisé par `L4_07`)

**Vérification :**
1. `cd daemon && go build ./...` compile sans erreur sur les 3 OS
2. Daemon démarre en standalone, écoute sur stdio, réagit au heartbeat
3. Tests Go unitaires passent

**Fichiers :** `daemon/remote/cmd/wmuxd-remote/main.go` (nouveau), `daemon/go.mod`, `daemon/remote/internal/...` (structure cmux ou équivalente)

---

### #C2 — CLI SSH connect : tunnel + daemon bootstrap

**Prérequis :** #C1 terminé
**Effort :** moyen
**Dépendances :** binaire `wmuxd-remote` compilé et accessible dans le PATH du client wmux

`wmux-cli/src/commands/ssh.rs` parse déjà `RemoteConfig` puis s'arrête. Il faut ouvrir un tunnel SSH (via `ssh.exe` natif Windows ou une lib Rust), copier le binaire daemon sur l'hôte distant, le lancer, brancher le PTY local sur le relay distant.

**Scope :**
- `ssh::connect(target)` : ouvre un tunnel SSH (subprocess `ssh.exe -T -o ...`)
- Auto-deploy du binaire `wmuxd-remote` sur l'hôte si absent (scp ou heredoc)
- Création d'un workspace distant via `workspace.create` IPC local avec flag `remote = true`
- Pipe du PTY local via le tunnel SSH vers le daemon
- Retirer les `eprintln!("not yet fully implemented")`

**Vérification :**
1. `wmux-cli ssh connect user@host` → workspace distant créé, shell distant dans un pane
2. `wmux-cli ssh disconnect` → workspace distant détruit proprement
3. Perte réseau : sidebar affiche l'icône SSH disconnect

**Fichiers :** `wmux-cli/src/commands/ssh.rs`, `wmux-ipc/src/handlers/workspace.rs` (flag remote), `wmux-core/src/remote.rs` (extension si nécessaire)

---

### #C3 — SSH reconnect + multi-client + browser proxy

**Prérequis :** #C1 et #C2 terminés
**Effort :** haut
**Dépendances :** tunnel + daemon opérationnels

Fonctionnalités du PRD 9 pas encore livrées : reconnect automatique avec backoff, coordination multi-client sur le même hôte, browser proxy SOCKS5 / HTTP CONNECT pour les panes browser distants.

**Scope :**
- Reconnexion automatique en cas de coupure : backoff exponentiel, état « reconnecting » dans la sidebar
- Restauration de la session distante après reconnect (le daemon maintient le PTY côté serveur)
- SOCKS5 / HTTP CONNECT proxy embarqué dans le daemon pour les browser panes distants
- Coordination multi-client : plusieurs wmux connectés au même daemon partagent la session, résolution des resize

**Vérification :**
1. Tuer le tunnel SSH manuellement → sidebar passe en reconnecting, reprend après ~5s
2. Ouvrir un browser dans un pane distant → traffic passe par le proxy, la page charge
3. Connecter 2 instances wmux au même hôte → mêmes workspaces, resize coordonné

**Fichiers :** `daemon/remote/internal/reconnect.go`, `daemon/remote/internal/proxy.go`, `wmux-core/src/remote.rs`, `wmux-cli/src/commands/ssh.rs`

---

## Phase D — Packaging & release (item unique, dépend de tout ce qui précède)

Objectif : livrer une release publique. **C'est la dernière barrière avant la première release GitHub.** Peut partir dès que les phases A et B sont stables si SSH est reporté, sinon attendre la phase C.

### #D1 — L4_07 Packaging + Distribution

**Prérequis :** Phases A et B terminées. Phase C terminée si SSH dans la release, sinon documenter l'absence du daemon.
**Effort :** moyen
**Dépendances :** `cargo-wix` (MSI), GitHub Actions, binaire `wmuxd-remote` compilé (si SSH in)

Rien n'existe : pas d'`installer/`, pas de `.github/workflows/release.yml`, pas de `winget/`, pas de `scoop/`, pas d'`.ico`.

**Scope :**
- Asset `.ico` et `app.manifest` (DPI-aware, version metadata)
- `build.rs` pour embed ico + manifest dans `wmux-app.exe`
- `installer/wmux.wxs` via WiX 4 (MSI qui pose `wmux-app.exe`, `wmux-cli.exe`, ajoute CLI au PATH, registre AUMID pour Toast, inclut `wmuxd-remote.exe` si Phase C livrée)
- `scripts/package.ps1` build + package script
- `winget/wmux.yaml` manifest
- `scoop/wmux.json` manifest
- `.github/workflows/release.yml` : trigger sur tag `v*`, clippy → fmt → test → build release → package MSI + zip portable → créer GitHub Release avec les artefacts
- `.github/workflows/ci.yml` (si pas déjà là) pour les PRs

**Vérification :**
1. `cargo build --release --workspace` produit les binaires attendus (< 15 MB app, < 5 MB cli)
2. MSI installe, ajoute au PATH, crée le raccourci menu démarrer, lance wmux correctement
3. Désinstallation : plus rien dans `Program Files`, entrée du menu supprimée
4. `scoop install wmux` depuis le bucket custom → binaires en place
5. Push d'un tag `v0.1.0` → CI build et publie automatiquement la release

**Fichiers :** `installer/wmux.wxs`, `scripts/package.ps1`, `winget/wmux.yaml`, `scoop/wmux.json`, `.github/workflows/release.yml`, `resources/icons/wmux.ico`, `wmux-app/app.manifest`, `wmux-app/build.rs`

---

## Graphe de dépendances (résumé)

```text
A1 Inter font        ─┐
A2 F12 DevTools       │
A3 Progress bar UI    ├── indépendants, parallélisables
A4 Keybindings        ─┘
                       │
B1 CLI browser       ─┐
B2 CLI notify         │
B3 CLI theme          ├── indépendants, parallélisables
B4 CLI update        ─┘
                       │
C1 Go daemon         ──┐
                       │  (séquentiel)
C2 CLI SSH connect   ──┤
                       │
C3 SSH reconnect+    ──┘
                       │
                       ▼
              D1 Packaging L4_07
              (bloqué tant que A, B, et C optionnellement pas finis)
```

---

## Séquence recommandée

Le chemin le plus court vers une première release publique :

1. **Tout Phase A en parallèle** (4 items, ~1 journée cumulée) → l'UI devient complète
2. **Tout Phase B en parallèle** (4 items, ~1 à 2 journées cumulées) → le CLI couvre toute l'API
3. **Décision release** : inclure SSH ou le reporter en v0.2 ?
   - Si **oui** : Phase C en séquence C1 → C2 → C3 (plusieurs jours, effort haut)
   - Si **non** : sauter Phase C, noter `SSH: pending v0.2` dans le README + laisser le stub actuel de `wmux-cli ssh`
4. **Phase D packaging** quand tout le reste est stable. Premier tag `v0.1.0`

À noter : rien dans cette liste ne demande un refactor d'architecture. Les 10 ADRs restent tous `Accepted`, aucune décision n'a besoin d'être superseded.
