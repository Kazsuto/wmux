use std::fmt;
use std::io::{Read, Write};
use std::sync::mpsc as std_mpsc;

use tokio::sync::mpsc;
use windows::Win32::System::Console::HPCON;

use crate::conpty::{resize_by_hpcon, ConPtyHandle};
use crate::error::PtyError;
use crate::manager::PtyHandle;
use crate::spawn::ChildProcess;

/// Events emitted by a PTY actor.
#[derive(Debug, Clone)]
pub enum PtyEvent {
    /// Raw output bytes from the PTY process.
    Output(Vec<u8>),
    /// The PTY process has exited.
    Exited {
        /// Whether the process exited with a success status code.
        success: bool,
    },
}

/// Handle for communicating with a running PTY actor.
///
/// The actor manages async I/O for a single PTY instance using
/// `spawn_blocking` for reads/writes and bounded channels for
/// communication (ADR-0008 actor pattern).
///
/// # Architecture
///
/// Internally, the actor spawns four tasks:
/// - **Reader** (`spawn_blocking`): reads ConPTY output into a 4096-byte buffer,
///   sends [`PtyEvent::Output`] via bounded channel. Includes flood detection.
/// - **Writer** (`spawn_blocking`): receives bytes from write channel,
///   writes to ConPTY input pipe.
/// - **Exit watcher** (`spawn_blocking`): waits for child process exit,
///   then shuts down ConPTY cleanly (Release + Close on 24H2+).
/// - **Resize handler** (`tokio::spawn`): receives resize requests,
///   applies them via `ResizePseudoConsole`.
///
/// All communication is through bounded channels — no `Arc<Mutex<T>>`.
pub struct PtyActorHandle {
    write_tx: mpsc::Sender<Vec<u8>>,
    resize_tx: mpsc::Sender<(u16, u16)>,
    event_rx: mpsc::Receiver<PtyEvent>,
}

impl fmt::Debug for PtyActorHandle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PtyActorHandle")
            .field("write_tx", &"...")
            .field("resize_tx", &"...")
            .finish()
    }
}

impl PtyActorHandle {
    /// Spawn a new PTY actor, taking ownership of the given [`PtyHandle`].
    ///
    /// Returns a handle for sending input, resizing, and receiving events.
    /// The actor runs until all channels are closed or the PTY process exits.
    pub fn spawn(handle: PtyHandle) -> Self {
        let (event_tx, event_rx) = mpsc::channel::<PtyEvent>(256);
        let (write_tx, write_rx) = mpsc::channel::<Vec<u8>>(256);
        let (resize_tx, resize_rx) = mpsc::channel::<(u16, u16)>(4);

        let (reader, writer, child, conpty) = handle.into_parts();

        // Copy the HPCON value for the resize handler (HPCON is Copy — just an isize).
        // The ConPtyHandle is moved into the exit watcher for proper shutdown ordering.
        let hpc = conpty.hpcon();

        // Synchronization: the reader holds reader_done_tx. When the reader
        // exits (EOF or error), it drops the sender. The exit watcher waits
        // on reader_done_rx to ensure all Output events are delivered before
        // the Exited event.
        let (reader_done_tx, reader_done_rx) = std_mpsc::sync_channel::<()>(0);

        // Clone resize_tx so the exit watcher can drop it before ConPTY
        // shutdown, ensuring the resize handler has exited and won't call
        // ResizePseudoConsole on a closed HPCON.
        let resize_tx_for_shutdown = resize_tx.clone();

        Self::spawn_reader(reader, event_tx.clone(), reader_done_tx);
        Self::spawn_writer(writer, write_rx);
        Self::spawn_exit_watcher(
            child,
            conpty,
            event_tx,
            reader_done_rx,
            resize_tx_for_shutdown,
        );
        Self::spawn_resize_handler(hpc, resize_rx);

        Self {
            write_tx,
            resize_tx,
            event_rx,
        }
    }

    /// Send input bytes to the PTY process.
    #[inline]
    pub async fn write(&self, data: Vec<u8>) -> Result<(), PtyError> {
        self.write_tx
            .send(data)
            .await
            .map_err(|_| PtyError::ChannelClosed)
    }

    /// Resize the PTY to the given dimensions.
    ///
    /// `cols` is clamped to a minimum of 2 to prevent ConPTY bug #19922
    /// (a 2-column character on a 1-column terminal causes an infinite loop).
    /// `rows` is clamped to a minimum of 1.
    #[inline]
    pub async fn resize(&self, rows: u16, cols: u16) -> Result<(), PtyError> {
        let cols = cols.max(2);
        let rows = rows.max(1);
        self.resize_tx
            .send((rows, cols))
            .await
            .map_err(|_| PtyError::ChannelClosed)
    }

