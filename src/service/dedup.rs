use anyhow::Result;
use bytes::Bytes;
use std::{
    collections::HashMap,
    future::Future,
    sync::Arc,
};
use tokio::sync::{Mutex, broadcast};
use tracing::{debug, instrument};

/// Singleflight deduplicator: ensures that only one in-flight processing job
/// runs for any given cache key at a time.
///
/// All concurrent callers for the **same key** will block and receive the same
/// result once the first caller completes processing.  This avoids redundant
/// libvips work and prevents thundering herd issues.
///
/// ## Approach
/// - A shared `HashMap<key, broadcast::Sender<Result<Bytes>>>` tracks in-flight work.
/// - First caller for a key inserts a sender and does the work.
/// - Subsequent callers for the same key subscribe to the sender and wait.
/// - After completion, the sender is removed from the map.
#[derive(Clone)]
pub struct DedupManager {
    /// Map of cache-key  →  broadcast channel for the in-flight result
    in_flight: Arc<Mutex<HashMap<String, broadcast::Sender<Result<Bytes, String>>>>>,
}

impl DedupManager {
    pub fn new() -> Self {
        Self {
            in_flight: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Execute `work` exactly once for the given key.
    ///
    /// If another task is already executing `work` for the same key,
    /// this call will wait for that result instead of starting a duplicate job.
    ///
    /// Returns `Err` if the work function returns an error (all waiters receive the error).
    #[instrument(skip(self, work), fields(key = %key))]
    pub async fn run<F, Fut>(&self, key: &str, work: F) -> Result<Bytes>
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = Result<Bytes>>,
    {
        let mut map = self.in_flight.lock().await;

        if let Some(sender) = map.get(key) {
            // Another task is already processing this key — subscribe and wait
            debug!("dedup: waiting for in-flight result");
            let mut rx = sender.subscribe();
            drop(map); // release lock while waiting

            return rx
                .recv()
                .await
                .map_err(|_| anyhow::anyhow!("dedup: in-flight sender dropped"))?
                .map_err(|e| anyhow::anyhow!("dedup: upstream error: {e}"));
        }

        // We are the first task for this key — register and do the work
        let (tx, _) = broadcast::channel::<Result<Bytes, String>>(1);
        map.insert(key.to_string(), tx.clone());
        drop(map); // release lock before doing expensive work

        debug!("dedup: executing work");
        let result = work().await;

        // Broadcast result to all waiters and clean up
        let result_to_broadcast = match &result {
            Ok(b) => Ok(b.clone()),
            Err(e) => Err(e.to_string()),
        };

        // Ignore send errors (no subscribers = fine)
        let _ = tx.send(result_to_broadcast);

        // Remove from in-flight map
        self.in_flight.lock().await.remove(key);

        result
    }
}

impl Default for DedupManager {
    fn default() -> Self {
        Self::new()
    }
}
