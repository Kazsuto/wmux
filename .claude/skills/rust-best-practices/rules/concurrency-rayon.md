---
title: Use Rayon par_iter() for CPU-Bound Data Parallelism
impact: HIGH
impactDescription: correct tool for CPU-bound work vs tokio which is for I/O
tags: concurrency, rayon, parallelism, cpu-bound, iterators
---

## Use Rayon par_iter() for CPU-Bound Data Parallelism

Tokio is for I/O-bound concurrency. For CPU-bound parallel computation, Rayon's work-stealing thread pool is the correct tool. Bridge with spawn_blocking when combining with async code.

**Incorrect (tokio::spawn for CPU-bound image resizing — blocks Tokio worker threads):**

```rust
use std::time::Duration;

struct Image { data: Vec<u8>, width: u32, height: u32 }

impl Image {
    fn resize(&self, width: u32, height: u32) -> Image {
        // Simulate CPU-heavy work
        std::thread::sleep(Duration::from_millis(5));
        Image { data: vec![0; (width * height * 3) as usize], width, height }
    }
}

async fn process_images_wrong(images: Vec<Image>) -> Vec<Image> {
    // tokio::spawn puts work on Tokio's I/O thread pool.
    // CPU-bound tasks starve the executor, blocking all I/O and timer events
    // on those threads — the entire runtime degrades under load.
    let tasks: Vec<_> = images.into_iter().map(|img| {
        tokio::spawn(async move { img.resize(128, 128) })
    }).collect();

    let mut results = Vec::new();
    for t in tasks {
        results.push(t.await.unwrap());
    }
    results
}
```

**Correct (spawn_blocking bridges async to Rayon's CPU thread pool):**

```rust
use std::time::Duration;
use rayon::prelude::*;

struct Image { data: Vec<u8>, width: u32, height: u32 }

impl Image {
    fn resize(&self, width: u32, height: u32) -> Image {
        std::thread::sleep(Duration::from_millis(5));
        Image { data: vec![0; (width * height * 3) as usize], width, height }
    }
}

async fn process_images_correct(images: Vec<Image>) -> Vec<Image> {
    // spawn_blocking moves work off the Tokio thread pool entirely.
    // Rayon distributes it across all CPU cores using work-stealing.
    // Tokio threads remain free to handle I/O and timers.
    tokio::task::spawn_blocking(move || {
        images
            .par_iter()                     // Rayon parallel iterator
            .map(|img| img.resize(128, 128))
            .collect()
    })
    .await
    .unwrap()
}

// Mixing async I/O with CPU processing: fetch then process
async fn fetch_and_process(urls: Vec<&str>) -> Vec<usize> {
    // Step 1: async I/O — fetch all images concurrently with Tokio
    let raw_images: Vec<Vec<u8>> = futures::future::join_all(
        urls.iter().map(|_url| async {
            tokio::fs::read("image.jpg").await.unwrap_or_default()
        })
    ).await;

    // Step 2: CPU work — process in parallel with Rayon
    tokio::task::spawn_blocking(move || {
        raw_images
            .par_iter()
            .map(|data| data.len()) // replace with actual CPU work
            .collect()
    })
    .await
    .unwrap()
}
```

**Note:** Rayon defaults to num_cpus threads (one per logical core). Customize the pool with rayon::ThreadPoolBuilder::new().num_threads(4).build_global().unwrap() at startup. For fine-grained control or to avoid contending with other Rayon users, build a custom ThreadPool rather than using the global one.
