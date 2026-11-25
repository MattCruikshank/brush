//! Random numbers for `$RANDOM` and `$SRANDOM`.
//!
//! On targets other than Unix and Windows (Wasm) there may be no OS randomness; a generator
//! seeded by the embedder ([`set_seed`], default 0) is used instead, so a shell there is
//! deterministic given its seed.
//! Everywhere else this is `rand::rng()`.

#[cfg(not(any(unix, windows)))]
static RNG: std::sync::Mutex<Option<rand::rngs::StdRng>> = std::sync::Mutex::new(None);

/// Seeds the generator (only has an effect on targets other than Unix and Windows).
#[allow(unused_variables)]
pub fn set_seed(seed: u64) {
    #[cfg(not(any(unix, windows)))]
    {
        use rand::SeedableRng as _;
        *RNG.lock().unwrap_or_else(|e| e.into_inner()) = Some(rand::rngs::StdRng::seed_from_u64(seed));
    }
}

/// Calls `f` with the generator.
pub(crate) fn with_rng<T>(f: impl FnOnce(&mut dyn rand::Rng) -> T) -> T {
    #[cfg(not(any(unix, windows)))]
    {
        use rand::SeedableRng as _;
        let mut rng = RNG.lock().unwrap_or_else(|e| e.into_inner());
        f(rng.get_or_insert_with(|| rand::rngs::StdRng::seed_from_u64(0)))
    }
    #[cfg(any(unix, windows))]
    f(&mut rand::rng())
}
