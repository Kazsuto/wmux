use std::process::Command;

use windows::{
    core::HSTRING,
    Data::Xml::Dom::XmlDocument,
    UI::Notifications::{ToastNotification, ToastNotificationManager},
};
use wmux_core::Notification;

/// App User Model ID for wmux toast notifications.
const AUMID: &str = "wmux";

/// Set the process-level AUMID required for toast notifications.
///
/// Must be called once at startup before any toast is shown.
/// Safe to call multiple times — idempotent.
pub fn init_aumid() {
    // SAFETY: SetCurrentProcessExplicitAppUserModelID is safe to call from
    // any thread and only writes to process-global COM state.
    unsafe {
        let aumid = HSTRING::from(AUMID);
        let _ = windows::Win32::UI::Shell::SetCurrentProcessExplicitAppUserModelID(&aumid);
    }
    tracing::debug!("AUMID set to {AUMID}");
}

/// Windows Toast notification service.
///
/// Wraps the WinRT `ToastNotificationManager` and provides a simple
/// interface for showing toast notifications with title and body.
pub struct ToastService {
    _private: (),
}

impl ToastService {
    /// Create a new toast service.
    pub fn new() -> Self {
        Self { _private: () }
    }

    /// Show a toast notification on the Windows desktop.
    ///
    /// Builds a toast XML template with the notification's title and body,
    /// then shows it via the WinRT Toast API. Errors are logged and
    /// silently ignored (toast failures should never crash the app).
    pub fn show(&self, notification: &Notification) {
        if let Err(e) = self.show_inner(notification) {
            tracing::warn!(error = %e, "failed to show toast notification");
        }
    }

    fn show_inner(&self, notification: &Notification) -> windows::core::Result<()> {
        let title = notification.title.as_deref().unwrap_or("wmux");
        let body = &notification.body;

        // Build toast XML template.
        let xml = XmlDocument::new()?;
        let template = format!(
            r#"<toast>
  <visual>
    <binding template="ToastGeneric">
      <text>{}</text>
      <text>{}</text>
    </binding>
  </visual>
</toast>"#,
            escape_xml(title),
            escape_xml(body),
        );
        xml.LoadXml(&HSTRING::from(&template))?;

        let toast = ToastNotification::CreateToastNotification(&xml)?;
        let notifier = ToastNotificationManager::CreateToastNotifierWithId(&HSTRING::from(AUMID))?;
        notifier.Show(&toast)?;

        tracing::debug!(
            title = title,
            body_len = body.len(),
            "toast notification shown",
        );

        // Execute custom notification command if configured.
        self.run_notification_command(notification);

        Ok(())
    }

    /// Execute a custom command on notification if configured.
    ///
    /// Spawns `cmd /c <command>` with environment variables:
    /// - WMUX_NOTIFICATION_TITLE
    /// - WMUX_NOTIFICATION_BODY
    /// - WMUX_WORKSPACE_ID (if available)
    /// - WMUX_SURFACE_ID (if available)
    fn run_notification_command(&self, notification: &Notification) {
        // Custom command support is config-driven (L3_11/config).
        // For now, check the WMUX_NOTIFICATION_COMMAND env var as a fallback.
        let command = match std::env::var("WMUX_NOTIFICATION_COMMAND") {
            Ok(cmd) if !cmd.is_empty() => cmd,
            _ => return,
        };

        let title = sanitize_env_value(notification.title.as_deref().unwrap_or(""));
        let body = sanitize_env_value(&notification.body);

        let mut cmd = Command::new("cmd");
        cmd.args(["/c", &command])
            .env("WMUX_NOTIFICATION_TITLE", &title)
            .env("WMUX_NOTIFICATION_BODY", &body);

        if let Some(ws) = &notification.source_workspace {
            cmd.env("WMUX_WORKSPACE_ID", ws.to_string());
        }
        if let Some(sf) = &notification.source_surface {
            cmd.env("WMUX_SURFACE_ID", sf.to_string());
        }

        match cmd.spawn() {
            Ok(_) => tracing::debug!(command = %command, "notification command spawned"),
            Err(e) => tracing::warn!(error = %e, command = %command, "notification command failed"),
        }
    }
}

impl Default for ToastService {
    fn default() -> Self {
        Self::new()
    }
}

/// Sanitize a value before passing it as an environment variable to `cmd /c`.
///
/// Strips characters that `cmd.exe` could expand or interpret:
/// `%` (env expansion), `!` (delayed expansion), `|&^<>` (pipe/redirect),
/// and control characters (newlines, etc.).
fn sanitize_env_value(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        if !matches!(c, '%' | '!' | '|' | '&' | '^' | '<' | '>' | '\r' | '\n') {
            out.push(c);
        }
    }
    out
}

/// Escape XML special characters.
fn escape_xml(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escape_xml_special_chars() {
        assert_eq!(escape_xml("a < b & c > d"), "a &lt; b &amp; c &gt; d");
        assert_eq!(escape_xml(r#"say "hello""#), "say &quot;hello&quot;");
        assert_eq!(escape_xml("it's"), "it&apos;s");
    }

    #[test]
    fn sanitize_env_value_strips_cmd_metacharacters() {
        assert_eq!(sanitize_env_value("hello%PATH%world"), "helloPATHworld");
        assert_eq!(sanitize_env_value("a|b&c^d<e>f"), "abcdef");
        assert_eq!(sanitize_env_value("line1\r\nline2"), "line1line2");
        assert_eq!(sanitize_env_value("safe text 123"), "safe text 123");
    }

    #[test]
    fn escape_xml_passthrough() {
        assert_eq!(escape_xml("Hello World"), "Hello World");
        assert_eq!(escape_xml(""), "");
    }

    #[test]
    #[ignore] // Requires Windows desktop session
    fn init_aumid_does_not_panic() {
        init_aumid();
    }

    #[test]
    #[ignore] // Requires Windows desktop session
    fn toast_service_show() {
        init_aumid();
        let svc = ToastService::new();
        let notification = Notification {
            id: wmux_core::NotificationId::new(),
            title: Some("Test".to_string()),
            body: "Hello from wmux".to_string(),
            subtitle: None,
            source: wmux_core::NotificationSource::Internal,
            source_workspace: None,
            source_surface: None,
            timestamp: std::time::SystemTime::now(),
            state: wmux_core::NotificationState::Unread,
        };
        svc.show(&notification);
    }
}
