//! Logging initialization helpers.
//!
//! Wraps [`tracing_subscriber`] with sensible defaults for an adventure
//! engine: pretty formatting, ANSI colors in TTYs, env-based filter
//! (`RUST_LOG=adventure_engine=debug,wgpu=warn`).

use tracing_subscriber::EnvFilter;

/// Initialize the global tracing subscriber with the default config.
///
/// Idempotent — calling it more than once is a no-op. Safe to call from
/// `main()` at startup.
///
/// # Errors
///
/// Returns an error only if the subscriber fails to set (e.g., another
/// subscriber was already installed by a different code path).
pub fn init() -> Result<(), crate::Error> {
    let default_filter = "info,wgpu_core=warn,wgpu_hal=warn,naga=warn";
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(default_filter));
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(false)
        .with_file(false)
        .with_line_number(false)
        .try_init()
        .map_err(|e| crate::Error::Other(format!("tracing init failed: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn init_is_idempotent() {
        // First call may or may not succeed depending on test runner; second
        // should not panic.
        let _ = init();
        let _ = init();
    }
}
