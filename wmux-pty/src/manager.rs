use std::collections::HashMap;
use std::fmt;
use std::fs::File;
use std::path::{Path, PathBuf};

use crate::conpty::{create_conpty, ConPtyHandle};
use crate::error::PtyError;
use crate::shell::{detect_shell, ShellType};
use crate::spawn::{spawn_command, ChildProcess};

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
/// Owns the ConPTY handle, I/O pipe files, and child process. The actor
/// consumes this via [`into_parts`] to distribute components across tasks.
pub struct PtyHandle {
    reader: File,
    writer: File,
    child: ChildProcess,
    conpty: ConPtyHandle,
}

impl fmt::Debug for PtyHandle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PtyHandle").finish_non_exhaustive()
    }
}

impl PtyHandle {
    /// Resize the terminal to the given dimensions.
    ///
    /// `cols` is clamped to a minimum of 2 to prevent ConPTY bug #19922
    /// (a 2-column character on a 1-column terminal causes an infinite loop).
    /// `rows` is clamped to a minimum of 1.
    pub fn resize(&self, rows: u16, cols: u16) -> Result<(), PtyError> {
        tracing::debug!(rows, cols, "resizing pty");
        self.conpty.resize(cols, rows)
    }

    /// Process ID of the child (shell) process.
    pub fn child_pid(&self) -> u32 {
        self.child.pid()
    }

