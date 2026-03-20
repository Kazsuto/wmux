pub mod actor;
pub mod error;
pub mod manager;
pub mod shell;

pub use actor::{PtyActorHandle, PtyEvent};
pub use error::PtyError;
pub use manager::{PtyHandle, PtyManager, SpawnConfig};
pub use shell::{detect_shell, ShellInfo, ShellType};
