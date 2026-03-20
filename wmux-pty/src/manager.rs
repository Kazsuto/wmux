use std::collections::HashMap;
use std::fmt;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use portable_pty::{native_pty_system, Child, CommandBuilder, MasterPty, PtySize, PtySystem};

use crate::error::PtyError;
use crate::shell::{detect_shell, ShellType};

const HOOK_POWERSHELL: &str = include_str!("../../resources/shell-integration/wmux.ps1");
const HOOK_BASH: &str = include_str!("../../resources/shell-integration/wmux.bash");
const HOOK_ZSH: &str = include_str!("../../resources/shell-integration/wmux.zsh");

/// Configuration for spawning a new PTY session.
#[derive(Debug, Clone)]
pub struct SpawnConfig {
    /// Path to shell executable. If `None`, auto-detects the best available shell.
    pub shell: Option<PathBuf>,
    /// Arguments to pass to the shell.
    pub args: Vec<String>,
    /// Extra environment variables to inject into the spawned process.
    pub env: HashMap<String, String>,
    /// Working directory. Falls back to user home directory, then `"."`.
    pub working_directory: Option<PathBuf>,
    /// Number of terminal columns.
    pub cols: u16,
    /// Number of terminal rows.
    pub rows: u16,
}

impl Default for SpawnConfig {
    fn default() -> Self {
        Self {
            shell: None,
            args: Vec::new(),
            env: HashMap::new(),
            working_directory: None,
            cols: 80,
            rows: 24,
        }
    }
}

/// Handle to a running PTY session.
///
/// Provides reader/writer access to the terminal I/O and control over
/// the child process and terminal dimensions.
pub struct PtyHandle {
    reader: Box<dyn Read + Send>,
    writer: Box<dyn Write + Send>,
    child: Box<dyn Child + Send + Sync>,
    master: Box<dyn MasterPty + Send>,
}

impl fmt::Debug for PtyHandle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PtyHandle").finish_non_exhaustive()
    }
}

impl PtyHandle {
    /// Get a mutable reference to the PTY reader (terminal output).
    #[inline]
    pub fn reader_mut(&mut self) -> &mut (dyn Read + Send) {
        &mut *self.reader
    }

    /// Get a mutable reference to the PTY writer (terminal input).
    #[inline]
    pub fn writer_mut(&mut self) -> &mut (dyn Write + Send) {
        &mut *self.writer
    }

    /// Get a mutable reference to the child process.
    #[inline]
    pub fn child_mut(&mut self) -> &mut (dyn Child + Send + Sync) {
        &mut *self.child
    }

    /// Clone an additional reader from the master PTY.
    pub fn try_clone_reader(&self) -> Result<Box<dyn Read + Send>, PtyError> {
        self.master
            .try_clone_reader()
            .map_err(|e| PtyError::CloneReaderFailed(e.into()))
    }

    /// Resize the terminal to the given dimensions.
    pub fn resize(&self, rows: u16, cols: u16) -> Result<(), PtyError> {
        tracing::debug!(rows, cols, "resizing pty");
        self.master
            .resize(PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|e| PtyError::ResizeFailed(e.into()))
    }

    /// Consume the handle, returning its individual components.
    ///
    /// Used by [`PtyActorHandle`](crate::PtyActorHandle) to move each component
    /// into separate async tasks.
    #[allow(clippy::type_complexity)]
    pub fn into_parts(
        self,
    ) -> (
        Box<dyn Read + Send>,
        Box<dyn Write + Send>,
        Box<dyn Child + Send + Sync>,
        Box<dyn MasterPty + Send>,
    ) {
        (self.reader, self.writer, self.child, self.master)
    }
}

/// Infer [`ShellType`] from a shell executable path.
fn shell_type_from_path(path: &Path) -> ShellType {
    let name = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    match name.as_str() {
        "pwsh" => ShellType::Pwsh,
        "powershell" => ShellType::PowerShell,
        "bash" => ShellType::Bash,
        _ => ShellType::Cmd,
    }
}

/// Write shell integration hook scripts to the user config directory.
///
/// Returns the directory containing the written hook files.
/// Uses `std::fs` (sync) intentionally — this is called from a synchronous spawn context
/// and the writes are one-time, tiny files.
fn ensure_hook_files() -> Result<PathBuf, PtyError> {
    let config = dirs::config_dir()
        .ok_or_else(|| PtyError::ShellNotFound("config directory not found".to_string()))?;
    let hook_dir = config.join("wmux").join("shell-integration");
    std::fs::create_dir_all(&hook_dir)?;
    std::fs::write(hook_dir.join("wmux.ps1"), HOOK_POWERSHELL)?;
    std::fs::write(hook_dir.join("wmux.bash"), HOOK_BASH)?;
    std::fs::write(hook_dir.join("wmux.zsh"), HOOK_ZSH)?;
    Ok(hook_dir)
}

