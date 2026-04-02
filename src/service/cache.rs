use anyhow::Result;
use bytes::Bytes;
use moka::future::Cache as MokaCache;
use std::path::PathBuf;
use tokio::fs;
use tracing::{debug, instrument};

/// Maximum number of entries in the in-memory cache
const MEMORY_CACHE_CAPACITY: u64 = 1024;

/// Default TTL for memory cache entries (10 minutes), overridable via CACHE_TTL_SECS
fn cache_ttl_secs() -> u64 {
    std::env::var("CACHE_TTL_SECS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(600)
}

/// Disk cache base directory
const DISK_CACHE_DIR: &str = ".cache/images";

/// Two-layer cache: memory (moka) + disk (filesystem).
///
/// ## Lookup order
/// 1. Memory cache (hot, ~µs access)
/// 2. Disk cache (warm, ~ms access)
/// 3. Miss → caller processes the image and calls `put`
#[derive(Clone)]
pub struct ImageCache {
    memory: MokaCache<String, Bytes>,
    disk_dir: PathBuf,
}

impl ImageCache {
    pub async fn new(capacity: u64) -> Result<Self> {
        let disk_dir = PathBuf::from(DISK_CACHE_DIR);
        fs::create_dir_all(&disk_dir).await?;

        let memory = MokaCache::builder()
            .max_capacity(capacity)
            .time_to_live(std::time::Duration::from_secs(cache_ttl_secs()))
            .build();

        Ok(Self { memory, disk_dir })
    }

    pub async fn default_cache() -> Result<Self> {
        Self::new(MEMORY_CACHE_CAPACITY).await
    }

    /// Return the current number of entries in the memory cache.
    pub fn entry_count(&self) -> u64 {
        self.memory.entry_count()
    }

    #[instrument(skip(self), fields(key = %key))]
    pub async fn get(&self, key: &str) -> Option<Bytes> {
        // Memory cache check
        if let Some(data) = self.memory.get(key).await {
            debug!("memory cache HIT");
            return Some(data);
        }

        // Disk cache check
        let disk_path = self.disk_path(key);
        match fs::read(&disk_path).await {
            Ok(data) => {
                debug!("disk cache HIT");
                let bytes = Bytes::from(data);

                self.memory.insert(key.to_string(), bytes.clone()).await;
                Some(bytes)
            }
            Err(_) => {
                debug!("cache MISS");
                None
            }
        }
    }

    #[instrument(skip(self, data), fields(key = %key, size = data.len()))]
    pub async fn put(&self, key: &str, data: Bytes) {
        let disk_path = self.disk_path(key);
        if let Err(e) = fs::write(&disk_path, &data).await {
            tracing::warn!("disk cache write failed for {key}: {e}");
        }

        self.memory.insert(key.to_string(), data).await;
    }

    pub async fn invalidate(&self, key: &str) {
        self.memory.invalidate(key).await;
        let disk_path = self.disk_path(key);
        let _ = fs::remove_file(&disk_path).await;
    }

    fn disk_path(&self, key: &str) -> PathBuf {
        let prefix = &key[..2.min(key.len())];
        let dir = self.disk_dir.join(prefix);
        dir.join(key)
    }

    pub async fn invalidate_by_url(&self, _url: &str) -> u32 {
        let mut count: u32 = 0;

        if let Ok(mut entries) = fs::read_dir(&self.disk_dir).await {
            while let Ok(Some(subdir)) = entries.next_entry().await {
                if subdir.file_type().await.map(|t| t.is_dir()).unwrap_or(false) {
                    if let Ok(mut files) = fs::read_dir(subdir.path()).await {
                        while let Ok(Some(file)) = files.next_entry().await {
                            if let Ok(_) = fs::remove_file(file.path()).await {
                                count += 1;
                            }
                        }
                    }
                }
            }
        }

        self.memory.invalidate_all();
        self.memory.run_pending_tasks().await;

        count
    }

    /// Sweep the disk cache and delete files older than `max_age`.
    /// Returns the number of files deleted.
    pub async fn cleanup_expired_disk_cache(&self, max_age: std::time::Duration) -> u32 {
        let mut count: u32 = 0;
        let now = std::time::SystemTime::now();

        if let Ok(mut entries) = fs::read_dir(&self.disk_dir).await {
            while let Ok(Some(subdir)) = entries.next_entry().await {
                if subdir.file_type().await.map(|t| t.is_dir()).unwrap_or(false) {
                    if let Ok(mut files) = fs::read_dir(subdir.path()).await {
                        while let Ok(Some(file)) = files.next_entry().await {
                            if let Ok(metadata) = file.metadata().await {
                                if let Ok(modified) = metadata.modified() {
                                    if let Ok(age) = now.duration_since(modified) {
                                        if age > max_age {
                                            if let Ok(_) = fs::remove_file(file.path()).await {
                                                count += 1;
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        
        count
    }
}

impl ImageCache {
    pub async fn ensure_disk_subdir(&self, key: &str) -> Result<()> {
        let prefix = &key[..2.min(key.len())];
        let dir = self.disk_dir.join(prefix);
        fs::create_dir_all(&dir).await?;
        Ok(())
    }
}
