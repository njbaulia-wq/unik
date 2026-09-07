//! FluxCut XDG-compliant Disk LRU Cache Subsystem.
//!
//! Provides disk persistence for lazily generated video thumbnails and
//! downsampled audio waveform envelopes.

use directories::ProjectDirs;
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};
use thiserror::Error;
use tracing::{debug, info};

#[derive(Error, Debug)]
pub enum CacheError {
    #[error("Cache directory unavailable")]
    DirectoryUnavailable,
    #[error("I/O error during cache operation: {0}")]
    IoError(#[from] std::io::Error),
}

/// Cache manager following XDG specifications for storing thumbnails and waveforms.
#[derive(Debug, Clone)]
pub struct DiskCacheManager {
    base_dir: PathBuf,
    thumbnails_dir: PathBuf,
    waveforms_dir: PathBuf,
}

impl DiskCacheManager {
    /// Initialize cache manager using default XDG cache directory.
    pub fn new() -> Result<Self, CacheError> {
        let dirs = ProjectDirs::from("org", "fluxcut", "FluxCut")
            .ok_or(CacheError::DirectoryUnavailable)?;
        let base_dir = dirs.cache_dir().to_path_buf();
        Self::with_base_dir(base_dir)
    }

    /// Initialize cache manager with a specific base directory (useful for testing or custom configs).
    pub fn with_base_dir(base_dir: PathBuf) -> Result<Self, CacheError> {
        let thumbnails_dir = base_dir.join("thumbnails");
        let waveforms_dir = base_dir.join("waveforms");

        fs::create_dir_all(&thumbnails_dir)?;
        fs::create_dir_all(&waveforms_dir)?;

        debug!(
            base = %base_dir.display(),
            "Initialized FluxCut disk cache manager"
        );

        Ok(Self {
            base_dir,
            thumbnails_dir,
            waveforms_dir,
        })
    }

    /// Return the base cache directory path.
    pub fn base_dir(&self) -> &Path {
        &self.base_dir
    }

    /// Generate a deterministic SHA256 cache key from file attributes and parameters.
    pub fn generate_key(path: &Path, modified_time: u64, file_size: u64, extra: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(path.to_string_lossy().as_bytes());
        hasher.update(modified_time.to_le_bytes());
        hasher.update(file_size.to_le_bytes());
        hasher.update(extra.as_bytes());
        let result = hasher.finalize();
        format!("{:x}", result)
    }

    /// Retrieve a cached thumbnail by key.
    pub fn get_thumbnail(&self, key: &str) -> Option<Vec<u8>> {
        let path = self.thumbnails_dir.join(format!("{}.bin", key));
        fs::read(path).ok()
    }

    pub const DEFAULT_MAX_CACHE_BYTES: u64 = 500 * 1024 * 1024;

    /// Store a thumbnail buffer into the cache and enforce bounded growth.
    pub fn put_thumbnail(&self, key: &str, data: &[u8]) -> Result<(), CacheError> {
        let path = self.thumbnails_dir.join(format!("{}.bin", key));
        fs::write(path, data)?;
        let _ = self.enforce_max_size(Self::DEFAULT_MAX_CACHE_BYTES);
        Ok(())
    }

    /// Retrieve a cached waveform by key.
    pub fn get_waveform(&self, key: &str) -> Option<Vec<u8>> {
        let path = self.waveforms_dir.join(format!("{}.bin", key));
        fs::read(path).ok()
    }

    /// Store a waveform buffer into the cache and enforce bounded growth.
    pub fn put_waveform(&self, key: &str, data: &[u8]) -> Result<(), CacheError> {
        let path = self.waveforms_dir.join(format!("{}.bin", key));
        fs::write(path, data)?;
        let _ = self.enforce_max_size(Self::DEFAULT_MAX_CACHE_BYTES);
        Ok(())
    }

