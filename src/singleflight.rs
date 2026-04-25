//! Tokio-based single-flight coalescer.
//!
//! When N concurrent callers issue identical requests, only the first
//! actually performs the work; the rest await the same future and receive
//! a cloned result. Inspired by Go's `singleflight`, kept tiny.

use std::collections::HashMap;
use std::future::Future;
use std::hash::Hash;
use std::sync::Arc;

use tokio::sync::{broadcast, Mutex};

/// Channel sender used to fan-out the leader's result to every waiting
/// caller. Aliased so the surrounding generic does not trip
/// `clippy::type_complexity`.
type FlightSender<V> = broadcast::Sender<Result<V, String>>;
/// Inner shared map type.
type FlightMap<K, V> = Arc<Mutex<HashMap<K, FlightSender<V>>>>;

/// A key-keyed coalescer.
pub(crate) struct SingleFlight<K, V>
where
    K: Hash + Eq + Clone + Send + 'static,
    V: Clone + Send + 'static,
{
    inner: FlightMap<K, V>,
}

impl<K, V> SingleFlight<K, V>
where
    K: Hash + Eq + Clone + Send + 'static,
    V: Clone + Send + 'static,
{
    pub(crate) fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Run `f()` for `key` if no in-flight call exists, otherwise await
    /// the existing one. Errors are propagated as `String` (because
    /// `crate::Error` is not `Clone`); the caller is expected to map
    /// the string back into a domain error.
    pub(crate) async fn do_call<F, Fut>(&self, key: K, f: F) -> Result<V, String>
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = Result<V, String>> + Send,
    {
        // Fast-path: existing flight exists, hop on it.
        let maybe_rx = {
            let mut map = self.inner.lock().await;
            if let Some(tx) = map.get(&key) {
                Some(tx.subscribe())
            } else {
                let (tx, _) = broadcast::channel(1);
                map.insert(key.clone(), tx);
                None
            }
        };

        if let Some(mut rx) = maybe_rx {
            return match rx.recv().await {
                Ok(res) => res,
                Err(e) => Err(format!("singleflight broadcast error: {e}")),
            };
        }

        // We're the leader: run `f`, broadcast, remove the slot.
        let res = f().await;

        let tx = {
            let mut map = self.inner.lock().await;
            map.remove(&key)
        };
        if let Some(tx) = tx {
            // Receivers might have all dropped — that's fine.
            let _ = tx.send(res.clone());
        }
        res
    }
}

impl<K, V> Clone for SingleFlight<K, V>
where
    K: Hash + Eq + Clone + Send + 'static,
    V: Clone + Send + 'static,
{
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::Duration;

    #[tokio::test]
    async fn coalesces_concurrent_calls() {
        let sf: SingleFlight<&str, u32> = SingleFlight::new();
        let counter = Arc::new(AtomicUsize::new(0));

        let mut handles = Vec::new();
        for _ in 0..10 {
            let sf = sf.clone();
            let counter = counter.clone();
            handles.push(tokio::spawn(async move {
                sf.do_call("k", || async move {
                    tokio::time::sleep(Duration::from_millis(20)).await;
                    counter.fetch_add(1, Ordering::SeqCst);
                    Ok::<u32, String>(42)
                })
                .await
            }));
        }

        for h in handles {
            assert_eq!(h.await.unwrap().unwrap(), 42);
        }
        // Only one actual execution.
        assert_eq!(counter.load(Ordering::SeqCst), 1);
    }
}
