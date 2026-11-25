//! The wall clock.
//!
//! On `wasm32-unknown-unknown` the standard library has no clock (`SystemTime::now` panics), so
//! the embedder supplies one with [`set_clock`] (METAL: `wasi:clocks/wall-clock`). Until it does,
//! the clock reads the Unix epoch. Everywhere else this is `SystemTime::now`.

use std::time::SystemTime;

#[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
static CLOCK: std::sync::OnceLock<fn() -> SystemTime> = std::sync::OnceLock::new();

/// Sets the clock (only has an effect on `wasm32-unknown-unknown`; the first call wins).
#[allow(unused_variables)]
pub fn set_clock(clock: fn() -> SystemTime) {
    #[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
    let _ = CLOCK.set(clock);
}

/// The current time.
pub fn now() -> SystemTime {
    #[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
    return CLOCK.get().map_or(SystemTime::UNIX_EPOCH, |clock| clock());
    #[cfg(not(all(target_arch = "wasm32", target_os = "unknown")))]
    SystemTime::now()
}

/// The current time in UTC.
pub fn now_utc() -> chrono::DateTime<chrono::Utc> {
    now().into()
}

/// The current local time (UTC on platforms without time zones).
pub fn now_local() -> chrono::DateTime<chrono::Local> {
    now().into()
}