    /// Enforce a maximum cache size by pruning the least recently modified cache files.
    /// Returns the total number of bytes freed.
    pub fn enforce_max_size(&self, max_bytes: u64) -> Result<u64, CacheError> {
        let mut files = Vec::new();
        let mut total_size = 0u64;

        for dir in [&self.thumbnails_dir, &self.waveforms_dir] {
            if let Ok(entries) = fs::read_dir(dir) {
                for entry in entries.flatten() {
                    if let Ok(meta) = entry.metadata() {
                        if meta.is_file() {
                            let len = meta.len();
                            let mtime =
                                meta.modified().unwrap_or(std::time::SystemTime::UNIX_EPOCH);
                            files.push((entry.path(), len, mtime));
                            total_size += len;
                        }
                    }
                }
            }
        }

        if total_size <= max_bytes {
            return Ok(0);
        }

        // Sort files oldest first
        files.sort_by_key(|(_, _, mtime)| *mtime);

        let mut bytes_freed = 0u64;
        for (path, len, _) in files {
            if total_size - bytes_freed <= max_bytes {
                break;
            }
            if fs::remove_file(&path).is_ok() {
                bytes_freed += len;
                debug!(path = %path.display(), freed = len, "Pruned cache artifact");
            }
        }

        Ok(bytes_freed)
    }

    /// Calculate total bytes stored in cache.
    pub fn total_size_bytes(&self) -> u64 {
        let mut total = 0;
        for dir in [&self.thumbnails_dir, &self.waveforms_dir] {
            if let Ok(entries) = fs::read_dir(dir) {
                for entry in entries.flatten() {
                    if let Ok(meta) = entry.metadata() {
                        total += meta.len();
                    }
                }
            }
        }
        total
    }

    /// Clear all thumbnails and waveforms from the cache.
    pub fn clear_all(&self) -> Result<(), CacheError> {
        info!("Clearing all cached media artifacts");
        for dir in [&self.thumbnails_dir, &self.waveforms_dir] {
            if let Ok(entries) = fs::read_dir(dir) {
                for entry in entries.flatten() {
                    let _ = fs::remove_file(entry.path());
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cache_put_and_get() {
        let temp_dir =
            std::env::temp_dir().join(format!("fluxcut_cache_test_{}", std::process::id()));
        let cache =
            DiskCacheManager::with_base_dir(temp_dir.clone()).expect("Failed to init cache");

        let key = DiskCacheManager::generate_key(
            Path::new("/mock/video.mp4"),
            123456,
            999999,
            "thumb_w160_t1000",
        );
        assert!(cache.get_thumbnail(&key).is_none());

        let test_data = vec![1, 2, 3, 4, 5, 42];
        cache
            .put_thumbnail(&key, &test_data)
            .expect("Failed to put thumbnail");

        let loaded = cache
            .get_thumbnail(&key)
            .expect("Thumbnail not found in cache");
        assert_eq!(loaded, test_data);

        assert!(cache.total_size_bytes() > 0);
        cache.clear_all().expect("Failed to clear cache");
        assert_eq!(cache.total_size_bytes(), 0);
        assert!(cache.get_thumbnail(&key).is_none());

        let _ = fs::remove_dir_all(temp_dir);
    }

    #[test]
    fn test_cache_lru_pruning() {
        let temp_dir =
            std::env::temp_dir().join(format!("fluxcut_cache_lru_{}", std::process::id()));
        let cache =
            DiskCacheManager::with_base_dir(temp_dir.clone()).expect("Failed to init cache");

        // Write 3 files of 100 bytes each
        let data = vec![0u8; 100];
        cache.put_thumbnail("k1", &data).unwrap();
        cache.put_thumbnail("k2", &data).unwrap();
        cache.put_thumbnail("k3", &data).unwrap();

        assert_eq!(cache.total_size_bytes(), 300);

        // Enforce max size of 150 bytes -> should prune oldest files down to <= 150
        let freed = cache.enforce_max_size(150).unwrap();
        assert!(freed >= 100);
        assert!(cache.total_size_bytes() <= 150);

        let _ = fs::remove_dir_all(temp_dir);
    }
}
