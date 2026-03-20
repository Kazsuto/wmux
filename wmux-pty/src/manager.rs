use std::collections::HashMap;
use std::fmt;
use std::io::{Read, Write};
use std::path::PathBuf;

use portable_pty::{native_pty_system, Child, CommandBuilder, MasterPty, PtySize, PtySystem};

use crate::error::PtyError;
use crate::shell::detect_shell;

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
        let shell_path = match config.shell {
            Some(path) => {
                tracing::info!(shell = %path.display(), "using configured shell");
                path
            }
            None => {
                let info = detect_shell()?;
                tracing::info!(
                    shell = %info.name,
                    path = %info.path.display(),
                    "using detected shell"
                );
                info.path
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
