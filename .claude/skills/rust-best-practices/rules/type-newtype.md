---
title: Newtype Pattern for Type Safety
impact: HIGH
impactDescription: prevents accidental argument swaps at zero runtime cost
tags: type, newtype, safety, zero-cost
---

## Newtype Pattern for Type Safety

Wrap primitive types to prevent accidental misuse. Two values of the same type but different semantics should be distinct types.

**Incorrect (easy to swap positional arguments of the same primitive type):**

```rust
fn transfer(from: u64, to: u64, amount: f64) {
    println!("Transferring {} from account {} to account {}", amount, from, to);
}

fn main() {
    // Compiles fine — but from and to are silently swapped
    transfer(9999, 1001, 250.0);
}
```

**Correct (distinct types make argument swaps a compile error):**

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct AccountId(u64);

#[derive(Debug, Clone, Copy, PartialEq)]
struct Amount(f64);

fn transfer(from: AccountId, to: AccountId, amount: Amount) {
    println!(
        "Transferring {:?} from {:?} to {:?}",
        amount, from, to
    );
}

fn main() {
    let sender = AccountId(1001);
    let receiver = AccountId(9999);
    let amount = Amount(250.0);

    // transfer(receiver, sender, amount); // compile error: argument order is explicit
    transfer(sender, receiver, amount);
}
```

**Note:** The newtype has zero runtime overhead — it has the same memory representation as the inner type. Add `#[repr(transparent)]` if you need guaranteed ABI compatibility (e.g., FFI). Consider `derive_more` (v2.x) to auto-derive forwarded trait impls (`Display`, `From`, `Add`, etc.) and reduce boilerplate. Avoid implementing `Deref` to the inner type, as it undermines the type safety the newtype provides. Also enables implementing foreign traits on foreign types (orphan rule workaround).
