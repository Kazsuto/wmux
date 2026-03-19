---
title: Implement From, Not Into
impact: HIGH
impactDescription: follows API guidelines and gets Into for free via blanket impl
tags: type, from, into, conversion, api-guidelines
---

## Implement From, Not Into

Per Rust API Guidelines (C-CONV-TRAITS): implement `From` and `TryFrom`. The standard library provides blanket `Into`/`TryInto` implementations automatically.

**Incorrect (implementing `Into` directly — wrong direction, no blanket `From`):**

```rust
#[derive(Debug)]
struct Celsius(f64);

#[derive(Debug)]
struct Fahrenheit(f64);

// Wrong: implement Into<Fahrenheit> for Celsius — goes against the convention
impl Into<Fahrenheit> for Celsius {
    fn into(self) -> Fahrenheit {
        Fahrenheit(self.0 * 9.0 / 5.0 + 32.0)
    }
}

fn main() {
    let boiling = Celsius(100.0);
    let f: Fahrenheit = boiling.into();
    println!("{:?}", f); // works, but From<Celsius> for Fahrenheit is not available
    // Fahrenheit::from(Celsius(0.0)); // compile error
}
```

**Correct (implement `From` — `Into` is provided for free via the blanket impl):**

```rust
#[derive(Debug)]
struct Celsius(f64);

#[derive(Debug)]
struct Fahrenheit(f64);

// Correct: implement From<Celsius> for Fahrenheit
impl From<Celsius> for Fahrenheit {
    fn from(c: Celsius) -> Self {
        Fahrenheit(c.0 * 9.0 / 5.0 + 32.0)
    }
}

#[derive(Debug, PartialEq)]
struct EvenNumber(i32);

#[derive(Debug)]
struct OddNumberError(i32);

// For fallible conversions, implement TryFrom
impl TryFrom<i32> for EvenNumber {
    type Error = OddNumberError;

    fn try_from(value: i32) -> Result<Self, Self::Error> {
        if value % 2 == 0 {
            Ok(EvenNumber(value))
        } else {
            Err(OddNumberError(value))
        }
    }
}

fn main() {
    // From is explicit and clear
    let f = Fahrenheit::from(Celsius(100.0));
    println!("{:?}", f);

    // Into comes for free — no extra impl needed
    let f: Fahrenheit = Celsius(0.0).into();
    println!("{:?}", f);

    // TryFrom / TryInto for fallible conversions
    let even = EvenNumber::try_from(4).unwrap();
    println!("{:?}", even);

    let result = EvenNumber::try_from(3);
    assert!(result.is_err());
}
```

**Note:** Follow the naming convention for conversion methods: `as_` for free reference-to-reference conversions (e.g., `as_str()`); `to_` for potentially expensive owned conversions (e.g., `to_string()`); `into_` for consuming conversions (e.g., `into_bytes()`).
