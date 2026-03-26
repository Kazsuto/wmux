# Brief Fonctionnel — wmux (Windows Terminal Multiplexer)

## 1. Contexte

Les développeurs utilisant des agents IA (Claude Code, Codex, OpenCode, Gemini CLI, Aider, Goose, Amp) sur macOS disposent de cmux, un multiplexeur de terminal avancé offrant : terminal GPU, split panes, sidebar avec métadonnées temps réel, navigateur intégré scriptable, API programmatique et notifications riches. **Aucune alternative équivalente n'existe sur Windows.** Windows Terminal ne propose que des onglets basiques, WezTerm manque d'intégration IA/navigateur/sidebar, et tmux via WSL n'est pas une expérience native.

Les développeurs Windows travaillant avec des agents IA sont contraints de jongler entre plusieurs fenêtres de terminal, n'ont pas de visibilité centralisée sur l'état de leurs agents, ne peuvent pas prévisualiser de pages web sans quitter le terminal, et ne reçoivent pas de notifications pertinentes quand un agent termine une tâche longue.

**wmux** est un multiplexeur de terminal natif Windows, open-source (MIT), qui vise à reproduire l'intégralité de l'expérience cmux sur Windows, avec compatibilité protocole pour que les agents IA existants fonctionnent sans modification.

## 2. Objectifs du projet

