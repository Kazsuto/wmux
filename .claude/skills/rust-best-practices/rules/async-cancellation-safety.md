---
title: Understand Cancellation Safety with tokio::select!
impact: CRITICAL
impactDescription: prevents silent data loss from dropped futures
tags: async, select, cancellation, safety, tokio
---

## Understand Cancellation Safety with tokio::select!

Futures in select! branches that don't win are dropped (cancelled). If they performed partial work before cancellation, that work is silently lost.

**Incorrect (partial reads lost when timeout branch wins):**

```rust
use tokio::io::AsyncReadExt;
use tokio::time::Duration;

async fn read_exact_with_timeout<R: AsyncReadExt + Unpin>(reader: &mut R) -> Option<Vec<u8>> {
    let mut buf = vec![0u8; 1024];

    tokio::select! {
        // read_exact is NOT cancellation safe: if the timeout branch wins,
        // partial bytes may have already been written into buf — and that
        // partial state is silently lost when the future is dropped.
        result = reader.read_exact(&mut buf) => {
            result.unwrap();
            Some(buf)
        }
        _ = tokio::time::sleep(Duration::from_secs(1)) => {
            // buf may contain partial data from read_exact, but we discard it
            None
        }
    }
}
```

**Correct (track buffer state before select!, handle partial reads):**

```rust
use tokio::io::AsyncReadExt;
use tokio::time::Duration;

async fn read_exact_with_timeout<R: AsyncReadExt + Unpin>(reader: &mut R) -> Option<Vec<u8>> {
    let mut buf = vec![0u8; 1024];
    let mut filled = 0usize;

    // Use a cancellation-safe loop with single `read` calls instead of read_exact.
    // `read` IS cancellation safe (guaranteed: no data read if dropped early),
    // while `read_exact` is NOT (partial data may already be in the buffer).
    let sleep = tokio::time::sleep(Duration::from_secs(1));
    tokio::pin!(sleep);

    loop {
        tokio::select! {
            result = reader.read(&mut buf[filled..]) => {
                let n = result.unwrap();
                if n == 0 { return Some(buf[..filled].to_vec()); }
                filled += n;
                if filled == buf.len() {
                    return Some(buf);
                }
            }
            _ = &mut sleep => {
                // Return whatever we have committed so far
                if filled > 0 {
                    return Some(buf[..filled].to_vec());
                }
                return None;
            }
        }
    }
}

// Preferred: use a cancellation-safe wrapper that tracks its own state
async fn read_message<R: AsyncReadExt + Unpin>(reader: &mut R) -> Option<Vec<u8>> {
    let mut buf = Vec::new();
    let mut tmp = [0u8; 256];

    loop {
        tokio::select! {
            result = reader.read(&mut tmp) => {
                let n = result.ok()?;
                if n == 0 { return Some(buf); }
                buf.extend_from_slice(&tmp[..n]);
                // Only commit when we have a complete message
                if buf.ends_with(b"\n") {
                    return Some(buf);
                }
            }
            _ = tokio::time::sleep(std::time::Duration::from_secs(5)) => {
                return None;
            }
        }
    }
}
```

**Note:** Oxide's RFD 400 is the definitive guide on this topic. Check the Tokio documentation for each API's cancellation safety status — it is explicitly documented for all standard I/O and synchronization types. Prefer cancellation-safe abstractions (e.g., tokio::io::BufReader::read_line) when available.

**Important:** When using `select!` in a loop, the `sleep` future resets every iteration. To enforce a total timeout, create the sleep BEFORE the loop and pin it:
```rust
let sleep = tokio::time::sleep(Duration::from_secs(5));
tokio::pin!(sleep);
loop {
    tokio::select! {
        result = reader.read(&mut tmp) => { /* ... */ }
        _ = &mut sleep => { break; } // total timeout, not per-iteration
    }
}
```
