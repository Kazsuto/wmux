---
title: Sealed Trait Pattern for Forward Compatibility
impact: MEDIUM
impactDescription: allows adding methods to public traits without breaking changes
tags: type, sealed, trait, api-design, semver
---

## Sealed Trait Pattern for Forward Compatibility

When a public trait should not be implemented externally, require a private supertrait. This preserves forward compatibility by allowing new methods to be added without breaking downstream crates.

**Incorrect (public trait with no seal — adding methods is a breaking semver change):**

```rust
// In your library crate:
pub trait DatabaseDriver {
    fn execute(&self, query: &str) -> Vec<String>;
    // Adding any new method here later is a breaking change for all implementors
}

pub struct PostgresDriver;

impl DatabaseDriver for PostgresDriver {
    fn execute(&self, query: &str) -> Vec<String> {
        vec![format!("pg result for: {}", query)]
    }
}

// External users can implement this trait, preventing you from adding methods
```

**Correct (sealed trait — external code can use but not implement it):**

```rust
// In your library crate:
mod private {
    pub trait Sealed {}
}

// Public trait requires the private supertrait — external crates cannot implement Sealed
pub trait DatabaseDriver: private::Sealed {
    fn execute(&self, query: &str) -> Vec<String>;
    fn ping(&self) -> bool {
        true // default impl — adding this later is non-breaking
    }
}

pub struct PostgresDriver;
pub struct SqliteDriver;

// Only types in this crate can implement Sealed, and therefore DatabaseDriver
impl private::Sealed for PostgresDriver {}
impl private::Sealed for SqliteDriver {}

impl DatabaseDriver for PostgresDriver {
    fn execute(&self, query: &str) -> Vec<String> {
        vec![format!("pg result for: {}", query)]
    }
}

impl DatabaseDriver for SqliteDriver {
    fn execute(&self, query: &str) -> Vec<String> {
        vec![format!("sqlite result for: {}", query)]
    }
}

fn run(driver: &impl DatabaseDriver) {
    println!("{:?}", driver.execute("SELECT 1"));
    println!("ping: {}", driver.ping());
}

fn main() {
    run(&PostgresDriver);
    run(&SqliteDriver);
}
```

**Note:** Used in the standard library — `SliceIndex` is sealed to allow future additions. External users can freely call methods on `DatabaseDriver` values and accept them as trait bounds, but cannot provide new implementations, giving the library author freedom to evolve the trait. Since Rust 1.78, you can add `#[diagnostic::on_unimplemented(message = "this trait is sealed and cannot be implemented outside this crate")]` to the sealed trait to provide a clear error message when external users accidentally try to implement it.
