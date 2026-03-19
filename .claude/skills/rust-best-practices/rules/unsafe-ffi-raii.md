---
title: Wrap FFI in RAII-Based Safe Abstractions
impact: CRITICAL
impactDescription: prevents resource leaks and double-free via automatic Drop cleanup
tags: unsafe, ffi, raii, drop, resource-management
---

## Wrap FFI in RAII-Based Safe Abstractions

Never expose raw FFI pointers. Wrap foreign resources in a struct that implements `Drop` for automatic cleanup. Validate all data crossing the FFI boundary.

**Incorrect (manual open/close — leaks on panic, no use-after-free protection):**

```rust
use std::ffi::CString;

// Hypothetical C library bindings.
mod ffi {
    #[repr(C)]
    pub struct DbHandle { _private: [u8; 0] }

    extern "C" {
        pub fn db_open(path: *const std::ffi::c_char) -> *mut DbHandle;
        pub fn db_close(handle: *mut DbHandle);
        pub fn db_query(handle: *mut DbHandle, sql: *const std::ffi::c_char) -> i32;
    }
}

fn run_query(path: &str, sql: &str) -> i32 {
    let c_path = CString::new(path).unwrap();
    let c_sql = CString::new(sql).unwrap();

    // If db_query panics, db_close is never called — resource leak.
    let handle = unsafe { ffi::db_open(c_path.as_ptr()) };
    let result = unsafe { ffi::db_query(handle, c_sql.as_ptr()) };

    // Forgotten call = leak. Called twice = double-free.
    unsafe { ffi::db_close(handle) };
    result
}
```

**Correct (RAII wrapper with automatic Drop — panic-safe, no double-free):**

```rust
use std::ffi::CString;
use std::ptr::NonNull;

// Hypothetical C library bindings.
mod ffi {
    #[repr(C)]
    pub struct DbHandle { _private: [u8; 0] }

    extern "C" {
        pub fn db_open(path: *const std::ffi::c_char) -> *mut DbHandle;
        pub fn db_close(handle: *mut DbHandle);
        pub fn db_query(handle: *mut DbHandle, sql: *const std::ffi::c_char) -> i32;
    }
}

/// Safe RAII wrapper around a raw C database handle.
///
/// The handle is guaranteed to be non-null after construction and is
/// automatically closed when `Database` is dropped, even on panic.
pub struct Database {
    // NonNull documents and enforces the non-null invariant.
    handle: NonNull<ffi::DbHandle>,
}

impl Database {
    /// Opens a database at the given path.
    ///
    /// Returns `None` if the C library returns a null pointer.
    pub fn open(path: &str) -> Option<Self> {
        let c_path = CString::new(path).ok()?;
        // SAFETY: db_open is called with a valid, null-terminated C string.
        // We immediately check for null before constructing the wrapper.
        let raw = unsafe { ffi::db_open(c_path.as_ptr()) };
        let handle = NonNull::new(raw)?;
        Some(Self { handle })
    }

    /// Executes a SQL query, returning the result code.
    pub fn query(&self, sql: &str) -> Option<i32> {
        let c_sql = CString::new(sql).ok()?;
        // SAFETY: self.handle is non-null and valid (maintained by the
        // constructor invariant). The CString lives for the duration of
        // this call. db_query does not take ownership of the handle.
        Some(unsafe { ffi::db_query(self.handle.as_ptr(), c_sql.as_ptr()) })
    }
}

impl Drop for Database {
    fn drop(&mut self) {
        // SAFETY: self.handle is non-null and was obtained from db_open.
        // Drop is called exactly once, so db_close is called exactly once.
        unsafe { ffi::db_close(self.handle.as_ptr()) }
    }
}

fn run_query(path: &str, sql: &str) -> Option<i32> {
    // db_close is called automatically when `db` goes out of scope,
    // whether by normal return or by panic unwinding.
    let db = Database::open(path)?;
    db.query(sql)
}
```

**Note:** Implement `Send` and `Sync` for your wrapper manually only if the underlying C library is documented as thread-safe. If thread safety is unclear, use `PhantomData<*mut ()>` as a field — raw pointers are neither `Send` nor `Sync`, so the compiler will refuse to allow the wrapper across thread boundaries, preventing data races at compile time. **Edition note:** Starting with Rust 2024 (edition = "2024"), `extern` blocks must be written as `unsafe extern "C" { ... }`. Individual items inside can be marked `safe` or `unsafe`; the default is `unsafe`.
