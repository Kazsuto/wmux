---
title: Never Use () as an Error Type
impact: HIGH
impactDescription: enables meaningful error messages and error chain participation
tags: error, unit-type, api-guidelines, display
---

## Never Use () as an Error Type

Per Rust API Guidelines (C-GOOD-ERR): `()` cannot implement `Display`, `Error`, or be used with `From<()>`. It produces useless panic messages and cannot participate in error chains.

**Incorrect (() as error type — no information on failure, cannot use ? in anyhow context):**

```rust
fn parse_positive(s: &str) -> Result<u32, ()> {
    let n: u32 = s.parse().map_err(|_| ())?;
    if n == 0 {
        return Err(());
    }
    Ok(n)
}

fn caller() {
    // Panics with: "called `Result::unwrap()` on an `Err` value: ()"
    // Completely useless. Cannot add context. Cannot display in logs.
    let n = parse_positive("abc").unwrap();
}
```

**Correct (named error type with Debug + Display + Error — even without data, the name carries meaning):**

```rust
use std::fmt;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ParsePositiveError {
    #[error("'{input}' is not a valid unsigned integer")]
    InvalidInteger {
        input: String,
        #[source]
        source: std::num::ParseIntError,
    },
    #[error("value must be positive, got zero")]
    Zero,
}

fn parse_positive(s: &str) -> Result<u32, ParsePositiveError> {
    let n: u32 = s.parse().map_err(|e| ParsePositiveError::InvalidInteger {
        input: s.to_string(),
        source: e,
    })?;
    if n == 0 {
        return Err(ParsePositiveError::Zero);
    }
    Ok(n)
}

// For truly zero-data errors, a unit struct still beats ():
#[derive(Debug, Error)]
#[error("operation is not permitted")]
pub struct NotPermitted;
```

**Note:** Even zero-data errors deserve a named type for meaningful `Debug`/`Display` output. A unit struct error costs nothing at runtime but makes logs and panics immediately actionable. The `thiserror` crate makes this nearly zero-boilerplate.
