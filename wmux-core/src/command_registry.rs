use serde::{Deserialize, Serialize};

/// A command entry in the command palette.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandEntry {
    /// Internal identifier for the command (e.g., "split_right").
    pub id: String,
    /// Display name shown in the palette (e.g., "Split Pane Right").
    pub name: String,
    /// Short description shown below the name.
    pub description: String,
    /// Keyboard shortcut hint (e.g., "Ctrl+D").
    pub shortcut: Option<String>,
}

/// A search result with relevance score.
#[derive(Debug, Clone)]
pub struct SearchResult<'a> {
    pub entry: &'a CommandEntry,
    pub score: u32,
}

/// Registry of all available commands for the command palette.
#[derive(Debug, Default)]
pub struct CommandRegistry {
    commands: Vec<CommandEntry>,
}

impl CommandRegistry {
    /// Create a new empty registry.
    #[must_use]
    pub fn new() -> Self {
        Self {
            commands: Vec::new(),
        }
    }

    /// Create a registry pre-populated with the default wmux commands.
    #[must_use]
    pub fn with_defaults() -> Self {
        let mut reg = Self::new();
        reg.register_defaults();
        reg
    }

    /// Register a single command.
    pub fn register(&mut self, entry: CommandEntry) {
        self.commands.push(entry);
    }

    /// Register all default wmux commands.
    // TODO(L2_16): route all user-visible strings through i18n system
    fn register_defaults(&mut self) {
        let defaults = [
            (
                "split_right",
                "Split Pane Right",
                "Split the focused pane horizontally",
                Some("Ctrl+D"),
            ),
            (
                "split_down",
                "Split Pane Down",
                "Split the focused pane vertically",
                Some("Alt+D"),
            ),
            (
                "close_pane",
                "Close Pane",
                "Close the focused pane",
                Some("Ctrl+W"),
            ),
            (
                "zoom_toggle",
                "Toggle Zoom",
                "Zoom or unzoom the focused pane",
                Some("Ctrl+Shift+Enter"),
            ),
            (
                "focus_up",
                "Focus Up",
                "Move focus to the pane above",
                Some("Alt+Up"),
            ),
            (
                "focus_down",
                "Focus Down",
                "Move focus to the pane below",
                Some("Alt+Down"),
            ),
            (
                "focus_left",
                "Focus Left",
                "Move focus to the pane on the left",
                Some("Alt+Left"),
            ),
            (
                "focus_right",
                "Focus Right",
                "Move focus to the pane on the right",
                Some("Alt+Right"),
            ),
            (
                "new_workspace",
                "New Workspace",
                "Create a new workspace",
                Some("Ctrl+N"),
            ),
            (
                "new_surface",
                "New Tab",
                "Create a new tab in the focused pane",
                Some("Ctrl+T"),
            ),
            (
                "toggle_sidebar",
                "Toggle Sidebar",
                "Show or hide the sidebar",
                Some("Ctrl+B"),
            ),
            (
                "copy",
                "Copy",
                "Copy selection to clipboard",
                Some("Ctrl+Shift+C"),
            ),
            (
                "paste",
                "Paste",
                "Paste from clipboard",
                Some("Ctrl+Shift+V"),
            ),
            ("find", "Find", "Search in terminal content", Some("Ctrl+F")),
            (
                "toggle_notification_panel",
                "Toggle Notifications",
                "Show or hide the notification panel",
                Some("Ctrl+Shift+I"),
            ),
            (
                "jump_last_unread",
                "Jump to Last Unread",
                "Navigate to the last unread notification",
                Some("Ctrl+Shift+U"),
            ),
        ];

        for (id, name, desc, shortcut) in defaults {
            self.commands.push(CommandEntry {
                id: id.to_string(),
                name: name.to_string(),
                description: desc.to_string(),
                shortcut: shortcut.map(|s| s.to_string()),
            });
        }
    }

    /// Search commands with fuzzy matching. Returns results sorted by relevance.
    ///
    /// Scoring: exact prefix match = 100, word-start match = 50, substring = 10.
    #[must_use]
    pub fn search(&self, query: &str) -> Vec<SearchResult<'_>> {
        if query.is_empty() {
            return self
                .commands
                .iter()
                .map(|e| SearchResult { entry: e, score: 1 })
                .collect();
        }

        let query_lower = query.to_lowercase();
        let mut results: Vec<SearchResult<'_>> = self
            .commands
            .iter()
            .filter_map(|entry| {
                let name_lower = entry.name.to_lowercase();
                let desc_lower = entry.description.to_lowercase();
                let id_lower = entry.id.to_lowercase();

                let score = if name_lower.starts_with(&query_lower) {
                    100
                } else if id_lower.starts_with(&query_lower) {
                    90
                } else if name_lower
                    .split_whitespace()
                    .any(|w| w.starts_with(&query_lower))
                {
                    50
                } else if name_lower.contains(&query_lower) || desc_lower.contains(&query_lower) {
                    10
                } else {
                    return None;
                };

                Some(SearchResult { entry, score })
            })
            .collect();

        results.sort_by(|a, b| b.score.cmp(&a.score));
        results
    }

    /// Return all registered commands.
    #[must_use]
    pub fn list_all(&self) -> &[CommandEntry] {
        &self.commands
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_registry() {
        let reg = CommandRegistry::new();
        assert!(reg.list_all().is_empty());
    }

    #[test]
    fn defaults_populated() {
        let reg = CommandRegistry::with_defaults();
        assert!(reg.list_all().len() >= 10);
    }

    #[test]
    fn search_prefix_scores_highest() {
        let reg = CommandRegistry::with_defaults();
        let results = reg.search("Split");
        assert!(!results.is_empty());
        assert_eq!(results[0].score, 100);
        assert!(results[0].entry.name.starts_with("Split"));
    }

    #[test]
    fn search_empty_returns_all() {
        let reg = CommandRegistry::with_defaults();
        let all = reg.list_all().len();
        let results = reg.search("");
        assert_eq!(results.len(), all);
    }

    #[test]
    fn search_no_match() {
        let reg = CommandRegistry::with_defaults();
        let results = reg.search("zzzznonexistent");
        assert!(results.is_empty());
    }

    #[test]
    fn search_case_insensitive() {
        let reg = CommandRegistry::with_defaults();
        let r1 = reg.search("split");
        let r2 = reg.search("SPLIT");
        assert_eq!(r1.len(), r2.len());
    }

    #[test]
    fn search_word_start() {
        let reg = CommandRegistry::with_defaults();
        let results = reg.search("Right");
        assert!(!results.is_empty());
        // "Focus Right" and "Split Pane Right" should match
        assert!(results.iter().any(|r| r.entry.name.contains("Right")));
    }

    #[test]
    fn command_entry_serde() {
        let entry = CommandEntry {
            id: "test".into(),
            name: "Test".into(),
            description: "A test".into(),
            shortcut: Some("Ctrl+T".into()),
        };
        let json = serde_json::to_string(&entry).unwrap();
        let back: CommandEntry = serde_json::from_str(&json).unwrap();
        assert_eq!(back.id, "test");
    }
}
