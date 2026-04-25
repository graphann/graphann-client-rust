//! Tiny LRU + TTL response cache.
//!
//! Used to elide repeated identical search calls within a configurable
//! window. Disabled by default; opt in via [`crate::ClientBuilder::cache`].

use std::sync::Arc;
use std::time::{Duration, Instant};

use lru::LruCache;
use parking_lot::Mutex;

/// LRU cache with a per-entry TTL.
///
/// `K` and `V` are `Clone` so callers can grab a value out by reference and
/// drop the lock immediately.
pub(crate) struct TtlCache<K, V>
where
    K: std::hash::Hash + Eq + Clone,
    V: Clone,
{
    inner: Arc<Mutex<LruCache<K, (Instant, V)>>>,
    ttl: Duration,
}

impl<K, V> TtlCache<K, V>
where
    K: std::hash::Hash + Eq + Clone,
    V: Clone,
{
    pub(crate) fn new(capacity: std::num::NonZeroUsize, ttl: Duration) -> Self {
        Self {
            inner: Arc::new(Mutex::new(LruCache::new(capacity))),
            ttl,
        }
    }

    pub(crate) fn get(&self, key: &K) -> Option<V> {
        let mut guard = self.inner.lock();
        let now = Instant::now();
        match guard.peek(key) {
            Some((inserted, _)) if now.duration_since(*inserted) > self.ttl => {
                guard.pop(key);
                None
            }
            _ => guard.get(key).map(|(_, v)| v.clone()),
        }
    }

    pub(crate) fn put(&self, key: K, value: V) {
        let mut guard = self.inner.lock();
        guard.put(key, (Instant::now(), value));
    }

    /// Drop every entry — use when an event invalidates everything (e.g.
    /// model swap on the server).
    pub(crate) fn clear(&self) {
        let mut guard = self.inner.lock();
        guard.clear();
    }
}

impl<K, V> Clone for TtlCache<K, V>
where
    K: std::hash::Hash + Eq + Clone,
    V: Clone,
{
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
            ttl: self.ttl,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::num::NonZeroUsize;

    #[test]
    fn put_get_roundtrip() {
        let cache: TtlCache<&str, u32> =
            TtlCache::new(NonZeroUsize::new(8).unwrap(), Duration::from_secs(5));
        cache.put("a", 1);
        assert_eq!(cache.get(&"a"), Some(1));
    }

    #[test]
    fn ttl_expires_entries() {
        let cache: TtlCache<&str, u32> =
            TtlCache::new(NonZeroUsize::new(8).unwrap(), Duration::from_millis(0));
        cache.put("a", 1);
        std::thread::sleep(Duration::from_millis(2));
        assert_eq!(cache.get(&"a"), None);
    }
}