/// Configure the `CommandBuilder` to source the wmux shell integration hook on startup.
fn inject_shell_hooks(cmd: &mut CommandBuilder, shell_type: &ShellType, hook_dir: &Path) {
    match shell_type {
        ShellType::Pwsh | ShellType::PowerShell => {
            let hook_path = hook_dir.join("wmux.ps1");
            // Escape single quotes in the path (PowerShell convention: '' inside '...')
            let hook_path_str = hook_path.to_string_lossy().replace('\'', "''");
            // -NoExit keeps PowerShell interactive after running -Command
            // -ExecutionPolicy Bypass allows loading an unsigned script for this session
            cmd.arg("-NoExit");
            cmd.arg("-ExecutionPolicy");
            cmd.arg("Bypass");
            cmd.arg("-Command");
            cmd.arg(format!(". '{hook_path_str}'"));
            tracing::debug!(hook = %hook_path.display(), "injected PowerShell shell integration hook");
        }
        ShellType::Bash => {
            let hook_path = hook_dir.join("wmux.bash");
            // Create a wrapper rcfile: source ~/.bashrc first, then the wmux hook.
            // This replaces the default rcfile via --rcfile without losing the user's config.
            let wrapper = std::env::temp_dir().join("wmux_bash_rc.sh");
            let content = format!(
                "[ -f ~/.bashrc ] && . ~/.bashrc\n. '{}'\n",
                hook_path.display()
            );
            if let Err(e) = std::fs::write(&wrapper, content) {
                tracing::warn!(error = %e, "failed to write bash wrapper rcfile, skipping hook injection");
                return;
            }
            tracing::debug!(hook = %hook_path.display(), "injected Bash shell integration hook");
            cmd.arg("--rcfile");
            cmd.arg(wrapper);
        }
        ShellType::Cmd => {
            tracing::debug!("cmd.exe does not support shell integration hooks, skipping");
        }
    }
}

/// Manages PTY lifecycle: shell detection, spawning, and handle creation.
pub struct PtyManager {
    pty_system: Box<dyn PtySystem + Send>,
}

impl fmt::Debug for PtyManager {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PtyManager").finish_non_exhaustive()
    }
}

impl PtyManager {
    /// Create a new `PtyManager` using the native PTY system (ConPTY on Windows).
    pub fn new() -> Self {
        Self {
            pty_system: native_pty_system(),
        }
    }

    /// Spawn a new PTY session with the given configuration.
    pub fn spawn(&self, config: SpawnConfig) -> Result<PtyHandle, PtyError> {
        // Detect or use configured shell
        let (shell_path, shell_type) = match config.shell {
            Some(path) => {
                tracing::info!(shell = %path.display(), "using configured shell");
                let st = shell_type_from_path(&path);
                (path, st)
            }
            None => {
                let info = detect_shell()?;
                tracing::info!(
                    shell = %info.name,
                    path = %info.path.display(),
                    "using detected shell"
                );
                (info.path, info.shell_type)
            }
        };

        // Build command
        let mut cmd = CommandBuilder::new(&shell_path);
        for arg in &config.args {
            cmd.arg(arg);
        }

        // Set working directory
        let cwd = config
            .working_directory
            .or_else(dirs::home_dir)
            .unwrap_or_else(|| PathBuf::from("."));
        if cwd.is_dir() {
            cmd.cwd(&cwd);
            tracing::debug!(cwd = %cwd.display(), "set working directory");
        } else {
            let fallback = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
            tracing::warn!(
                requested = %cwd.display(),
                fallback = %fallback.display(),
                "working directory does not exist, using fallback"
            );
            cmd.cwd(&fallback);
        }

        // Inject standard environment variables
        cmd.env("TERM", "xterm-256color");
        cmd.env("TERM_PROGRAM", "wmux");

        // Inject WMUX_* variables from config
        for (key, value) in &config.env {
            cmd.env(key, value);
        }

        // Inject shell integration hooks (OSC 7 CWD tracking, OSC 133 prompt marks)
        match ensure_hook_files() {
            Ok(hook_dir) => inject_shell_hooks(&mut cmd, &shell_type, &hook_dir),
            Err(e) => tracing::warn!(error = %e, "failed to set up shell integration hook files"),
        }

        // Open PTY pair
        let size = PtySize {
            rows: config.rows,
            cols: config.cols,
            pixel_width: 0,
            pixel_height: 0,
        };

        let pair = self
            .pty_system
            .openpty(size)
            .map_err(|e| PtyError::SpawnFailed(e.into()))?;

        // Spawn command in the slave PTY
        let child = pair
            .slave
            .spawn_command(cmd)
            .map_err(|e| PtyError::SpawnFailed(e.into()))?;

        tracing::info!(
            shell = %shell_path.display(),
            rows = config.rows,
            cols = config.cols,
            "pty session spawned"
        );

        // Obtain reader and writer from master
        let reader = pair
            .master
            .try_clone_reader()
            .map_err(|e| PtyError::SpawnFailed(e.into()))?;
        let writer = pair
            .master
            .take_writer()
            .map_err(|e| PtyError::SpawnFailed(e.into()))?;

        Ok(PtyHandle {
            reader,
            writer,
            child,
            master: pair.master,
        })
    }
}

