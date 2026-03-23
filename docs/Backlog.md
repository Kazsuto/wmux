# Fonctionnalités codées mais non fonctionnelles

## 1. Command Palette (Ctrl+Shift+P) — NON BRANCHÉ

**Fichiers concernés:**
- `wmux-ui/src/command_palette.rs` — Module complet avec state, rendu des quads, hit-testing
- `wmux-core/src/command_registry.rs` — Registre de 16 commandes avec recherche fuzzy
- `wmux-ui/src/window/handlers.rs` ligne 652-653

**Problème:** Le raccourci Ctrl+Shift+P est détecté et matche `ShortcutAction::CommandPalette`, mais le handler ne fait qu'un `tracing::debug!("CommandPalette shortcut (placeholder - Task L4_01)")`. Le `CommandPalette` struct et le `CommandRegistry` sont entièrement implémentés avec rendu, navigation, recherche fuzzy, mais aucun n'est instancié dans `UiState` ni appelé dans le pipeline de rendu. Il n'y a pas de champ `command_palette` dans `UiState`.

**À faire:**
- Ajouter un champ `CommandPalette` et `CommandRegistry` dans `UiState`
- Brancher le shortcut pour ouvrir/fermer la palette
- Intercepter les touches quand la palette est ouverte (query input, navigation haut/bas, Enter pour exécuter)
- Appeler `command_palette.render_quads()` dans le pipeline de rendu
- Ajouter les TextAreas glyphon pour le texte de la palette
- Exécuter la commande sélectionnée via mapping command_id -> ShortcutAction

---

## 2. Notification Panel (Ctrl+Shift+I) — NON BRANCHÉ

**Fichiers concernés:**
- `wmux-ui/src/notification_panel.rs` — Module complet avec state, quads, scroll, hit-test, text areas
- `wmux-ui/src/window/handlers.rs` lignes 668-672

**Problème:** Le raccourci Ctrl+Shift+I est détecté comme `ShortcutAction::NotificationPanelToggle` mais le handler ne fait qu'un `tracing::debug!("NotificationPanelToggle shortcut (placeholder - L3_09 UI wiring)")`. Le `NotificationPanel` struct est entièrement implémenté avec rendu, scroll, hover, mais il n'est pas instancié dans `UiState` et jamais rendu. Même chose pour `JumpLastUnread` (Ctrl+Shift+U).

