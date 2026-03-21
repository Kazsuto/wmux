use std::path::PathBuf;

/// Connection state of a remote workspace.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RemoteConnectionState {
    Connecting,
    Connected,
    Disconnected { reason: String },
    Reconnecting { attempt: u32 },
}

/// Configuration for a remote workspace connection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteConfig {
    pub user: String,
    pub host: String,
    pub port: u16,
    pub identity_file: Option<PathBuf>,
}

impl RemoteConfig {
    /// Parse a "user@host[:port]" connection string.
    pub fn parse(target: &str) -> Result<Self, RemoteError> {
        let (user_host, port) = if let Some((uh, p)) = target.rsplit_once(':') {
            let port: u16 = p.parse().map_err(|_| RemoteError::InvalidTarget {
                target: target.to_owned(),
            })?;
            (uh, port)
        } else {
            (target, 22)
        };

        let (user, host) = user_host
            .split_once('@')
            .ok_or(RemoteError::InvalidTarget {
                target: target.to_owned(),
            })?;

        if user.is_empty() || host.is_empty() {
            return Err(RemoteError::InvalidTarget {
                target: target.to_owned(),
            });
        }

        // Reject dangerous characters that could be interpreted as SSH options
        // or shell metacharacters if passed through a shell context.
        fn is_safe_ssh_field(s: &str) -> bool {
            !s.starts_with('-')
                && s.bytes()
                    .all(|b| b > 0x20 && !b"'\"\\;|&`$(){}[]!#~".contains(&b))
        }

        if !is_safe_ssh_field(user) || !is_safe_ssh_field(host) {
            return Err(RemoteError::InvalidTarget {
                target: target.to_owned(),
            });
        }

        Ok(Self {
            user: user.to_owned(),
            host: host.to_owned(),
            port,
            identity_file: None,
        })
    }

    /// Build SSH command arguments for connecting.
    pub fn ssh_args(&self) -> Vec<String> {
        let mut args = vec![
            "-o".to_owned(),
            "StrictHostKeyChecking=accept-new".to_owned(),
            "-p".to_owned(),
            self.port.to_string(),
        ];
        if let Some(key) = &self.identity_file {
            args.push("-i".to_owned());
            args.push(key.display().to_string());
        }
        args.push(format!("{}@{}", self.user, self.host));
        args
    }
}

/// Reconnection backoff calculator using exponential backoff.
///
/// Delays: 1s, 2s, 4s, 8s, 16s, 32s, 60s (capped).
#[derive(Debug)]
pub struct ReconnectBackoff {
    attempt: u32,
    max_delay_secs: u64,
}

impl ReconnectBackoff {
    pub fn new() -> Self {
        Self {
            attempt: 0,
            max_delay_secs: 60,
        }
    }

    pub fn next_delay(&mut self) -> std::time::Duration {
        let delay = (1u64 << self.attempt.min(6)).min(self.max_delay_secs);
        self.attempt += 1;
        std::time::Duration::from_secs(delay)
    }

    pub fn reset(&mut self) {
        self.attempt = 0;
    }

    pub fn attempt(&self) -> u32 {
        self.attempt
    }
}

impl Default for ReconnectBackoff {
    fn default() -> Self {
        Self::new()
    }
}

/// Error type for remote operations.
#[derive(Debug, thiserror::Error)]
pub enum RemoteError {
    #[error("invalid SSH target: {target}")]
    InvalidTarget { target: String },
    #[error("SSH connection failed: {0}")]
    ConnectionFailed(String),
    #[error("daemon provisioning failed: {0}")]
    DaemonProvisionFailed(String),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

#[cfg(test)]
mod tests {
    use super::*;

    // RemoteConfig::parse tests

    #[test]
    fn parse_user_at_host() {
        let config = RemoteConfig::parse("alice@example.com").unwrap();
        assert_eq!(config.user, "alice");
        assert_eq!(config.host, "example.com");
        assert_eq!(config.port, 22);
        assert!(config.identity_file.is_none());
    }

    #[test]
    fn parse_user_at_host_with_port() {
        let config = RemoteConfig::parse("bob@192.168.1.1:2222").unwrap();
        assert_eq!(config.user, "bob");
        assert_eq!(config.host, "192.168.1.1");
        assert_eq!(config.port, 2222);
    }

    #[test]
    fn parse_missing_at_sign_is_error() {
        assert!(RemoteConfig::parse("nousernamehere").is_err());
    }

    #[test]
    fn parse_empty_user_is_error() {
        assert!(RemoteConfig::parse("@host.example.com").is_err());
    }

    #[test]
    fn parse_empty_host_is_error() {
        assert!(RemoteConfig::parse("user@").is_err());
    }

    #[test]
    fn parse_invalid_port_is_error() {
        assert!(RemoteConfig::parse("user@host:notaport").is_err());
    }

    #[test]
    fn parse_port_out_of_range_is_error() {
        // 99999 overflows u16
        assert!(RemoteConfig::parse("user@host:99999").is_err());
    }

    #[test]
    fn ssh_args_default_port() {
        let config = RemoteConfig::parse("carol@server.local").unwrap();
        let args = config.ssh_args();
        assert!(args.contains(&"-o".to_owned()));
        assert!(args.contains(&"StrictHostKeyChecking=accept-new".to_owned()));
        assert!(args.contains(&"-p".to_owned()));
        assert!(args.contains(&"22".to_owned()));
        assert!(args.last().unwrap() == "carol@server.local");
    }

    #[test]
    fn ssh_args_with_identity_file() {
        let mut config = RemoteConfig::parse("dave@host.example").unwrap();
        config.identity_file = Some(std::path::PathBuf::from("/home/dave/.ssh/id_ed25519"));
        let args = config.ssh_args();
        assert!(args.contains(&"-i".to_owned()));
        assert!(args.contains(&"/home/dave/.ssh/id_ed25519".to_owned()));
    }

    // ReconnectBackoff tests

    #[test]
    fn backoff_starts_at_one_second() {
        let mut backoff = ReconnectBackoff::new();
        assert_eq!(backoff.next_delay(), std::time::Duration::from_secs(1));
    }

    #[test]
    fn backoff_doubles_each_attempt() {
        let mut backoff = ReconnectBackoff::new();
        let delays: Vec<u64> = (0..7).map(|_| backoff.next_delay().as_secs()).collect();
        assert_eq!(delays, vec![1, 2, 4, 8, 16, 32, 60]);
    }

    #[test]
    fn backoff_caps_at_sixty_seconds() {
        let mut backoff = ReconnectBackoff::new();
        // Exhaust the exponential growth
        for _ in 0..10 {
            backoff.next_delay();
        }
        assert_eq!(backoff.next_delay(), std::time::Duration::from_secs(60));
    }

    #[test]
    fn backoff_resets_correctly() {
        let mut backoff = ReconnectBackoff::new();
        for _ in 0..5 {
            backoff.next_delay();
        }
        assert!(backoff.attempt() > 0);
        backoff.reset();
        assert_eq!(backoff.attempt(), 0);
        assert_eq!(backoff.next_delay(), std::time::Duration::from_secs(1));
    }

    #[test]
    fn backoff_attempt_increments() {
        let mut backoff = ReconnectBackoff::new();
        assert_eq!(backoff.attempt(), 0);
        backoff.next_delay();
        assert_eq!(backoff.attempt(), 1);
        backoff.next_delay();
        assert_eq!(backoff.attempt(), 2);
    }
}