    /// Receive the next event from the PTY actor.
    ///
    /// Returns `None` when the actor has shut down (all event senders dropped).
    #[inline]
    pub async fn next_event(&mut self) -> Option<PtyEvent> {
        self.event_rx.recv().await
    }

    /// Maximum bytes per second before the reader considers the PTY output
    /// to be flooding (e.g. ConPTY infinite loop bug #19922). 10 MB/s is
    /// well above any realistic terminal output rate.
    const FLOOD_THRESHOLD_BYTES: usize = 10 * 1024 * 1024;

    fn spawn_reader(
        reader: std::fs::File,
        event_tx: mpsc::Sender<PtyEvent>,
        _reader_done: std_mpsc::SyncSender<()>,
    ) {
        // _reader_done is held for its Drop: when this task exits, the sender
        // is dropped, unblocking the exit watcher's recv().
        tokio::task::spawn_blocking(move || {
            let _done_guard = _reader_done;
            let mut reader = reader;
            let mut buf = [0u8; 4096];

            // Flood detection: track bytes received in a 1-second window.
            // Protects against ConPTY bug #19922 where a 2-column character
            // on a 1-column terminal causes an infinite loop emitting \r\n\x20.
            let mut window_start = std::time::Instant::now();
            let mut window_bytes: usize = 0;

            loop {
                match reader.read(&mut buf) {
                    Ok(0) => break,
                    Ok(n) => {
                        // Flood detection: reset window every second.
                        let elapsed = window_start.elapsed();
                        if elapsed >= std::time::Duration::from_secs(1) {
                            window_bytes = 0;
                            window_start = std::time::Instant::now();
                        }
                        window_bytes = window_bytes.saturating_add(n);

                        if window_bytes > Self::FLOOD_THRESHOLD_BYTES {
                            tracing::error!(
                                bytes_per_sec = window_bytes,
                                "pty output flood detected (possible ConPTY bug #19922), killing reader"
                            );
                            break;
                        }

                        if event_tx
                            .blocking_send(PtyEvent::Output(buf[..n].to_vec()))
                            .is_err()
                        {
                            break;
                        }
                    }
                    Err(e) => {
                        tracing::debug!(error = %e, "pty read error");
                        break;
                    }
                }
            }
            tracing::debug!("pty reader task exited");
        });
    }

    fn spawn_writer(writer: std::fs::File, write_rx: mpsc::Receiver<Vec<u8>>) {
        tokio::task::spawn_blocking(move || {
            let mut writer = writer;
            let mut write_rx = write_rx;
            while let Some(data) = write_rx.blocking_recv() {
                if let Err(e) = writer.write_all(&data) {
                    tracing::debug!(error = %e, "pty write error");
                    break;
                }
            }
            let _ = writer.flush();
            tracing::debug!("pty writer task exited");
        });
    }

    fn spawn_exit_watcher(
        child: ChildProcess,
        mut conpty: ConPtyHandle,
        event_tx: mpsc::Sender<PtyEvent>,
        reader_done_rx: std_mpsc::Receiver<()>,
        resize_tx_for_shutdown: mpsc::Sender<(u16, u16)>,
    ) {
        tokio::task::spawn_blocking(move || {
            let success = match child.wait() {
                Ok(success) => success,
                Err(e) => {
                    tracing::warn!(error = %e, "failed to wait for pty child process");
                    false
                }
            };

            // Wait for the reader to drain all remaining output before
            // sending the Exited event, so consumers never see Exited
            // before the final Output chunks.
            let _ = reader_done_rx.recv();

            // Drop the resize channel sender to ensure the resize handler
            // exits BEFORE we close the HPCON. This prevents use-after-close
            // of the raw HPCON in the resize handler.
            drop(resize_tx_for_shutdown);

            // Shut down ConPTY cleanly. On 24H2+ this calls
            // ReleasePseudoConsole then ClosePseudoConsole. On older
            // Windows, ClosePseudoConsole blocks here (safe in spawn_blocking).
            conpty.shutdown();

            let _ = event_tx.blocking_send(PtyEvent::Exited { success });
            tracing::debug!("pty exit watcher exited");
        });
    }

    fn spawn_resize_handler(hpc: HPCON, mut resize_rx: mpsc::Receiver<(u16, u16)>) {
        tokio::spawn(async move {
            while let Some((rows, cols)) = resize_rx.recv().await {
                if let Err(e) = resize_by_hpcon(hpc, cols, rows) {
                    tracing::warn!(rows, cols, error = %e, "pty resize failed");
                }
            }
            tracing::debug!("pty resize handler exited");
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn _assert_send<T: Send>() {}

    #[test]
    fn pty_event_is_send() {
        _assert_send::<PtyEvent>();
    }

    #[test]
    fn pty_actor_handle_is_send() {
        _assert_send::<PtyActorHandle>();
    }
}
