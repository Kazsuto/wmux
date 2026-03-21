pub mod auth;
pub mod error;
pub mod handler;
pub mod handlers;
pub mod protocol;
pub mod router;
pub mod server;

pub use auth::{ConnectionCtx, SecurityMode};
pub use error::IpcError;
pub use handler::{Handler, RpcError};
pub use protocol::{RpcErrorCode, RpcRequest, RpcResponse};
pub use router::Router;
pub use server::{pipe_name, IpcServer, IpcServerHandle};
