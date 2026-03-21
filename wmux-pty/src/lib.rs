pub mod actor;
pub mod conpty;
pub mod error;
pub mod manager;
pub mod shell;
pub mod spawn;

pub use actor::{PtyActorHandle, PtyEvent};
pub use conpty::ConPtyHandle;
pub use error::PtyError;
pub use manager::{PtyHandle, PtyManager, SpawnConfig};
pub use shell::{detect_shell, ShellInfo, ShellType};
pub use spawn::ChildProcess;
