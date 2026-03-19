# Product Requirements Document: wmux

## Vision Produit

**Problème**
cmux est le multiplexeur de terminal de référence pour les développeurs utilisant des agents IA (Claude Code, Codex, OpenCode, Gemini CLI, Aider, Goose, Amp) sur macOS. Il offre une expérience complète : terminal GPU via libghostty, split panes avec surfaces (onglets par pane), sidebar verticale avec métadonnées temps réel (branche git, ports, statut agent, progress bars, logs), navigateur intégré scriptable (50+ commandes d'automatisation DOM), API socket/CLI pour orchestration programmatique, et un système de notifications riche (rings visuels, badges, panneau dédié, commandes personnalisées). Cependant, **cmux n'existe que sur macOS** (Swift + AppKit). Les développeurs Windows n'ont aucune alternative équivalente. Windows Terminal ne fait que des onglets basiques, WezTerm manque d'intégration IA/navigateur/sidebar, et tmux dans WSL n'est pas une expérience native.

**Solution**
wmux est un multiplexeur de terminal natif Windows, écrit en Rust, qui reproduit l'intégralité de l'expérience cmux sur Windows. Même hiérarchie conceptuelle (Window > Workspace > Pane > Surface > Panel), même protocole JSON-RPC v2, même API socket avec adaptations Windows (Named Pipes au lieu d'Unix sockets, ConPTY au lieu de PTY posix, WebView2 au lieu de WebKit, Toast Windows au lieu de NSUserNotification, wgpu/Direct3D 12 au lieu de Metal). L'objectif est qu'un agent IA compatible cmux fonctionne avec wmux avec un minimum d'adaptation (changement de socket path uniquement).

**Critères de succès**
- Un utilisateur de Claude Code sur Windows a la même expérience qu'un utilisateur cmux sur macOS
- Les agents IA compatibles cmux fonctionnent avec wmux sans modification de leur code (seul le transport change : Named Pipes au lieu d'Unix sockets)
- Interface agréable, fluide et facile d'utilisation, intégrée visuellement à Windows
- Chaque fonctionnalité est fonctionnelle et fiable
- Adoption par la communauté open-source (stars, forks, contributeurs)

---

## Modèle Conceptuel

wmux reprend exactement la hiérarchie de cmux à 5 niveaux :

```
Window (fenêtre native Win32)
 └── Workspace (entrée dans la sidebar — "onglet vertical")
      └── Pane (région splittée dans le workspace)
           └── Surface (onglet dans un pane — identifié par WMUX_SURFACE_ID)
                └── Panel (contenu : terminal ConPTY ou navigateur WebView2)
```

**Window** : Fenêtre native Win32 avec sa propre sidebar. Plusieurs fenêtres possibles.
**Workspace** : Unité d'organisation principale. Chaque workspace apparaît comme un onglet vertical dans la sidebar, affichant : branche git, répertoire de travail, ports en écoute, texte de notification, statut PR, badges, progress bars, et logs. Raccourcis : `Ctrl+N` (créer), `Ctrl+1-9` (naviguer).
**Pane** : Région obtenue par split horizontal/vertical au sein d'un workspace. Chaque pane contient une barre d'onglets pour ses surfaces. Raccourcis : `Ctrl+D` (split droite), `Ctrl+Shift+D` (split bas).
**Surface** : Onglet individuel dans un pane. Chaque surface est identifiée par `WMUX_SURFACE_ID` et peut être un terminal ou un navigateur. Raccourci : `Ctrl+T` (nouvelle surface dans le pane courant).
**Panel** : Le contenu effectif d'une surface — soit un terminal ConPTY (avec session shell), soit un navigateur WebView2 (avec URL et état).

Cette hiérarchie est exposée dans l'API IPC et les variables d'environnement, permettant aux agents IA de cibler précisément n'importe quel élément.

---

## Utilisateurs Cibles

### Persona Principal : Le Développeur IA sur Windows
- **Rôle** : Développeur utilisant quotidiennement des agents IA (Claude Code avec abonnement Max, Codex, OpenCode, Gemini CLI, Aider, Goose, Amp) sur Windows
- **Pain Points** :
  - Aucun équivalent de cmux sur Windows — doit jongler entre plusieurs fenêtres de terminal
  - Windows Terminal ne supporte ni split panes avancés, ni navigateur intégré, ni API pour agents IA, ni sidebar avec métadonnées
  - Utiliser tmux via WSL est un compromis insatisfaisant (pas natif, pas de navigateur intégré, friction de configuration)
  - Pas de notifications intelligentes quand un agent termine une tâche longue — les notifications Windows Terminal sont rudimentaires
  - Impossible de voir l'état de plusieurs agents en parallèle (pas de sidebar avec statut, progress bars, logs)
- **Motivations** : Avoir un environnement de développement unifié et optimisé pour le workflow IA multi-agents, sans quitter Windows
- **Objectif** : Lancer plusieurs instances de Claude Code en parallèle, voir leur statut dans la sidebar, prévisualiser dans le navigateur intégré, recevoir des notifications contextuelles, et contrôler le tout via CLI — dans une seule fenêtre

### Persona Secondaire : Le Power User Windows
- **Rôle** : Développeur ou administrateur système qui veut un multiplexeur terminal natif et moderne sur Windows
- **Pain Points** :
  - Les terminaux Windows existants sont soit basiques (Windows Terminal) soit complexes à configurer (WezTerm)
  - Pas de persistance de session fiable ni de workspaces organisés avec métadonnées visibles
  - Pas de navigateur intégré pour prévisualiser des apps web sans quitter le terminal
- **Motivations** : Productivité, organisation, esthétique native Windows
- **Objectif** : Remplacer son terminal actuel par un outil plus puissant et mieux intégré

---

## Fonctionnalités Core (MVP)

### Must-Have

#### 1. Terminal GPU-Acceleré
**Description** : Terminal complet avec rendu GPU via wgpu/Direct3D 12, parsing VTE (séquences d'échappement ANSI/VT100/VT220/xterm-256color), scrollback configurable (défaut 4K lignes), sélection texte, copier/coller, et gestion ConPTY. Supporte PowerShell 5/7, cmd.exe, bash (Git Bash/MSYS2), et les shells WSL (bash, zsh, fish). Rendu de glyphes via DirectWrite avec support ligatures, emojis, et Nerd Fonts.
**Valeur utilisateur** : Un terminal rapide, fluide et compatible avec tous les outils en ligne de commande Windows, avec un rendu typographique de qualité.
**Métrique de succès** : Latence input-to-display < 16ms (60fps), rendu correct de toutes les séquences ANSI/VT courantes, pas de glitch visuel, affichage correct des emojis et Nerd Fonts.

#### 2. Multiplexeur (Split Panes + Workspaces + Surfaces)
**Description** : Système de split panes (horizontal/vertical) avec arbre binaire, dividers draggables, zoom de pane (`Ctrl+Shift+Enter`), et focus routing clavier (`Alt+Ctrl+Flèches`). Chaque pane contient une ou plusieurs **surfaces** (onglets), navigables via `Ctrl+Tab`. Workspaces multiples avec sidebar verticale, navigation clavier (`Ctrl+1-9`), renommage, drag-and-drop pour réorganiser, badges de notification, et indicateurs d'état temps réel. Support de `swap-pane` pour échanger deux panes, et `break-pane`/`join-pane` pour déplacer des surfaces entre workspaces.
**Valeur utilisateur** : Organiser plusieurs terminaux et contextes de travail dans une seule fenêtre, comme tmux/cmux mais en natif Windows et visuel.
**Métrique de succès** : Création/navigation entre panes, surfaces et workspaces sans lag perceptible, layout stable après redimensionnement de fenêtre, surfaces correctement isolées.

#### 3. CLI & API IPC (Compatibilité Agents IA)
**Description** : Serveur Named Pipes avec protocole JSON-RPC v2 compatible cmux. CLI `wmux.exe` avec commandes couvrant l'intégralité de l'API. Authentification HMAC-SHA256. Variables d'environnement pour découverte automatique. Compatible Claude Code, Codex, OpenCode, Gemini CLI, Aider, Goose, Amp, et tout agent CLI.

**Chemins Named Pipe** :
- Release : `\\.\pipe\wmux`
- Debug : `\\.\pipe\wmux-debug`
- Custom : via `WMUX_SOCKET_PATH`

**Modes d'accès** (comme cmux) :
| Mode | Comportement | Activation |
|------|-------------|------------|
| Off | Pipe désactivé | `WMUX_SOCKET_MODE=off` |
| wmux-only | Seuls les processus lancés par wmux (défaut) | Settings UI |
| allowAll | Tous les processus locaux | `WMUX_SOCKET_MODE=allowAll` |

**Variables d'environnement** (injectées dans chaque surface) :
- `WMUX_SOCKET_PATH` : Chemin du Named Pipe
- `WMUX_WORKSPACE_ID` : ID du workspace courant
- `WMUX_SURFACE_ID` : ID de la surface courante
- `WMUX_WINDOW_ID` : ID de la fenêtre courante
- `TERM_PROGRAM` : `wmux`
- `TERM` : `xterm-256color`

**Méthodes API principales** (protocole JSON-RPC v2, une requête = un objet JSON terminé par newline) :

| Catégorie | Méthodes |
|-----------|----------|
| Workspace | `workspace.list`, `workspace.create`, `workspace.select`, `workspace.current`, `workspace.close`, `workspace.rename` |
| Surface | `surface.split`, `surface.list`, `surface.focus`, `surface.close`, `surface.read_text` |
| Input | `surface.send_text`, `surface.send_key` |
| Notification | `notification.create`, `notification.list`, `notification.clear` |
| Sidebar | `sidebar.set_status`, `sidebar.clear_status`, `sidebar.list_status`, `sidebar.set_progress`, `sidebar.clear_progress`, `sidebar.log`, `sidebar.clear_log`, `sidebar.list_log`, `sidebar.state` |
| Browser | `browser.open`, `browser.navigate`, `browser.click`, `browser.fill`, `browser.eval`, `browser.snapshot`, `browser.screenshot`, ... (50+ sous-commandes) |
| System | `system.ping`, `system.capabilities`, `system.identify` |

**Options CLI globales** :
- `--pipe PATH` : Chemin Named Pipe custom
- `--json` : Sortie JSON
- `--window ID` : Cibler une fenêtre
- `--workspace ID` : Cibler un workspace
- `--surface ID` : Cibler une surface

**Valeur utilisateur** : Les agents IA peuvent contrôler wmux programmatiquement — créer des panes, envoyer du texte, lire le contenu du terminal, ouvrir le navigateur, afficher des statuts dans la sidebar, recevoir des notifications.
**Métrique de succès** : Claude Code (abonnement Max) peut utiliser wmux comme terminal avec les mêmes capacités qu'avec cmux. Un script écrit pour l'API cmux fonctionne avec wmux en changeant uniquement le transport (Named Pipe au lieu d'Unix socket).

#### 4. Navigateur Intégré (WebView2)
**Description** : Navigateur Chromium intégré dans les surfaces (panes) via WebView2, avec une API d'automatisation complète couvrant 8 catégories de commandes :

| Catégorie | Commandes |
|-----------|-----------|
| Navigation | `open`, `open-split`, `navigate`, `back`, `forward`, `reload`, `url`, `focus-webview`, `is-webview-focused`, `identify` |
| Attente | `wait` (sélecteurs, texte, URL, état de chargement, condition JS) |
| Interaction DOM | `click`, `dblclick`, `hover`, `focus`, `check`, `uncheck`, `scroll-into-view`, `type`, `fill`, `press`, `keydown`, `keyup`, `select`, `scroll` |
| Inspection | `snapshot` (arbre d'accessibilité), `screenshot`, `get`, `is`, `find`, `highlight` |
| JavaScript | `eval`, `addinitscript`, `addscript`, `addstyle` |
| Frames/Dialogs | `frame`, `dialog`, `download` |
| État/Session | `cookies`, `storage`, `state` |
| Onglets/Logs | `tab`, `console`, `errors` |

Le navigateur est ciblable via surface ID, ce qui permet aux agents IA de contrôler des surfaces navigateur spécifiques. DevTools accessible via `F12`. L'arbre d'accessibilité (`snapshot`) permet aux agents de comprendre la structure de la page pour interagir intelligemment.

**Valeur utilisateur** : Les agents IA peuvent ouvrir une page web, attendre qu'elle charge, interagir avec le DOM (cliquer, remplir des formulaires, exécuter du JavaScript), prendre des screenshots, et lire les logs console — le tout programmatiquement via l'API, sans quitter wmux.
**Métrique de succès** : Ouverture d'un navigateur dans une surface en < 1s. JavaScript eval retourne la valeur (pas juste "OK"). Snapshot de l'arbre d'accessibilité fonctionnel. DevTools accessible. Cookie/storage management fonctionnel.

#### 5. Sidebar Metadata System
**Description** : Système de métadonnées programmable dans la sidebar, permettant aux agents IA et scripts d'afficher des informations en temps réel pour chaque workspace. Trois types de métadonnées :

**Statuts** (badges clé-valeur avec icône et couleur) :
```bash
wmux sidebar set-status <key> <value> --icon=<icon> --color=<color> --workspace=<id>
wmux sidebar clear-status <key> --workspace=<id>
wmux sidebar list-status --workspace=<id>
```
Exemples d'usage : agent "En attente d'input" (icône 🔵, couleur bleue), build "Build OK" (icône ✅, couleur verte), "3 erreurs" (icône ❌, couleur rouge).

**Progress bars** (0.0 à 1.0 avec label) :
```bash
wmux sidebar set-progress 0.75 --label="Build 75%" --workspace=<id>
wmux sidebar clear-progress --workspace=<id>
```
Exemples d'usage : progression d'un build, téléchargement, migration de données.

**Logs** (entrées horodatées avec niveau et source) :
```bash
wmux sidebar log --level=info|progress|success|warning|error --source=<name> --workspace=<id> -- "Message"
wmux sidebar clear-log --workspace=<id>
wmux sidebar list-log --limit=<n> --workspace=<id>
```
Exemples d'usage : "Claude Code: Fichier créé src/main.rs", "Build: Compilation réussie en 3.2s".

**État complet** :
```bash
wmux sidebar state --workspace=<id>
```
Retourne toutes les métadonnées : cwd, branche git, ports, statuts, progress, logs.

**Intégration Claude Code** : Tracking lifecycle PID-aware — détection automatique quand un processus agent se termine, nettoyage des statuts "Needs input" obsolètes via un timer de balayage (30s).

**Valeur utilisateur** : Voir en un coup d'oeil l'état de tous les agents et processus en cours dans chaque workspace, sans avoir à naviguer dans chaque pane. Un "tableau de bord" intégré à la sidebar.
**Métrique de succès** : Les statuts, progress bars et logs sont mis à jour en < 500ms après un appel API. Le nettoyage automatique des processus terminés fonctionne de manière fiable.

#### 6. Lecture du Terminal (Read Screen)
**Description** : Capacité de lire programmatiquement le contenu visible et le scrollback d'une surface terminal via l'API. Équivalent de `capture-pane` dans tmux / `surface.read_text` dans cmux.
```bash
wmux surface read-text --surface=<id>
wmux surface read-text --surface=<id> --start=-100  # 100 dernières lignes
```
**Valeur utilisateur** : Les agents IA peuvent lire ce qui s'affiche dans un terminal — vérifier la sortie d'une commande, détecter des erreurs, monitorer un processus — sans intervention humaine.
**Métrique de succès** : Lecture du contenu d'une surface terminal de 4K lignes en < 100ms. Le texte retourné correspond exactement à ce qui est affiché.

#### 7. Notifications
**Description** : Système de notifications riche avec cycle de vie complet et intégration Windows native.

**Cycle de vie** (comme cmux) :
1. **Received** — Apparaît dans le panneau ; alerte desktop déclenchée (sauf suppression)
2. **Unread** — Badge affiché sur le workspace dans la sidebar
3. **Read** — Effacé quand le workspace est consulté
4. **Cleared** — Supprimé du panneau

**Règles de suppression** (les alertes desktop sont supprimées quand) :
- La fenêtre wmux est active ET le workspace émetteur est actif
- Le panneau de notifications est ouvert

**Sources de notifications** :
- Séquences terminales OSC 9 (basique), OSC 99 (Kitty — riche avec sous-titres et IDs), OSC 777 (RXVT — simple titre/corps)
- CLI : `wmux notify --title "Titre" --subtitle "Sous-titre" --body "Corps"`
- API : `notification.create` via JSON-RPC
- Hooks Claude Code : événements "Stop" et "PostToolUse"

**Indicateurs visuels** :
- Rings bleus lumineux autour des panes nécessitant attention
- Badges de notification sur les workspaces dans la sidebar
- Éclairage des onglets sidebar
- Popover temporaire

**Panneau de notifications** (`Ctrl+Shift+I`) :
- Historique complet des notifications
- Clic sur une notification → navigation vers le workspace source
- `Ctrl+Shift+U` → Saut vers la notification non-lue la plus récente

**Commande personnalisée sur notification** :
Exécuter une commande shell à chaque notification (configurable dans Settings). Variables d'environnement disponibles :
- `WMUX_NOTIFICATION_TITLE`
- `WMUX_NOTIFICATION_SUBTITLE`
- `WMUX_NOTIFICATION_BODY`

**Sons** :
- Sélecteur de son système Windows (sons .wav)
- Fichier son personnalisé via `powershell -c (New-Object Media.SoundPlayer 'chemin.wav').PlaySync()`
- Text-to-speech via Win32 SAPI (`Add-Type -AssemblyName System.Speech`)
- Sélecteur "None" pour commande personnalisée uniquement

**Intégration Windows Toast** :
- Notifications Toast natives Windows 10/11 via WinRT (`Windows.UI.Notifications`)
- Support des actions dans les Toast (boutons "Voir", "Ignorer")
- Icône wmux dans les notifications

**Valeur utilisateur** : Être informé quand un agent termine une tâche longue, avec des indicateurs visuels riches dans l'interface ET des notifications système Windows, sans surveiller constamment chaque terminal.
**Métrique de succès** : Notification reçue dans les 2s après un événement. Toast Windows affiché correctement. Panneau de notifications fonctionnel avec navigation. Sons personnalisables.

#### 8. Persistance de Session
**Description** : Sauvegarde automatique (toutes les 8 secondes) et restauration au lancement.

**Ce qui est restauré** :
- Disposition des fenêtres (position, taille)
- Workspaces (ordre, noms)
- Layout des panes et surfaces
- Répertoires de travail de chaque surface
- Historique terminal / scrollback (best-effort, jusqu'à 4K lignes)
- URLs et historique de navigation des surfaces navigateur
- Métadonnées sidebar (statuts, logs)

**Ce qui n'est PAS restauré (v1)** :
- Processus actifs (Claude Code, vim, ssh) — les surfaces redémarrent un shell frais dans le bon répertoire
- État interne des applications (tmux imbriqué, sessions vim)
- Connexions réseau / tunnels SSH

**Valeur utilisateur** : Ne jamais perdre son environnement de travail après un redémarrage ou un crash. Retrouver ses workspaces, layouts et scrollback immédiatement.
**Métrique de succès** : Après fermeture et réouverture, le layout est restauré fidèlement (workspaces, panes, répertoires, scrollback). Temps de restauration < 3s pour 10 workspaces.

#### 9. Support SSH Remote
**Description** : Commande `wmux ssh` pour créer des workspaces distants durables. Daemon Go (`wmuxd-remote`) provisionné automatiquement sur la machine distante. Reconnexion automatique avec exponential backoff. Proxy navigateur via SOCKS5/HTTP CONNECT. Relay CLI via reverse TCP pour contrôler wmux depuis le remote.

**Workflow** :
1. `wmux ssh user@host` → Connexion SSH, provisionnement de `wmuxd-remote` si absent
2. Workspace distant créé dans la sidebar (icône SSH distincte)
3. Panes et surfaces fonctionnent comme en local
4. Déconnexion réseau → Reconnexion automatique, session préservée côté remote
5. `wmux ssh disconnect` → Fermeture propre

**Valeur utilisateur** : Travailler sur des machines distantes avec la même expérience qu'en local, sans perdre la session en cas de déconnexion.
**Métrique de succès** : Reconnexion SSH automatique après coupure réseau (< 10s avec backoff). Session préservée côté remote. Latence interactive < 100ms sur une connexion à 50ms RTT.

#### 10. Thèmes & Configuration
**Description** : Compatibilité avec les fichiers de configuration Ghostty (~50+ thèmes). Commande interactive `wmux themes` pour lister, prévisualiser et appliquer des thèmes avec surcharge persistante.

**Sources de configuration** (par priorité décroissante) :
1. `%APPDATA%\wmux\config` (configuration wmux spécifique)
2. `%APPDATA%\ghostty\config` (import Ghostty existant)
3. Défauts intégrés

**Personnalisation** :
- Thèmes terminal (couleurs, palette)
- Thèmes sidebar (couleurs, opacité, largeur)
- Polices via DirectWrite (support ligatures, Nerd Fonts, fallback chain)
- Détection automatique dark/light mode système Windows
- Opacité de fond (transparency)
- Taille de scrollback

**CLI thèmes** :
```bash
wmux themes list              # Lister les thèmes disponibles
wmux themes set <name>        # Appliquer un thème (persistant)
wmux themes clear             # Revenir au thème par défaut
```

**Valeur utilisateur** : Personnaliser l'apparence du terminal selon ses préférences, réutiliser ses thèmes Ghostty existants, changer de thème sans redémarrer.
**Métrique de succès** : Un fichier config Ghostty existant est importé et appliqué correctement. Changement de thème en live sans redémarrage.

#### 11. Palette de Commandes
**Description** : Overlay (`Ctrl+Shift+P`) avec recherche fuzzy sur toutes les actions disponibles (commandes, raccourcis, workspaces, surfaces). Navigation entièrement au clavier. Recherche cross-surfaces (`Ctrl+P` pour chercher dans toutes les surfaces).
**Valeur utilisateur** : Accéder rapidement à n'importe quelle fonctionnalité sans mémoriser tous les raccourcis.
**Métrique de succès** : Recherche et exécution d'une commande en < 3 interactions clavier. Résultats affichés en < 100ms.

#### 12. Recherche Terminal
**Description** : Overlay de recherche (`Ctrl+F`) avec navigation vi-style (`n`/`N`). Highlight des résultats dans le terminal. Support regex optionnel.
**Valeur utilisateur** : Retrouver rapidement une sortie passée dans le scrollback.
**Métrique de succès** : Recherche dans un scrollback de 4K lignes en < 100ms.

#### 13. Shell Integration & Détection Git
**Description** : Hooks automatiques pour PowerShell 5/7, bash (Git Bash), zsh, et fish. La sidebar affiche pour chaque workspace :
- Branche git courante et statut (clean/dirty)
- Répertoire de travail
- Ports en écoute (détection automatique via netstat/ss)
- Statut PR (si détectable via `gh`)
- Texte de la dernière notification

**Variables d'environnement** injectées (en plus de celles de l'API) :
- `WMUX_WORKSPACE_ID` : ID du workspace
- `WMUX_SURFACE_ID` : ID de la surface

Les hooks détectent les changements de répertoire (`cd`) et mettent à jour la sidebar en temps réel.

**Valeur utilisateur** : Contexte visible en permanence dans la sidebar sans taper de commandes git status ni surveiller les ports.
**Métrique de succès** : Branche git détectée et affichée dans les 2s après navigation dans un repo. Ports en écoute détectés dans les 5s.

#### 14. Auto-Update
**Description** : Vérification automatique via GitHub Releases API. Téléchargement en arrière-plan. Installation staged (téléchargé mais appliqué au prochain lancement). Notification de mise à jour dans la barre de titre (pill/badge). Possibilité de vérifier manuellement via menu ou `wmux update check`.
**Valeur utilisateur** : Toujours avoir la dernière version sans action manuelle.
**Métrique de succès** : Détection d'une nouvelle version dans l'heure suivant sa publication. Mise à jour appliquée au lancement suivant sans perte de données.

#### 15. Effets Visuels Windows 11
**Description** : Backdrop Mica/Acrylic pour la sidebar via DWM API (`DwmSetWindowAttribute`). Coins arrondis natifs. Intégration thème dark/light système via `UISettings`. Fallback gracieux (fond opaque avec couleur de thème) sur Windows 10.
**Valeur utilisateur** : Interface qui s'intègre visuellement avec le reste de Windows 11, tout en restant fonctionnelle et esthétique sur Windows 10.
**Métrique de succès** : Effets Mica/Acrylic visibles sur Windows 11. Application fonctionnelle et esthétique sur Windows 10 (fond opaque). Transition dark/light mode instantanée.

#### 16. Localisation FR/EN
**Description** : Interface disponible en français et en anglais. Détection automatique de la langue système via Win32 API (`GetUserDefaultUILanguage`) avec possibilité de changement manuel dans les paramètres.
**Valeur utilisateur** : Utiliser l'application dans sa langue préférée.
**Métrique de succès** : 100% des textes de l'interface traduits dans les deux langues. Changement de langue sans redémarrage.

---

## Raccourcis Clavier (Mapping macOS → Windows)

Correspondance complète des raccourcis cmux (Cmd-based) vers wmux (Ctrl-based) :

| Action | cmux (macOS) | wmux (Windows) |
|--------|-------------|----------------|
| Nouveau workspace | `Cmd+N` | `Ctrl+N` |
| Naviguer workspace 1-9 | `Cmd+1-9` | `Ctrl+1-9` |
| Nouvelle surface (onglet) | `Cmd+T` | `Ctrl+T` |
| Split droite | `Cmd+D` | `Ctrl+D` |
| Split bas | `Cmd+Shift+D` | `Ctrl+Shift+D` |
| Naviguer entre panes | `Alt+Cmd+Flèches` | `Alt+Ctrl+Flèches` |
| Zoom pane | `Cmd+Shift+Enter` | `Ctrl+Shift+Enter` |
| Fermer surface | `Cmd+W` | `Ctrl+W` |
| Panneau notifications | `Cmd+Shift+I` | `Ctrl+Shift+I` |
| Notification non-lue suivante | `Cmd+Shift+U` | `Ctrl+Shift+U` |
| Palette de commandes | `Cmd+Shift+P` | `Ctrl+Shift+P` |
| Recherche terminal | `Cmd+F` | `Ctrl+F` |
| Recherche cross-surfaces | `Cmd+P` | `Ctrl+P` |
| Copier | `Cmd+C` | `Ctrl+Shift+C` |
| Coller | `Cmd+V` | `Ctrl+Shift+V` |
| DevTools navigateur | `Cmd+Alt+I` | `F12` |

Tous les raccourcis sont personnalisables dans la configuration.

---

## Adaptations macOS → Windows

| Composant cmux (macOS) | Équivalent wmux (Windows) | Notes |
|------------------------|--------------------------|-------|
| Swift + AppKit | Rust + Win32 API | Performance native, pas d'Electron |
| libghostty (Metal) | wgpu (Direct3D 12) + VTE crate | Rendu GPU custom, parsing VTE séparé |
| Unix sockets (`/tmp/cmux.sock`) | Named Pipes (`\\.\pipe\wmux`) | Même protocole JSON-RPC v2 |
| posix PTY | ConPTY | API native Windows depuis 1809 |
| WebKit (intégré macOS) | WebView2 (Chromium/Edge) | Runtime distribué avec Windows, installable séparément |
| NSUserNotification | Windows Toast (WinRT) | Support actions dans les Toast |
| Sparkle (auto-update) | GitHub Releases API + custom updater | Vérification automatique, install staged |
| `afplay` (sons) | Win32 `PlaySound` / PowerShell `SoundPlayer` | Fichiers .wav |
| `say` (TTS) | Win32 SAPI / `System.Speech` | Text-to-speech natif Windows |
| DMG + Homebrew | MSI/EXE + winget + Scoop | Distribution multi-canal |
| Ghostty config `~/.config/ghostty/` | `%APPDATA%\ghostty\` + `%APPDATA%\wmux\` | Import Ghostty, config propre wmux |
| Mica/Acrylic (natif macOS) | DWM API Mica/Acrylic (Win11 only) | Fallback fond opaque sur Win10 |
| `TERM=xterm-ghostty` | `TERM=xterm-256color` | Compatibilité maximale Windows |
| `TERM_PROGRAM=ghostty` | `TERM_PROGRAM=wmux` | Identification du terminal |
| Cmd+key | Ctrl+key | Standard Windows |

---

## Parcours Utilisateurs

### Workflow 1 : Développeur IA avec Claude Code (multi-agents)
1. **Lancer wmux** → L'application restaure la session précédente (workspaces, panes, scrollback, URLs navigateur)
2. **Créer un workspace "projet-x"** → Nouveau workspace dans la sidebar avec terminal PowerShell, branche git affichée
3. **Splitter le pane** → Terminal à gauche, navigateur intégré à droite (`wmux browser open-split http://localhost:3000`)
4. **Lancer Claude Code** → L'agent détecte wmux via `WMUX_SOCKET_PATH` et utilise l'API IPC
5. **Claude Code travaille** → Crée des panes additionnels, envoie du texte, ouvre des URLs, affiche son statut dans la sidebar via `sidebar.set_status` et sa progression via `sidebar.set_progress`
6. **Agents en parallèle** → Créer un 2e workspace pour un 2e agent Claude Code. La sidebar montre le statut de chaque agent indépendamment
7. **Notification** → wmux affiche un ring bleu sur le pane de l'agent qui a terminé, un badge sur le workspace, et un Toast Windows. La sidebar montre "Needs input"
8. **`Ctrl+Shift+U`** → Navigation directe vers l'agent nécessitant attention
9. **Vérifier les résultats** → Lire les logs sidebar, naviguer entre les panes pour voir le code, les tests. L'agent utilise `browser.snapshot` + `browser.click` pour interagir avec la preview web
10. **Fermer wmux** → L'état est sauvegardé automatiquement pour la prochaine session

### Workflow 2 : Power User multi-projets
1. **Lancer wmux** → Session restaurée avec 3 workspaces (frontend, backend, infra)
2. **Workspace "frontend"** → Terminal + navigateur côte à côte, branche `feature/auth` visible dans la sidebar
3. **Workspace "backend"** → 3 panes : éditeur, logs server, tests. Ports 3000 et 5432 visibles dans la sidebar
4. **Workspace "infra"** → SSH vers un serveur distant via `wmux ssh`, icône SSH dans la sidebar
5. **Ctrl+Shift+P** → Palette de commandes pour naviguer rapidement entre les contextes
6. **Ctrl+F** → Recherche dans le scrollback pour retrouver une erreur passée
7. **Thèmes** → `wmux themes set catppuccin-mocha` pour changer le thème en live

---

## Hors Scope (v1)

Explicitement NON inclus dans le MVP :
- **Support macOS/Linux** : wmux v1 est exclusivement Windows
- **Système de plugins/extensions** : Pas d'API d'extension tierce dans la v1
- **Marketplace de thèmes** : Les thèmes sont chargés localement uniquement
- **Telemetry/Analytics** : Pas de collecte de données dans la v1
- **Support IME complet CJK** : Support basique via winit, améliorations en v2
- **Accessibilité screen reader** : Support basique, amélioration incrémentale en v2 via Win32 UI Automation
- **Distribution via Microsoft Store** : Distribution via GitHub Releases, winget, et Scoop uniquement
- **Application mobile/tablette** : Desktop uniquement
- **Restauration de processus actifs** : Les surfaces restaurent le shell dans le bon répertoire, pas les processus en cours (Claude Code, vim, etc.)
- **`pipe-pane`** : Streaming de la sortie pane vers une commande shell (post-MVP)
- **`wait-for`** : Primitive de synchronisation pour scripts (post-MVP)
- **`set-hook`** : Automatisation event-driven côté serveur (post-MVP)

---

### Should-Have (Post-MVP)

- **Support Linux** : Port natif pour les distributions Linux populaires
- **Marketplace de thèmes** : Téléchargement et partage de thèmes communautaires
- **Extensions/plugins** : Système de plugins pour étendre les fonctionnalités
- **Telemetry opt-in** : Crash reporting (Sentry) et analytics (PostHog) optionnels
- **Accessibilité** : Support complet screen readers via Win32 UI Automation
- **IME avancé** : Support complet CJK (chinois, japonais, coréen)
- **Localisations supplémentaires** : Autres langues au-delà de FR/EN
- **PowerShell/COM automation** : Scripting externe (équivalent AppleScript)
- **pipe-pane** : Streaming de sortie terminal vers un processus externe
- **wait-for** : Synchronisation de scripts via signaux nommés
- **set-hook** : Exécuter des commandes sur événements wmux (workspace créé, pane fermé, etc.)
- **popup** : Panneau overlay flottant pour affichage temporaire

---

## Décisions Prises

- **Compatibilité Windows 10** : Oui, dès le MVP. Windows 10 1809+ est le minimum requis (ConPTY). Les effets Mica/Acrylic sont activés sur Windows 11 avec fallback gracieux (fond opaque) sur Windows 10.
- **Protocole cmux** : Compatible à ~95%. Mêmes noms de méthodes, même structure JSON-RPC v2, mais avec adaptations Windows (Named Pipes au lieu d'Unix sockets, chemins Windows). Les agents IA compatibles cmux doivent pouvoir communiquer avec wmux en changeant uniquement le transport.
- **Hiérarchie conceptuelle** : Identique à cmux (Window > Workspace > Pane > Surface > Panel). Les mêmes termes, les mêmes IDs, la même logique de ciblage.
- **Navigateur** : WebView2 dans un HWND enfant séparé (pas dans la surface wgpu). Permet l'accès aux DevTools et à l'API d'automatisation complète.
- **Rendu terminal** : wgpu custom (pas iced/egui pour la grille terminale). Contrôle total du rendu pour atteindre < 16ms de latence.
- **Sidebar Metadata** : API complète dès le MVP (statuts, progress, logs) — c'est un différenciateur clé pour les workflows IA multi-agents.
- **Licence** : MIT — maximise l'adoption et la contribution, aligné avec l'écosystème Rust (WezTerm, Alacritty, Rio).
- **Nom de package** : `wmux` sur winget et Scoop. Fallback : `wmux-terminal` si le nom est déjà pris. Réserver dès la première release alpha.
- **Documentation** : README GitHub uniquement pour le MVP. Pas de site web dédié dans la v1.

---

## Métriques de Succès

**Métriques Primaires** :
- **Parité fonctionnelle** : ≥90% des fonctionnalités cmux reproduites et fonctionnelles
- **Compatibilité Claude Code** : Claude Code (Max) fonctionne dans wmux avec les mêmes capacités qu'avec cmux
- **Compatibilité API** : Un script utilisant l'API cmux fonctionne avec wmux en changeant uniquement le transport
- **Stabilité** : < 1 crash par semaine d'utilisation intensive
- **Performance** : Latence input-to-display < 16ms (60fps)

**Métriques Secondaires** :
- **Adoption open-source** : 100+ stars GitHub dans les 3 premiers mois
- **Satisfaction utilisateur** : Feedback positif de la communauté (issues, discussions)
- **Temps d'installation** : < 5 minutes de l'installation au premier terminal fonctionnel
- **Sidebar metadata** : Temps de mise à jour < 500ms pour statuts/progress/logs

---

## Timeline & Jalons

- **Phase 1 — Terminal fonctionnel** : Semaines 1-8 — Terminal single-pane avec rendu GPU wgpu, ConPTY, PowerShell, VTE parsing, scrollback, sélection texte, copier/coller
- **Phase 2 — Multiplexeur** : Semaines 5-12 — Split panes avec arbre binaire, surfaces (onglets par pane), workspaces avec sidebar verticale, shell integration, détection git
- **Phase 3 — IPC & CLI** : Semaines 9-14 — Named Pipes, JSON-RPC v2, CLI wmux.exe, sidebar metadata (statuts/progress/logs), read-screen, modes d'accès, compatibilité agents IA
- **Phase 4 — Fonctionnalités avancées** : Semaines 11-20 — Navigateur WebView2 avec API d'automatisation complète, notifications (cycle de vie complet, Toast Windows, sons, commande personnalisée), persistance de session, thèmes Ghostty, SSH remote
- **Phase 5 — Polish** : Semaines 17-24 — Palette de commandes, recherche terminal, effets visuels Win11, auto-update, packaging (winget/Scoop), localisation FR/EN, documentation
- **MVP Complet** : ~6 mois (potentiellement plus rapide avec développement assisté par Claude Code)
- **Première release publique** : Dès que le MVP est stable et testé