- Fournir aux développeurs Windows une expérience de multiplexeur terminal équivalente à cmux sur macOS
- Permettre aux agents IA compatibles cmux de fonctionner avec wmux sans modification de leur code (seul le transport change)
- Atteindre une parité fonctionnelle ≥ 90 % avec cmux
- Offrir une interface fluide et visuellement intégrée à Windows (10 et 11)
- Garantir une stabilité de production (< 1 crash par semaine d'utilisation intensive)
- Maintenir une latence d'affichage inférieure à 16 ms (60 fps)
- Obtenir une adoption communautaire open-source (100+ stars GitHub en 3 mois)

## 3. Acteurs et rôles

| Acteur | Rôle | Besoins principaux |
|--------|------|--------------------|
| Développeur IA sur Windows | Utilisateur principal — utilise quotidiennement des agents IA multi-instances | Voir l'état de tous ses agents en parallèle dans une sidebar, contrôler le terminal via API/CLI, prévisualiser dans un navigateur intégré, recevoir des notifications contextuelles |
| Power User Windows | Utilisateur secondaire — développeur ou admin système cherchant un multiplexeur moderne | Organiser plusieurs contextes de travail (workspaces), personnaliser l'apparence, persister ses sessions, accéder à un SSH distant intégré |
| Agent IA (Claude Code, Codex, etc.) | Acteur programmatique — interagit avec wmux via API | Découvrir wmux automatiquement (variables d'environnement), créer/gérer des panes et workspaces, lire le contenu du terminal, ouvrir et automatiser un navigateur, afficher des statuts et des barres de progression |
| Contributeur open-source | Développeur tiers souhaitant contribuer au projet | Comprendre l'architecture modulaire, accéder à une documentation claire, trouver des issues sur lesquelles contribuer |

## 4. Fonctionnalités attendues

### 4.1 Terminal

**Essentielles**

- Le système doit permettre d'afficher un terminal avec rendu GPU fluide (60 fps) supportant les séquences d'échappement ANSI/VT100/VT220/xterm-256color
- Le système doit permettre de lancer un shell natif Windows (PowerShell 5/7, cmd.exe, bash via Git Bash/MSYS2, shells WSL)
- Le système doit permettre de faire défiler un historique de commandes (scrollback configurable, par défaut 4 000 lignes)
- Le système doit permettre de sélectionner du texte à la souris et de copier/coller via raccourcis clavier
- Le système doit permettre d'afficher correctement les emojis, ligatures et icônes Nerd Fonts
- Le système doit permettre de rechercher du texte dans l'historique du terminal via un overlay de recherche dédié
- Le système doit permettre de lire programmatiquement le contenu visible et le scrollback d'un terminal (équivalent de capture-pane)

**Souhaitées**

- Il serait utile que le système permette la recherche avec expressions régulières dans le scrollback

### 4.2 Multiplexeur (split panes, workspaces, surfaces)

**Essentielles**

- Le système doit permettre de diviser un pane horizontalement ou verticalement pour créer des sous-régions indépendantes
- Le système doit permettre de redimensionner les panes en déplaçant les séparateurs à la souris
- Le système doit permettre de zoomer un pane pour qu'il occupe toute la zone de travail (et dé-zoomer pour revenir au layout)
- Le système doit permettre de naviguer entre les panes avec des raccourcis clavier directionnels
- Le système doit permettre de créer plusieurs workspaces, chacun avec son propre layout de panes
- Le système doit permettre de naviguer entre les workspaces via raccourcis clavier (1 à 9) ou via la sidebar
- Le système doit permettre de renommer un workspace
- Le système doit permettre de créer plusieurs surfaces (onglets) au sein d'un même pane
- Le système doit permettre d'échanger deux panes entre eux (swap-pane)
- Le système doit permettre de déplacer une surface d'un workspace à un autre (break-pane / join-pane)

**Souhaitées**

- Il serait utile que le système permette de réorganiser les workspaces dans la sidebar par glisser-déposer

### 4.3 Sidebar et métadonnées

**Essentielles**

- Le système doit permettre d'afficher une sidebar verticale listant tous les workspaces avec leurs métadonnées
- Le système doit permettre d'afficher la branche git courante et le statut (clean/dirty) pour chaque workspace
- Le système doit permettre d'afficher le répertoire de travail courant pour chaque workspace
- Le système doit permettre d'afficher des statuts programmables (clé-valeur avec icône et couleur) par workspace, définis via API
- Le système doit permettre d'afficher une barre de progression (0 à 100 %) par workspace, définie via API
- Le système doit permettre d'afficher un journal d'événements horodaté (logs) par workspace, alimenté via API
- Le système doit permettre de consulter l'état complet des métadonnées d'un workspace en une seule requête
- Le système doit permettre de détecter automatiquement la fin d'un processus agent et de nettoyer ses statuts obsolètes

**Souhaitées**

- Il serait utile que le système permette d'afficher les ports réseau en écoute dans chaque workspace
- Il serait utile que le système permette d'afficher le statut de pull request (si détectable)

### 4.4 API programmatique et CLI

**Essentielles**

- Le système doit permettre à des programmes externes de contrôler wmux via un protocole de communication inter-processus compatible avec le protocole cmux existant
- Le système doit permettre de découvrir automatiquement le point de connexion via des variables d'environnement injectées dans chaque session
- Le système doit permettre d'authentifier les connexions pour empêcher l'accès non autorisé
- Le système doit permettre de configurer le niveau d'accès (désactivé, processus internes uniquement, tous les processus locaux)
- Le système doit permettre d'exécuter toutes les actions disponibles via une interface en ligne de commande
- Le système doit permettre d'obtenir les résultats en format lisible par un humain ou en format structuré pour traitement automatisé
- Le système doit permettre de cibler précisément une fenêtre, un workspace, ou une surface spécifique dans chaque commande
- Le système doit permettre d'envoyer du texte ou des séquences de touches à une surface terminal via API

**Souhaitées**

- Il serait utile que le système fournisse l'auto-complétion des commandes dans le shell

### 4.5 Navigateur intégré

**Essentielles**

- Le système doit permettre d'ouvrir un navigateur web dans une surface (pane), au même titre qu'un terminal
- Le système doit permettre de naviguer vers une URL, revenir en arrière, avancer et recharger la page
- Le système doit permettre d'interagir avec les éléments d'une page web via API (cliquer, remplir des champs, cocher, faire défiler)
- Le système doit permettre d'exécuter du code dans la page web et d'en récupérer le résultat via API
- Le système doit permettre de capturer une représentation structurée (arbre d'accessibilité) ou une image de la page via API
- Le système doit permettre d'accéder aux outils de développement du navigateur
- Le système doit permettre d'attendre qu'un élément, un texte ou un état de chargement spécifique apparaisse dans la page via API
- Le système doit permettre de gérer les cookies et le stockage local du navigateur via API

**Souhaitées**

- Il serait utile que le système permette d'injecter des scripts ou des styles personnalisés au chargement des pages
- Il serait utile que le système permette de gérer les boîtes de dialogue et les téléchargements via API

### 4.6 Notifications

**Essentielles**

- Le système doit permettre de recevoir des notifications depuis le terminal (séquences OSC), depuis l'API et depuis la ligne de commande
- Le système doit permettre d'afficher les notifications via le système natif de notifications du bureau Windows (Toast)
- Le système doit permettre d'afficher un indicateur visuel (anneau lumineux) autour du pane source d'une notification
- Le système doit permettre d'afficher un badge de notification non-lue sur le workspace source dans la sidebar
- Le système doit permettre de consulter l'historique complet des notifications dans un panneau dédié
- Le système doit permettre de naviguer directement vers le workspace source en cliquant sur une notification
- Le système doit permettre de naviguer vers la notification non-lue la plus récente via un raccourci clavier
- Le système doit permettre de gérer le cycle de vie des notifications (reçue → non-lue → lue → effacée)
- Le système doit permettre de supprimer les alertes bureau lorsque le workspace source est actif ou le panneau ouvert

**Souhaitées**

- Il serait utile que le système permette de configurer un son personnalisé pour les notifications
- Il serait utile que le système permette d'exécuter une commande shell personnalisée à chaque notification reçue
- Il serait utile que le système permette la synthèse vocale (text-to-speech) pour les notifications

### 4.7 Persistance de session

**Essentielles**

- Le système doit permettre de sauvegarder automatiquement l'état complet de la session à intervalles réguliers
- Le système doit permettre de restaurer automatiquement la session au lancement (workspaces, layouts, répertoires, scrollback)
- Le système doit permettre de restaurer la disposition des fenêtres (position, taille) et des panes
- Le système doit permettre de restaurer les URLs et l'historique de navigation des surfaces navigateur

### 4.8 Thèmes et configuration

**Essentielles**

- Le système doit permettre de personnaliser les couleurs du terminal via des fichiers de thème
- Le système doit permettre d'importer et d'appliquer des thèmes existants au format Ghostty (~50+ thèmes)
- Le système doit permettre de changer de thème sans redémarrer l'application
- Le système doit permettre de configurer la police d'affichage (avec support des ligatures et des polices Nerd Fonts)
- Le système doit permettre de détecter automatiquement le mode sombre/clair du système et d'adapter l'interface

**Souhaitées**

- Il serait utile que le système permette de lister, prévisualiser et appliquer les thèmes via la ligne de commande
- Il serait utile que le système permette de personnaliser l'opacité et la transparence de l'interface

### 4.9 Palette de commandes

**Essentielles**

- Le système doit permettre d'ouvrir un overlay de recherche rapide via un raccourci clavier pour accéder à toutes les actions disponibles
- Le système doit permettre de rechercher des commandes, raccourcis, workspaces et surfaces par recherche floue (fuzzy)
- Le système doit permettre de naviguer et d'exécuter les résultats entièrement au clavier

### 4.10 Autres besoins

**Essentielles**

- Le système doit permettre de se connecter à des machines distantes via SSH et de créer des workspaces distants durables, avec reconnexion automatique en cas de coupure
- Le système doit permettre de vérifier et d'installer les mises à jour automatiquement, avec notification dans l'interface
- Le système doit permettre d'afficher l'interface en français et en anglais, avec détection automatique de la langue système
- Le système doit permettre d'appliquer des effets visuels natifs (transparence, coins arrondis) sur les systèmes compatibles, avec un rendu dégradé gracieux sur les systèmes plus anciens
- Le système doit permettre aux hooks de shell de détecter les changements de répertoire et de mettre à jour la sidebar en temps réel

**Souhaitées**

- Il serait utile que le système permette d'ajouter des localisations supplémentaires au-delà du français et de l'anglais

## 5. Critères de succès

| Critère | Indicateur mesurable |
|---------|----------------------|
| Fluidité du terminal | Latence input-to-display < 16 ms (60 fps) |
| Compatibilité agents IA | Un script cmux fonctionne avec wmux en changeant uniquement le transport |
| Compatibilité Claude Code | Claude Code (Max) fonctionne avec les mêmes capacités qu'avec cmux |
| Parité fonctionnelle | ≥ 90 % des fonctionnalités cmux reproduites |
| Stabilité | < 1 crash par semaine d'utilisation intensive |
| Réactivité IPC | Aller-retour API < 5 ms en local |
| Ouverture navigateur | Surface navigateur opérationnelle en < 1 s |
| Restauration de session | < 3 s pour 10 workspaces |
| Recherche scrollback | < 100 ms sur 4 000 lignes |
| Mise à jour sidebar | Statuts/progress/logs mis à jour en < 500 ms après appel API |
| Notification | Délivrée en < 2 s après l'événement source |
| Mémoire (idle, 1 pane) | < 80 Mo RSS |
| Mémoire (10 panes, 3 workspaces) | < 250 Mo RSS |
| Installation | < 5 minutes de l'installation au premier terminal fonctionnel |
| Adoption communautaire | 100+ stars GitHub dans les 3 premiers mois |

## 6. Points à clarifier

- Le support des shells WSL (bash, zsh, fish) est-il attendu dès le MVP ou peut-il être reporté en v2 ?
- Quel est le comportement attendu si WebView2 n'est pas installé sur le système : blocage de l'installation ou dégradation gracieuse (terminal uniquement) ?
- Les raccourcis clavier par défaut sont-ils définitivement fixés ou doivent-ils encore être validés par des tests utilisateur ?
- Le daemon SSH distant (Go) est-il réutilisé tel quel depuis cmux ou nécessite-t-il des adaptations spécifiques à wmux ?
- La restauration du scrollback lors de la reprise de session est décrite comme "best-effort" : quel est le seuil acceptable de perte de données ?
- Le support multi-fenêtres (plusieurs Window) est-il attendu dès le MVP ou uniquement mono-fenêtre ?
- Quel est le comportement attendu pour les Nerd Fonts si la police configurée ne les supporte pas (fallback automatique ou message d'erreur) ?