    /// Consume the handle, returning its individual components.
    ///
    /// Used by [`PtyActorHandle`](crate::PtyActorHandle) to move each component
    /// into separate async tasks.
    pub fn into_parts(self) -> (File, File, ChildProcess, ConPtyHandle) {
        (self.reader, self.writer, self.child, self.conpty)
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

/// Collect shell arguments and extra environment variables for hook injection.
///
/// Returns `(args, extra_env)` where `extra_env` should be merged into the
/// spawn environment. PowerShell hooks use an env var to avoid path quoting
/// issues with `quote_arg` (spaces/backslashes in paths cause `-Command`
/// values to be double-quoted, which PowerShell misinterprets).
fn build_shell_args(
    base_args: &[String],
    shell_type: &ShellType,
    hook_dir: Option<&Path>,
) -> (Vec<String>, Vec<(String, String)>) {
    let mut args: Vec<String> = base_args.to_vec();
    let mut extra_env: Vec<(String, String)> = Vec::new();

    let Some(hook_dir) = hook_dir else {
        return (args, extra_env);
    };

    match shell_type {
        ShellType::Pwsh | ShellType::PowerShell => {
            let hook_path = hook_dir.join("wmux.ps1");
            // Pass hook path via env var to avoid quote_arg mangling the
            // -Command value. `. $env:WMUX_SHELL_HOOK` has no spaces or
            // backslashes, so quote_arg passes it through unchanged.
            extra_env.push((
                "WMUX_SHELL_HOOK".to_string(),
                hook_path.to_string_lossy().into_owned(),
            ));
            args.push("-NoExit".to_string());
            args.push("-ExecutionPolicy".to_string());
            args.push("Bypass".to_string());
            args.push("-Command".to_string());
            args.push(". $env:WMUX_SHELL_HOOK".to_string());
            tracing::debug!(hook = %hook_path.display(), "injected PowerShell shell integration hook");
        }
        ShellType::Bash => {
            let hook_path = hook_dir.join("wmux.bash");
            let wrapper = std::env::temp_dir().join("wmux_bash_rc.sh");
            let content = format!(
                "[ -f ~/.bashrc ] && . ~/.bashrc\n. '{}'\n",
                hook_path.display()
            );
            if let Err(e) = std::fs::write(&wrapper, content) {
                tracing::warn!(error = %e, "failed to write bash wrapper rcfile, skipping hook injection");
                return (args, extra_env);
            }
            tracing::debug!(hook = %hook_path.display(), "injected Bash shell integration hook");
            args.push("--rcfile".to_string());
            args.push(wrapper.to_string_lossy().into_owned());
        }
        ShellType::Cmd => {
            tracing::debug!("cmd.exe does not support shell integration hooks, skipping");
        }
    }

    (args, extra_env)
}

/// Manages PTY lifecycle: shell detection, spawning, and handle creation.
///
/// Uses ConPTY directly via the `windows` crate — no portable-pty dependency.
/// ConPTY is created with `PSEUDOCONSOLE_RESIZE_QUIRK` to prevent reflow
/// output on resize, and supports proper 24H2+ shutdown via
/// `ReleasePseudoConsole`.
#[derive(Debug)]
pub struct PtyManager;

impl PtyManager {
    /// Create a new `PtyManager`.
    pub fn new() -> Self {
        Self
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

        // Resolve working directory
        let cwd = config
            .working_directory
            .or_else(dirs::home_dir)
            .unwrap_or_else(|| PathBuf::from("."));
        let cwd = if cwd.is_dir() {
            tracing::debug!(cwd = %cwd.display(), "set working directory");
            cwd
        } else {
            let fallback = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
            tracing::warn!(
                requested = %cwd.display(),
                fallback = %fallback.display(),
                "working directory does not exist, using fallback"
            );
            fallback
        };

        // Build environment variables
        let mut env = HashMap::new();
        env.insert("TERM".to_string(), "xterm-256color".to_string());
        env.insert("COLORTERM".to_string(), "truecolor".to_string());
        env.insert("TERM_PROGRAM".to_string(), "wmux".to_string());
        env.extend(config.env);

        // Inject shell integration hooks
        let hook_dir = match ensure_hook_files() {
            Ok(dir) => Some(dir),
            Err(e) => {
                tracing::warn!(error = %e, "failed to set up shell integration hook files");
                None
            }
        };
        let (args, extra_env) = build_shell_args(&config.args, &shell_type, hook_dir.as_deref());
        for (k, v) in extra_env {
            env.insert(k, v);
        }

        // Create ConPTY with PSEUDOCONSOLE_RESIZE_QUIRK
        let pair = create_conpty(config.cols, config.rows)?;

        // Spawn shell inside the ConPTY
        let child = spawn_command(pair.conpty.hpcon(), &shell_path, &args, &env, &cwd)?;

        tracing::info!(
            shell = %shell_path.display(),
            rows = config.rows,
            cols = config.cols,
            pid = child.pid(),
            "pty session spawned (ConPTY direct)"
        );

        Ok(PtyHandle {
            reader: pair.output_read,
            writer: pair.input_write,
            child,
            conpty: pair.conpty,
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
        let _manager = PtyManager::default();
    }

    #[test]
    fn build_shell_args_powershell_hooks() {
        let hook_dir = PathBuf::from("C:/test/hooks");
        let (args, extra_env) = build_shell_args(&[], &ShellType::Pwsh, Some(&hook_dir));
        assert!(args.contains(&"-NoExit".to_string()));
        assert!(args.contains(&"-ExecutionPolicy".to_string()));
        assert!(args.contains(&"Bypass".to_string()));
        assert!(args.contains(&". $env:WMUX_SHELL_HOOK".to_string()));
        assert!(extra_env.iter().any(|(k, _)| k == "WMUX_SHELL_HOOK"));
    }

    #[test]
    fn build_shell_args_no_hook_dir() {
        let (args, extra_env) = build_shell_args(&["--login".to_string()], &ShellType::Bash, None);
        assert_eq!(args, vec!["--login".to_string()]);
        assert!(extra_env.is_empty());
    }

    #[test]
    fn build_shell_args_cmd_no_hooks() {
        let (args, extra_env) = build_shell_args(&[], &ShellType::Cmd, Some(Path::new("C:/hooks")));
        assert!(args.is_empty());
        assert!(extra_env.is_empty());
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

        // Small resize: cols clamped to 2 (ConPTY bug #19922 prevention)
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