**À faire:**
- Ajouter un champ `NotificationPanel` dans `UiState`
- Brancher le shortcut `NotificationPanelToggle` pour toggle le panel
- Brancher `JumpLastUnread` pour naviguer à la dernière notification
- Appeler `notification_panel.render_quads()` dans le pipeline de rendu
- Ajouter les TextAreas pour le contenu des notifications
- Alimenter le panel avec les notifications du `NotificationStore` (qui existe déjà dans l'actor)

---

## 3. Auto-Updater — NON BRANCHÉ

**Fichiers concernés:**
- `wmux-app/src/updater.rs` — Module complet avec check GitHub, download, apply

**Problème:** Le fichier commence par le commentaire explicite: `// Module is declared but not yet wired into the main application loop.` et `#![allow(dead_code)]`. Aucune fonction de `updater.rs` n'est appelée depuis `main.rs`. Le module `UpdateChecker` peut vérifier les mises à jour, télécharger et appliquer, mais rien n'est connecté.

**À faire:**
- Appeler `UpdateChecker::new()` au démarrage dans `main.rs`
- Spawner une tâche async pour `check_for_update()` périodiquement (ou au démarrage)
- Notifier l'utilisateur via le système de notifications ou la status bar quand une MAJ est dispo
- Gérer le flow download + apply (avec confirmation utilisateur)

---

## 4. System IPC Handler — NON ENREGISTRÉ

**Fichiers concernés:**
- `wmux-ipc/src/handlers/system.rs` — Handler fonctionnel (ping, capabilities, identify)
- `wmux-app/src/main.rs` lignes 42-65

**Problème:** Dans `main.rs`, le router enregistre `workspace`, `surface`, `browser`, et `sidebar`, mais **pas `system`**. Le `SystemHandler` est complètement implémenté et testé, mais les méthodes `system.ping`, `system.capabilities` et `system.identify` sont inaccessibles via IPC car le handler n'est pas enregistré.

**À faire:**
- Ajouter `router.register("system", std::sync::Arc::new(wmux_ipc::handlers::system::SystemHandler::new()));` dans `main.rs`

---

## 5. CLI Notify Commands — STUB

**Fichiers concernés:**
- `wmux-cli/src/commands/notify.rs`

**Problème:** Les commandes `wmux notify create`, `wmux notify list` et `wmux notify clear` existent dans le CLI, mais elles retournent toutes un message d'erreur hardcodé: `"not yet implemented (pending Task L3_08)"`. Aucun appel IPC n'est effectué.

**À faire:**
- Implémenter les appels IPC correspondants (`notify.create`, `notify.list`, `notify.clear`)
- Créer un handler IPC `NotifyHandler` côté serveur
- Connecter au `NotificationStore` de l'actor

---

## 6. CLI SSH Commands — STUB

**Fichiers concernés:**
- `wmux-cli/src/commands/ssh.rs`
- `wmux-core/src/remote.rs` — `RemoteConfig`, `ReconnectBackoff`, `RemoteConnectionState`

**Problème:** Les commandes `wmux ssh connect` et `wmux ssh disconnect` existent mais affichent juste: `"SSH remote connection to '...' is not yet fully implemented"`. Le module `remote.rs` dans wmux-core est entièrement implémenté (parsing SSH target, validation, backoff, ssh_args), mais le daemon Go (wmuxd-remote) et le tunnel SSH ne sont pas intégrés.

**À faire:**
- Intégrer le daemon Go wmuxd-remote
- Implémenter le tunnel SSH avec le remote workspace model
- Brancher les commandes CLI aux opérations réelles

---

## 7. Toggle Dev Tools (F12) — STUB

**Fichiers concernés:**
- `wmux-ui/src/window/handlers.rs` ligne 665-666

**Problème:** Le raccourci F12 est détecté comme `ShortcutAction::ToggleDevTools` mais le handler ne fait qu'un `tracing::debug!("ToggleDevTools shortcut (placeholder)")`. Aucune action n'est exécutée.

**À faire:**
- Définir ce que "dev tools" signifie pour wmux (inspecteur de debug? performance overlay? WebView2 devtools?)
- Implémenter l'action correspondante

---

## 8. Sidebar progress bar et activity log — DONNÉES NON RENDUES

**Fichiers concernés:**
- `wmux-core/src/metadata_store.rs` — `ProgressState`, `LogEntry`, `MetadataStore` complets
- `wmux-core/src/app_state/mod.rs` — Commandes `SidebarSetProgress`, `SidebarAddLog`, etc.
- `wmux-ipc/src/handlers/sidebar.rs` — Handlers IPC fonctionnels

**Problème:** Les données de progress bar et activity log sont correctement stockées dans `MetadataStore` et accessibles via IPC (`sidebar.set_progress`, `sidebar.log`, etc.), mais la sidebar UI (`sidebar.rs`) ne rend que les noms de workspace, les badges d'unread, et les status icons. Il n'y a **aucun rendu visuel** de la progress bar ni du log d'activité dans la sidebar.

**À faire:**
- Ajouter le rendu de la progress bar dans `SidebarState::render_quads()`
- Ajouter le rendu du log d'activité (derniers entries) dans la sidebar
- Récupérer les données de `MetadataSnapshot` dans le render loop

---

## 9. Certaines browser.* IPC methods — NON IMPLÉMENTÉES

**Fichiers concernés:**
- `wmux-ui/src/window/event_loop.rs` ligne 1888
- `wmux-browser/src/automation.rs`

**Problème:** Le handler browser supporte `open`, `navigate`, `back`, `forward`, `reload`, `url`, `eval`, `close`, et `identify`. Mais environ 25 méthodes listées dans `BROWSER_METHODS` (click, dblclick, hover, fill, type, press, select, check, scroll, snapshot, screenshot, get, is, find, highlight, wait, console, errors, cookies, storage, state, tab, addinitscript, open-split) retournent `"browser.{method} not yet implemented"`. Le module `automation.rs` dans wmux-browser a une implémentation partielle pour certaines de ces méthodes, mais la majorité ne sont pas branchées dans `handle_browser_command`.

**À faire:**
- Brancher les méthodes d'automation implémentées dans `automation.rs` au handler `handle_browser_command`
- Implémenter les méthodes manquantes dans `automation.rs`
- Ajouter screenshot via CapturePreview de WebView2

---

## 10. Port scanning — DONNÉES NON EXPLOITÉES VISUELLEMENT

**Fichiers concernés:**
- `wmux-core/src/port_scanner.rs` — Scanner netstat fonctionnel
- `wmux-core/src/app_state/actor.rs` — Scan périodique (15s) via `port_scan_interval`

**Problème:** L'actor scanne les ports en écoute toutes les 15 secondes et stocke le résultat dans `Workspace::ports` via `AppCommand::UpdatePorts`. Les données sont collectées, mais **jamais affichées** à l'utilisateur — ni dans la sidebar, ni dans la status bar, ni ailleurs dans l'UI.

**À faire:**
- Afficher les ports en écoute dans la sidebar (section metadata par workspace)
- Ou dans la status bar du workspace actif
- Ou dans un tooltip au hover d'un workspace

---

## 11. i18n — SYSTÈME CHARGÉ MAIS NON UTILISÉ

**Fichiers concernés:**
- `wmux-config/src/locale.rs` — Système i18n complet (en + fr)
- `resources/locales/en.toml`, `fr.toml`

**Problème:** Le système de localisation est entièrement implémenté avec détection de la langue système, chargement TOML, lookup par clé en dot-notation. Cependant, **aucune chaîne utilisateur dans l'UI n'utilise ce système**. Toutes les chaînes sont hardcodées en anglais: `"New Workspace"`, `"WORKSPACES"`, `"No matches"`, `"Split Right"`, etc. Les multiples `TODO(L2_16)` dans le code le confirment.

**À faire:**
- Instancier `Locale` au démarrage (depuis la config `language`)
- Passer la locale dans `UiState`
- Remplacer toutes les chaînes hardcodées par des appels `locale.t("key")`

---

## 12. Config keybindings — CHARGÉ MAIS NON UTILISÉ

**Fichiers concernés:**
- `wmux-config/src/config.rs` — Champ `keybindings: HashMap<String, String>`

**Problème:** Le config supporte un champ `keybindings` pour personnaliser les raccourcis, mais `ShortcutMap` utilise des bindings hardcodés. Le HashMap est chargé depuis le fichier config mais jamais lu par `ShortcutMap::match_shortcut()`.

**À faire:**
- Passer la config keybindings à `ShortcutMap`
- Implémenter le parsing des keybindings customisés
- Appliquer les overrides sur les bindings par défaut

---

## Résumé — Liste des tâches prioritaires

| # | Feature | Effort | Impact |
|---|---------|--------|--------|
| 1 | **Enregistrer SystemHandler dans le router IPC** | Trivial (1 ligne) | Les commandes system.ping/capabilities/identify deviennent accessibles |
| 2 | **Brancher le NotificationPanel** | Moyen | Le panel de notifications (déjà codé) devient visible et fonctionnel |
| 3 | **Brancher le CommandPalette** | Moyen-haut | La palette de commandes (déjà codée) devient visible et fonctionnelle |
| 4 | **Rendre la progress bar + logs sidebar** | Moyen | Les données déjà collectées via IPC deviennent visibles |
| 5 | **Afficher les ports scannés** | Faible | Les ports en écoute (déjà détectés) deviennent visibles |
| 6 | **Brancher les browser automation methods** | Moyen | Les 25+ méthodes déjà listées dans le handler deviennent fonctionnelles |
| 7 | **Brancher l'auto-updater** | Moyen | Les utilisateurs sont notifiés des nouvelles versions |
| 8 | **Brancher i18n dans l'UI** | Moyen-haut | L'application supporte le français et autres langues |
| 9 | **Implémenter les CLI notify commands** | Moyen | Les commandes notify create/list/clear fonctionnent |
| 10 | **Brancher les config keybindings** | Moyen | Les utilisateurs peuvent personnaliser les raccourcis |
| 11 | **Implémenter ToggleDevTools** | Faible | Le raccourci F12 fait quelque chose |
| 12 | **Implémenter SSH remote** | Haut | Les workspaces distants fonctionnent (dépend du daemon Go) |
