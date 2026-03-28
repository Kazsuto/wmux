# wmux — Distribution & Installation Guide

Guide complet pour passer de `cargo run` à une application native installable.

## État actuel

| Aspect | État | Action requise |
|--------|------|----------------|
| Code & compilation | ✅ Prêt | — |
| Release profile (LTO, strip, opt-level 3) | ✅ Excellent | — |
| README | ✅ Complet | — |
| Metadata Cargo.toml (authors, description, repo) | ❌ Incomplet | Étape 2b |
| Icône .exe Windows (.ico) | ❌ Absent | Étape 2a |
| Manifest Windows (DPI, UAC) | ❌ Absent | Étape 2a |
| build.rs (embed resources) | ❌ Absent | Étape 2a |
| CI/CD GitHub Actions | ❌ Absent | Étape 2c |
| Installer .msi (cargo-wix) | ❌ Absent | Étape 2c |
| Package managers (winget, scoop) | ❌ Absent | Niveau 3 |

---

## Niveau 1 — Utilisation locale (immédiat)

### Build release

```bash
cargo build --release
```

Produit deux binaires dans `target/release/` :
- **`wmux-app.exe`** — application GUI (terminal multiplexer)
- **`wmux.exe`** — CLI client

### Installation manuelle

```bash
# Créer un dossier permanent pour les binaires
mkdir -p ~/bin

# Copier les binaires
cp target/release/wmux-app.exe ~/bin/
cp target/release/wmux.exe ~/bin/
```

Ajouter `C:\Users\<USERNAME>\bin` au PATH Windows :
1. Paramètres Windows > Système > Informations système > Paramètres avancés
2. Variables d'environnement > PATH (utilisateur) > Nouveau
3. Ajouter le chemin du dossier `bin`
4. Redémarrer le terminal

Après ça : `wmux-app` lance l'app, `wmux` lance le CLI depuis n'importe quel terminal.

---

## Niveau 2 — Distribution GitHub (autres utilisateurs)

### Étape 2a — Identité Windows de l'exe

Actuellement les .exe ont une icône générique et aucune info dans "Propriétés".

#### 1. Créer l'icône

Fichier `resources/icon.ico` — doit contenir les tailles : 256x256, 48x48, 32x32, 16x16.

