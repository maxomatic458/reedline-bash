use std::os::raw::c_char;

/// A null-terminated array of strings, as required by `struct builtin.long_doc`.
pub struct DocLines(pub [*const c_char; 3]);

// SAFETY: every pointer is to a `'static` C string literal and is only read.
unsafe impl Sync for DocLines {}

/// Run `body`, turn a panic into `fallback`.
pub fn guard<T>(fallback: T, body: impl FnOnce() -> T + std::panic::UnwindSafe) -> T {
    match std::panic::catch_unwind(body) {
        Ok(value) => value,
        Err(_) => fallback,
    }
}
