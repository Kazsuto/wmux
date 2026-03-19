---
title: Property-Based Testing with proptest
impact: HIGH
impactDescription: finds edge cases that hand-written tests miss
tags: testing, proptest, property-based, fuzzing, invariants
---

## Property-Based Testing with proptest

Supplement example-based tests with property-based tests that verify invariants across randomly generated inputs. proptest auto-shrinks failing inputs.

**Incorrect (hand-picked examples miss edge cases):**

```rust
#[test]
fn test_sort_example_only() {
    // Only tests vec![3, 1, 2] — misses duplicates, negatives,
    // i32::MIN, empty input, already-sorted input, etc.
    let mut v = vec![3, 1, 2];
    my_sort(&mut v);
    assert_eq!(v, vec![1, 2, 3]);
}
```

**Correct (properties verified over hundreds of random inputs):**

```rust
use proptest::prelude::*;

proptest! {
    #[test]
    fn sort_produces_ordered_output(mut v in prop::collection::vec(any::<i32>(), 0..200)) {
        my_sort(&mut v);
        for window in v.windows(2) {
            prop_assert!(window[0] <= window[1]);
        }
    }

    #[test]
    fn sort_preserves_elements(v in prop::collection::vec(any::<i32>(), 0..200)) {
        let mut sorted = v.clone();
        my_sort(&mut sorted);
        let mut expected = v;
        expected.sort();
        prop_assert_eq!(sorted, expected);
    }
}
```

**Note:** Roundtrip properties (serialize/deserialize) are particularly powerful. Use proptest-derive for custom types.