impl Default for PtyManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hook_content_not_empty() {
        assert!(!HOOK_POWERSHELL.is_empty());
        assert!(!HOOK_BASH.is_empty());
        assert!(!HOOK_ZSH.is_empty());
    }

    #[test]
    fn shell_type_from_path_detection() {
        assert_eq!(shell_type_from_path(Path::new("pwsh.exe")), ShellType::Pwsh);
        assert_eq!(
            shell_type_from_path(Path::new("powershell.exe")),
            ShellType::PowerShell
        );
        assert_eq!(shell_type_from_path(Path::new("bash.exe")), ShellType::Bash);
        assert_eq!(shell_type_from_path(Path::new("cmd.exe")), ShellType::Cmd);
        // Unknown shell falls back to Cmd
        assert_eq!(shell_type_from_path(Path::new("fish")), ShellType::Cmd);
    }

    #[test]
    #[ignore] // Writes to user config directory — run with: cargo test -- --ignored
    fn ensure_hook_files_writes_all_scripts() {
        let hook_dir = ensure_hook_files().expect("failed to write hook files");
        assert!(hook_dir.join("wmux.ps1").exists());
        assert!(hook_dir.join("wmux.bash").exists());
        assert!(hook_dir.join("wmux.zsh").exists());
    }

    #[test]
    fn spawn_config_defaults() {
        let config = SpawnConfig::default();
        assert!(config.shell.is_none());
        assert!(config.args.is_empty());
        assert!(config.env.is_empty());
        assert!(config.working_directory.is_none());
        assert_eq!(config.cols, 80);
        assert_eq!(config.rows, 24);
    }

    #[test]
    fn pty_manager_default() {
        // Should not panic
        let _manager = PtyManager::default();
    }

    #[test]
    #[ignore] // Requires real ConPTY — run with: cargo test -- --ignored
    fn spawn_default_shell() {
        let manager = PtyManager::new();
        let config = SpawnConfig::default();
        let handle = manager.spawn(config);
        assert!(handle.is_ok(), "failed to spawn: {:?}", handle.err());
    }

    #[test]
    #[ignore] // Requires real ConPTY
    fn spawn_and_resize() {
        let manager = PtyManager::new();
        let config = SpawnConfig::default();
        let handle = manager.spawn(config).expect("failed to spawn");

        // Normal resize
        assert!(handle.resize(50, 120).is_ok());

        // Small resize (1x1) should not panic
        assert!(handle.resize(1, 1).is_ok());
    }

    #[test]
    #[ignore] // Requires real ConPTY
    fn spawn_with_env_vars() {
        let manager = PtyManager::new();
        let mut config = SpawnConfig::default();
        config
            .env
            .insert("WMUX_WORKSPACE_ID".to_string(), "test-ws".to_string());
        config
            .env
            .insert("WMUX_SURFACE_ID".to_string(), "test-sf".to_string());

        let handle = manager.spawn(config);
        assert!(handle.is_ok());
    }

    #[test]
    #[ignore] // Requires real ConPTY
    fn spawn_nonexistent_cwd_falls_back() {
        let manager = PtyManager::new();
        let mut config = SpawnConfig::default();
        config.working_directory = Some(PathBuf::from("C:\\nonexistent_dir_12345"));

        // Should not fail — falls back to home dir
        let handle = manager.spawn(config);
        assert!(handle.is_ok());
    }
}
