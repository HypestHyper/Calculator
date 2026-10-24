//! For development: with CALCULATOR_TIMING set, print how long each step of
//! start-up took, in milliseconds since the program started.

use std::sync::LazyLock;
use std::time::Instant;

static START: LazyLock<Instant> = LazyLock::new(Instant::now);
static ENABLED: LazyLock<bool> = LazyLock::new(|| std::env::var_os("CALCULATOR_TIMING").is_some());

/// Call first thing in main, so times count from there.
pub fn start() {
    LazyLock::force(&START);
}

pub fn mark(step: &str) {
    if *ENABLED {
        eprintln!(
            "calculator: {:7.1} ms  {step}",
            START.elapsed().as_secs_f64() * 1000.0
        );
    }
}

pub fn enabled() -> bool {
    *ENABLED
}
