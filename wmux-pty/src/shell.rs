use std::borrow::Cow;
use std::fmt;
use std::path::PathBuf;
use std::process::Command;

use crate::PtyError;

/// Type of shell detected on the system.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ShellType {
    /// PowerShell 7+ (pwsh)
    Pwsh,
    /// Windows PowerShell 5.x (powershell)
    PowerShell,
    /// Command Prompt (cmd.exe)
    Cmd,
    /// Bash (Git Bash / MSYS2)
    Bash,
}

impl fmt::Display for ShellType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Pwsh => write!(f, "PowerShell 7"),
            Self::PowerShell => write!(f, "Windows PowerShell"),
            Self::Cmd => write!(f, "Command Prompt"),
            Self::Bash => write!(f, "Bash"),
        }
    }
}

/// Information about a detected shell.
#[derive(Debug, Clone)]
pub struct ShellInfo {
    /// Full path to the shell executable.
    pub path: PathBuf,
    /// Human-readable display name.
    pub name: Cow<'static, str>,
    /// Detected shell type.
    pub shell_type: ShellType,
}

/// Detect the best available shell on the system.
///
/// Detection priority: pwsh → powershell → cmd.exe.
/// Falls back to cmd.exe which is always available on Windows.
pub fn detect_shell() -> Result<ShellInfo, PtyError> {
    let candidates = [
        ("pwsh", ShellType::Pwsh, "PowerShell 7"),
        ("powershell", ShellType::PowerShell, "Windows PowerShell"),
        ("bash", ShellType::Bash, "Bash"),
        ("cmd", ShellType::Cmd, "Command Prompt"),
    ];

    for (exe, shell_type, display_name) in candidates {
        tracing::debug!(shell = exe, "checking shell availability");
        if let Some(path) = find_in_path(exe) {
            tracing::info!(shell = exe, path = %path.display(), "detected shell");
            return Ok(ShellInfo {
                path,
                name: Cow::Borrowed(display_name),
                shell_type,
            });
        }
    }

    // cmd.exe should always exist — if where.exe failed, use it directly
    tracing::warn!("no shell found via where.exe, falling back to cmd.exe");
    Ok(ShellInfo {
        path: PathBuf::from("cmd.exe"),
        name: Cow::Borrowed("Command Prompt"),
        shell_type: ShellType::Cmd,
    })
}

/// Check if an executable exists in PATH using `where.exe`.
fn find_in_path(name: &str) -> Option<PathBuf> {
    let output = Command::new("where.exe").arg(name).output().ok()?;

    if !output.status.success() {
        return None;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let first_line = stdout.lines().next()?;
    let trimmed = first_line.trim();
    if trimmed.is_empty() {
        return None;
    }

    Some(PathBuf::from(trimmed))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detect_shell_returns_ok() {
        // cmd.exe always exists on Windows, so this should never fail
        let result = detect_shell();
        assert!(result.is_ok());
    }

    #[test]
    fn shell_type_display() {
        assert_eq!(ShellType::Pwsh.to_string(), "PowerShell 7");
        assert_eq!(ShellType::PowerShell.to_string(), "Windows PowerShell");
        assert_eq!(ShellType::Cmd.to_string(), "Command Prompt");
        assert_eq!(ShellType::Bash.to_string(), "Bash");
    }

    #[test]
    fn find_cmd_in_path() {
        // cmd.exe should always be findable on Windows
        let result = find_in_path("cmd");
        assert!(result.is_some());
    }
}