Outils pour créer un .ico :
- [RealFaviconGenerator](https://realfavicongenerator.net/) — upload un PNG 512x512, télécharge le .ico
- [GIMP](https://www.gimp.org/) — export en .ico avec multi-résolution
- [ImageMagick](https://imagemagick.org/) : `magick icon-512.png -define icon:auto-resize=256,48,32,16 icon.ico`

#### 2. Créer le manifest Windows

Fichier `resources/app.manifest` :

```xml
<?xml version="1.0" encoding="utf-8"?>
<assembly manifestVersion="1.0" xmlns="urn:schemas-microsoft-com:asm.v1"
          xmlns:asmv3="urn:schemas-microsoft-com:asm.v3">
  <assemblyIdentity
    type="win32"
    name="wmux"
    version="0.1.0.0"
    processorArchitecture="amd64" />

  <description>wmux — Native Windows Terminal Multiplexer</description>

  <!-- DPI awareness (Win10+) -->
  <asmv3:application>
    <asmv3:windowsSettings
      xmlns="http://schemas.microsoft.com/SMI/2016/WindowsSettings">
      <dpiAwareness>PerMonitorV2</dpiAwareness>
      <dpiAware>true</dpiAware>
    </asmv3:windowsSettings>
  </asmv3:application>

  <!-- UAC : pas d'élévation requise -->
  <trustInfo xmlns="urn:schemas-microsoft-com:asm.v3">
    <security>
      <requestedPrivileges>
        <requestedExecutionLevel level="asInvoker" uiAccess="false" />
      </requestedPrivileges>
    </security>
  </trustInfo>

  <!-- Compatibilité OS -->
  <compatibility xmlns="urn:schemas-microsoft-com:compatibility.v1">
    <application>
      <!-- Windows 10 / 11 -->
      <supportedOS Id="{8e0f7a12-bfb3-4fe8-b9a5-48fd50a15a9a}" />
    </application>
  </compatibility>
</assembly>
```

#### 3. Ajouter winresource au build

Dépendance dans `wmux-app/Cargo.toml` :

```toml
[build-dependencies]
winresource = "0.1"
```

Créer `wmux-app/build.rs` :

```rust
fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default() == "windows" {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("../resources/icon.ico");
        res.set_manifest_file("../resources/app.manifest");
        res.set("FileDescription", "wmux — Native Windows Terminal Multiplexer");
        res.set("ProductName", "wmux");
        res.set("OriginalFilename", "wmux-app.exe");
        res.set("LegalCopyright", "MIT License");
        res.compile()
            .expect("Failed to compile Windows resources");
    }
}
```

Même chose pour `wmux-cli/build.rs` (avec `OriginalFilename` = `wmux.exe`).

Résultat : l'exe aura une icône custom + infos complètes dans Propriétés > Détails.

---

### Étape 2b — Metadata Cargo.toml

Dans `Cargo.toml` (workspace root), compléter :

```toml
[workspace.package]
version = "0.1.0"
edition = "2021"
license = "MIT"
rust-version = "1.80"
description = "Native Windows terminal multiplexer — GPU-accelerated, split panes, workspaces"
authors = ["Christian <email>"]
repository = "https://github.com/<user>/wmux"
homepage = "https://github.com/<user>/wmux"
keywords = ["terminal", "multiplexer", "windows", "gpu", "wgpu"]
categories = ["command-line-utilities", "gui"]
```

Dans `wmux-app/Cargo.toml` et `wmux-cli/Cargo.toml`, hériter :

```toml
[package]
description.workspace = true
authors.workspace = true
repository.workspace = true
homepage.workspace = true
```

---

### Étape 2c — cargo-dist + GitHub Actions (release automatisée)

#### Installation

```bash
cargo install cargo-dist
```

#### Initialisation

```bash
cargo dist init
```

Cela crée un `dist-workspace.toml` (ou section `[dist]` dans Cargo.toml). Configuration recommandée :

```toml
[dist]
cargo-dist-version = "0.27"       # version actuelle
ci = "github"                      # GitHub Actions
installers = ["msi"]               # Génère un .msi Windows
targets = ["x86_64-pc-windows-msvc"]
install-path = "CARGO_HOME"
```

#### Générer le workflow CI

```bash
cargo dist generate-ci github
```

Crée `.github/workflows/release.yml` — le workflow qui :
1. Se déclenche quand tu push un tag `v*`
2. Compile en mode release sur un runner Windows
3. Génère un `.msi` installeur + `.zip` portable
4. Crée une GitHub Release avec les artifacts
5. Ajoute les checksums SHA256

#### Workflow de release

```bash
# 1. S'assurer que tout compile
cargo clippy --workspace -- -W clippy::all
cargo fmt --all
cargo test --workspace

# 2. Mettre à jour la version dans Cargo.toml
# [workspace.package] version = "0.1.0"

# 3. Commit + tag
git add -A
git commit -m "chore: prepare v0.1.0 release"
git tag v0.1.0

# 4. Push
git push origin master --tags

# 5. GitHub Actions fait le reste automatiquement
# → Release créée sur https://github.com/<user>/wmux/releases
```

---

### Expérience utilisateur finale

Un utilisateur qui trouve le repo GitHub :

1. Va dans l'onglet **Releases**
2. Télécharge `wmux-0.1.0-x86_64.msi`
3. Double-clic → installeur Windows standard
4. `wmux-app` et `wmux` sont ajoutés au PATH automatiquement
5. Raccourci optionnel dans le menu Démarrer
6. Désinstallation propre via "Ajout/Suppression de programmes"

Alternative portable : télécharger le `.zip`, extraire, lancer directement.

---

## Niveau 3 — Package managers (futur)

### Winget (Microsoft)

cargo-dist peut auto-générer le manifest. Sinon, créer manuellement :

```yaml
# manifests/w/wmux/wmux/0.1.0/wmux.wmux.installer.yaml
PackageIdentifier: wmux.wmux
PackageVersion: 0.1.0
InstallerType: msi
Installers:
  - Architecture: x64
    InstallerUrl: https://github.com/<user>/wmux/releases/download/v0.1.0/wmux-0.1.0-x86_64.msi
    InstallerSha256: <sha256>
```

Soumettre via PR sur [microsoft/winget-pkgs](https://github.com/microsoft/winget-pkgs).

Résultat : `winget install wmux`

### Scoop

Créer un bucket custom ou soumettre au bucket principal :

```json
{
  "version": "0.1.0",
  "description": "Native Windows terminal multiplexer — GPU-accelerated",
  "homepage": "https://github.com/<user>/wmux",
  "license": "MIT",
  "url": "https://github.com/<user>/wmux/releases/download/v0.1.0/wmux-0.1.0-x86_64.zip",
  "hash": "<sha256>",
  "bin": ["wmux-app.exe", "wmux.exe"],
  "checkver": "github",
  "autoupdate": {
    "url": "https://github.com/<user>/wmux/releases/download/v$version/wmux-$version-x86_64.zip"
  }
}
```

Résultat : `scoop install wmux`

### crates.io (développeurs Rust uniquement)

```bash
cargo publish -p wmux-cli
```

Résultat : `cargo install wmux` (compile depuis les sources — nécessite Rust + MSVC toolchain).

> **Note :** crates.io est pour les développeurs Rust. Les utilisateurs finaux passent par .msi, winget ou scoop.

---

## Séquence recommandée

| Étape | Quoi | Prérequis | Effort |
|-------|------|-----------|--------|
| 1 | `cargo build --release` + copie manuelle | Aucun | 5 min |
| 2a | Icône + manifest + build.rs | Créer un .ico | ~1h |
| 2b | Metadata Cargo.toml | Créer le repo GitHub | 10 min |
| 2c | cargo-dist + GitHub Actions | Repo GitHub public | ~1h |
| 3 | Winget / Scoop manifests | Releases fonctionnelles | ~30 min chacun |

---

## Outils nécessaires

| Outil | Installation | Rôle |
|-------|-------------|------|
| `cargo-dist` | `cargo install cargo-dist` | Automatise build + release + installers |
| `cargo-wix` | `cargo install cargo-wix` | Génère .msi (utilisé par cargo-dist) |
| `winresource` | Crate Rust (build-dependency) | Embarque icône/manifest dans .exe |
| WiX Toolset v4+ | [wixtoolset.org](https://wixtoolset.org/) | Backend pour cargo-wix |

## Fichiers à créer

```
wmux/
├── resources/
│   ├── icon.ico              # Icône multi-résolution (à créer)
│   └── app.manifest          # Manifest Windows (DPI, UAC)
├── wmux-app/
│   └── build.rs              # Embed resources dans wmux-app.exe
├── wmux-cli/
│   └── build.rs              # Embed resources dans wmux.exe
├── .github/
│   └── workflows/
│       └── release.yml       # CI/CD release (généré par cargo-dist)
└── dist-workspace.toml       # Config cargo-dist (généré par cargo dist init)
```
