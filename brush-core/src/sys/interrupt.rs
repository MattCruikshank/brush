//! Interrupting a running command (Ctrl+C), for embedders that have no `SIGINT` (Wasm).
//!
//! The embedder installs a check with [`set_check`]; while it returns true, the shell stops
//! at its next checkpoint — before each command of a list and each loop iteration — with
//! [`ErrorKind::Interrupted`](crate::error::ErrorKind::Interrupted) (exit status 130, nothing
//! printed). The embedder clears its flag once the interrupted command has returned. Without a
//! check (the default), nothing is ever interrupted this way.

static CHECK: std::sync::OnceLock<fn() -> bool> = std::sync::OnceLock::new();

/// Sets the check (the first call wins).
pub fn set_check(check: fn() -> bool) {
    let _ = CHECK.set(check);
}

/// Fails with `Interrupted` if the embedder says so.
pub(crate) fn check() -> Result<(), crate::error::Error> {
    if CHECK.get().is_some_and(|check| check()) {
        return Err(crate::error::ErrorKind::Interrupted.into());
    }
    Ok(())
}
