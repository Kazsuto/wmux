---
title: Use 'static Deliberately, Not as an Escape Hatch
impact: HIGH
impactDescription: avoids over-restricting API callers
tags: lifetime, static, bounds, api-design
---

## Use 'static Deliberately, Not as an Escape Hatch

`'static` means the reference is valid for the entire program lifetime. Requiring it restricts callers to string literals, leaked memory, or owned types behind a reference. Most APIs do not actually need this guarantee.

**Incorrect ('static lifetime on parameters that don't need it — rejects dynamically created strings):**

```rust
// Only accepts string literals or leaked memory.
// Callers cannot pass a String, a formatted message, or a &str from a local.
fn log_message(msg: &'static str) {
    println!("[LOG] {msg}");
}

// Rejects any closure that captures local references.
fn run_callback(f: impl Fn() + 'static) {
    f();
}

fn caller(name: String) {
    log_message("hello");       // works — literal
    log_message(&name);         // ERROR: `name` does not live long enough
    log_message(name.leak());   // works — but leaks memory to satisfy 'static
}
```

**Correct (use the shortest lifetime that the function actually needs):**

```rust
// Accepts any &str regardless of where the data lives.
fn log_message(msg: &str) {
    println!("[LOG] {msg}");
}

// Accepts closures capturing local references.
fn run_callback(f: impl Fn()) {
    f();
}

// 'static IS correct here: tokio::spawn requires the future to live
// independently on the thread pool — it may outlive the spawning scope.
fn spawn_task(name: String) {
    tokio::spawn(async move {
        // `name` is moved in (owned), satisfying 'static without leaking.
        println!("task: {name}");
    });
}

// 'static IS correct here: type-erased error stored in a Box.
fn make_error() -> Box<dyn std::error::Error + Send + Sync + 'static> {
    "something went wrong".into()
}
```

**Note:** Use `'static` only when data genuinely must live for the entire program: thread-spawned tasks (`tokio::spawn`, `std::thread::spawn`), global statics, and type-erased trait objects stored past the current scope. If you find yourself adding `'static` just to make the compiler happy, reach for owned types (`String`, `Arc<str>`) instead.
