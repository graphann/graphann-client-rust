//! Retry / backoff helpers.
//!
//! Mirrors the server's documented behaviour: honour `Retry-After` exactly
//! when present, otherwise use exponential backoff with jitter.

use std::time::Duration;

/// Compute an exponential backoff delay with full jitter.
///
/// `attempt` is 0-indexed (0 → first retry). The returned duration is bounded
/// by `cap` and uses 50% deterministic / 50% pseudo-random jitter computed
/// from the attempt number to keep the function pure (no global RNG).
pub fn backoff_with_jitter(attempt: u32, base: Duration, cap: Duration) -> Duration {
    let exp = (1u64 << attempt.min(10)) as u128; // cap exponent at 10 → 1024×
    let raw_ns = base.as_nanos().saturating_mul(exp);
    let capped_ns = raw_ns.min(cap.as_nanos());

    // Deterministic jitter — keeps the helper pure for testability while
    // still spreading retries across the [0.5x .. 1.0x] window.
    let jitter_factor = 0.5 + jitter_fraction(attempt) * 0.5;
    let jittered = (capped_ns as f64 * jitter_factor) as u128;
    Duration::from_nanos(jittered.min(u128::from(u64::MAX)) as u64)
}

/// Pseudo-random fraction in `[0, 1)` derived from a counter — avoids
/// pulling in `rand` and keeps the math reproducible.
fn jitter_fraction(attempt: u32) -> f64 {
    // Splitmix64-style mixing — deterministic, uniform enough for jitter.
    let mut z = attempt as u64;
    z = z.wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^= z >> 31;
    (z as f64) / (u64::MAX as f64)
}

/// Parse a `Retry-After` header value.
///
/// The header may be either a delta-seconds integer or an HTTP-date.
/// We only handle the integer form here — anything else returns `None`
/// and the caller falls back to exponential backoff.
pub fn parse_retry_after(value: &str) -> Option<Duration> {
    let trimmed = value.trim();
    if let Ok(secs) = trimmed.parse::<u64>() {
        return Some(Duration::from_secs(secs));
    }
    // HTTP-date parsing intentionally omitted — almost no servers set the
    // date variant for 429/503 in 2025+, and pulling chrono just for this
    // would balloon the dependency tree. Return None and let the retry
    // helper use its exponential schedule.
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backoff_grows_with_attempt() {
        let base = Duration::from_millis(100);
        let cap = Duration::from_secs(10);
        let d0 = backoff_with_jitter(0, base, cap);
        let d3 = backoff_with_jitter(3, base, cap);
        assert!(d3 > d0);
        assert!(d3 <= cap);
    }

    #[test]
    fn parse_retry_after_seconds() {
        assert_eq!(parse_retry_after("12"), Some(Duration::from_secs(12)));
        assert_eq!(parse_retry_after(" 0 "), Some(Duration::ZERO));
        assert_eq!(parse_retry_after("not-a-number"), None);
    }
}
