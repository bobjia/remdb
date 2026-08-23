//! Inference result cache
//!
//! This module provides an LRU cache for model inference results,
//! reducing redundant computations for identical inputs.

use alloc::collections::VecDeque;
use alloc::string::String;
use alloc::vec::Vec;
use core::hash::{Hash, Hasher};
use std::collections::HashMap;
use std::sync::{Arc, Mutex, RwLock};

#[cfg(feature = "log")]
use crate::log::{debug, info, warn};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CacheStats {
    pub hits: u64,
    pub misses: u64,
    pub evictions: u64,
    pub current_size: usize,
    pub max_size: usize,
}

impl CacheStats {
    pub fn hit_rate(&self) -> f64 {
        let total = self.hits + self.misses;
        if total == 0 {
            0.0
        } else {
            self.hits as f64 / total as f64
        }
    }
}

impl Default for CacheStats {
    fn default() -> Self {
        Self {
            hits: 0,
            misses: 0,
            evictions: 0,
            current_size: 0,
            max_size: 1000,
        }
    }
}

#[derive(Debug, Clone)]
pub struct CacheConfig {
    pub max_entries: usize,
    pub max_memory_mb: usize,
    pub ttl_seconds: Option<u64>,
    pub enabled: bool,
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            max_entries: 1000,
            max_memory_mb: 256,
            ttl_seconds: Some(3600),
            enabled: true,
        }
    }
}

#[derive(Debug, Clone)]
pub struct CacheEntry {
    pub output: Vec<f32>,
    pub created_at: std::time::Instant,
    pub access_count: u64,
    pub memory_size: usize,
}

impl CacheEntry {
    pub fn new(output: Vec<f32>) -> Self {
        let memory_size = output.len() * core::mem::size_of::<f32>();
        Self {
            output,
            created_at: std::time::Instant::now(),
            access_count: 1,
            memory_size,
        }
    }

    pub fn is_expired(&self, ttl_seconds: u64) -> bool {
        self.created_at.elapsed().as_secs() > ttl_seconds
    }

    pub fn touch(&mut self) {
        self.access_count += 1;
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CacheKey {
    pub model_name: String,
    pub input_hash: u64,
}

impl CacheKey {
    pub fn new(model_name: String, inputs: &[Vec<f32>]) -> Self {
        let input_hash = Self::hash_inputs(inputs);
        Self {
            model_name,
            input_hash,
        }
    }

    fn hash_inputs(inputs: &[Vec<f32>]) -> u64 {
        let mut hasher = FxHasher::default();
        inputs.len().hash(&mut hasher);
        for input in inputs {
            input.len().hash(&mut hasher);
            for val in input {
                val.to_bits().hash(&mut hasher);
            }
        }
        hasher.finish()
    }
}

#[derive(Default)]
struct FxHasher {
    hash: usize,
}

impl Hasher for FxHasher {
    fn finish(&self) -> u64 {
        self.hash as u64
    }

    fn write(&mut self, bytes: &[u8]) {
        for byte in bytes {
            self.hash = self.hash.rotate_left(5) ^ (*byte as usize);
            self.hash = self.hash.wrapping_mul(0x517cc1b727220a95);
        }
    }
}

pub struct InferenceCache {
    entries: HashMap<CacheKey, CacheEntry>,
    access_order: VecDeque<CacheKey>,
    config: CacheConfig,
    stats: CacheStats,
    current_memory: usize,
}

impl InferenceCache {
    pub fn new(config: CacheConfig) -> Self {
        let max_size = config.max_entries;
        Self {
            entries: HashMap::with_capacity(max_size),
            access_order: VecDeque::with_capacity(max_size),
            config,
            stats: CacheStats {
                max_size,
                ..Default::default()
            },
            current_memory: 0,
        }
    }

    pub fn get(&mut self, key: &CacheKey) -> Option<&CacheEntry> {
        if !self.config.enabled {
            return None;
        }

        if let Some(entry) = self.entries.get_mut(key) {
            if let Some(ttl) = self.config.ttl_seconds {
                if entry.is_expired(ttl) {
                    self.remove_entry(key);
                    self.stats.misses += 1;
                    return None;
                }
            }
            entry.touch();
            self.stats.hits += 1;

            self.update_access_order(key);

            Some(self.entries.get(key)?)
        } else {
            self.stats.misses += 1;
            None
        }
    }

    pub fn put(&mut self, key: CacheKey, output: Vec<f32>) {
        if !self.config.enabled {
            return;
        }

        if self.entries.contains_key(&key) {
            return;
        }

        let entry = CacheEntry::new(output);
        let entry_memory = entry.memory_size;

        while self.entries.len() >= self.config.max_entries
            || self.current_memory + entry_memory > self.config.max_memory_mb * 1024 * 1024
        {
            if !self.evict_lru() {
                break;
            }
        }

        self.current_memory += entry_memory;
        self.access_order.push_back(key.clone());
        self.entries.insert(key, entry);
        self.stats.current_size = self.entries.len();

        #[cfg(feature = "log")]
        debug!(
            "Cache put: {} entries, {} bytes",
            self.entries.len(),
            self.current_memory
        );
    }

    pub fn remove(&mut self, key: &CacheKey) -> bool {
        if self.entries.remove(key).is_some() {
            self.access_order.retain(|k| k != key);
            self.stats.current_size = self.entries.len();
            true
        } else {
            false
        }
    }

    fn remove_entry(&mut self, key: &CacheKey) {
        if let Some(entry) = self.entries.remove(key) {
            self.current_memory -= entry.memory_size;
            self.access_order.retain(|k| k != key);
            self.stats.current_size = self.entries.len();
        }
    }

    fn evict_lru(&mut self) -> bool {
        if let Some(lru_key) = self.access_order.pop_front() {
            if let Some(entry) = self.entries.remove(&lru_key) {
                self.current_memory -= entry.memory_size;
                self.stats.evictions += 1;
                self.stats.current_size = self.entries.len();

                #[cfg(feature = "log")]
                debug!("Cache evicted LRU entry for model: {}", lru_key.model_name);

                return true;
            }
        }
        false
    }

    fn update_access_order(&mut self, key: &CacheKey) {
        self.access_order.retain(|k| k != key);
        self.access_order.push_back(key.clone());
    }

    pub fn clear(&mut self) {
        self.entries.clear();
        self.access_order.clear();
        self.current_memory = 0;
        self.stats.current_size = 0;

        #[cfg(feature = "log")]
        info!("Cache cleared");
    }

    pub fn stats(&self) -> CacheStats {
        self.stats
    }

    pub fn is_enabled(&self) -> bool {
        self.config.enabled
    }

    pub fn set_enabled(&mut self, enabled: bool) {
        self.config.enabled = enabled;
        if !enabled {
            self.clear();
        }
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn memory_usage(&self) -> usize {
        self.current_memory
    }

    pub fn prune_expired(&mut self) -> usize {
        let ttl = match self.config.ttl_seconds {
            Some(t) => t,
            None => return 0,
        };

        let expired_keys: Vec<CacheKey> = self
            .entries
            .iter()
            .filter(|(_, entry)| entry.is_expired(ttl))
            .map(|(key, _)| key.clone())
            .collect();

        let count = expired_keys.len();
        for key in expired_keys {
            self.remove_entry(&key);
        }

        if count > 0 {
            #[cfg(feature = "log")]
            info!("Pruned {} expired cache entries", count);
        }

        count
    }
}

pub struct ThreadSafeCache {
    inner: Arc<Mutex<InferenceCache>>,
}

impl ThreadSafeCache {
    pub fn new(config: CacheConfig) -> Self {
        Self {
            inner: Arc::new(Mutex::new(InferenceCache::new(config))),
        }
    }

    pub fn get(&self, key: &CacheKey) -> Option<CacheEntry> {
        self.inner.lock().ok()?.get(key).cloned()
    }

    pub fn put(&self, key: CacheKey, output: Vec<f32>) {
        if let Ok(mut cache) = self.inner.lock() {
            cache.put(key, output);
        }
    }

    pub fn remove(&self, key: &CacheKey) -> bool {
        self.inner
            .lock()
            .map(|mut c| c.remove(key))
            .unwrap_or(false)
    }

    pub fn clear(&self) {
        if let Ok(mut cache) = self.inner.lock() {
            cache.clear();
        }
    }

    pub fn stats(&self) -> Option<CacheStats> {
        self.inner.lock().ok().map(|c| c.stats())
    }

    pub fn is_enabled(&self) -> bool {
        self.inner.lock().map(|c| c.is_enabled()).unwrap_or(false)
    }

    pub fn set_enabled(&self, enabled: bool) {
        if let Ok(mut cache) = self.inner.lock() {
            cache.set_enabled(enabled);
        }
    }

    pub fn prune_expired(&self) -> usize {
        self.inner
            .lock()
            .map(|mut c| c.prune_expired())
            .unwrap_or(0)
    }
}

impl Clone for ThreadSafeCache {
    fn clone(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
        }
    }
}

lazy_static::lazy_static! {
    static ref GLOBAL_CACHE: RwLock<Option<ThreadSafeCache>> = RwLock::new(None);
}

pub fn init_cache(config: CacheConfig) -> Result<(), String> {
    let mut cache = GLOBAL_CACHE
        .write()
        .map_err(|_| "Failed to acquire cache lock")?;

    *cache = Some(ThreadSafeCache::new(config));

    #[cfg(feature = "log")]
    info!("Global inference cache initialized");

    Ok(())
}

pub fn get_cache() -> Option<ThreadSafeCache> {
    GLOBAL_CACHE.read().ok()?.as_ref().cloned()
}

pub fn get_or_init_cache() -> ThreadSafeCache {
    if let Some(cache) = get_cache() {
        return cache;
    }

    let config = CacheConfig::default();
    let cache = ThreadSafeCache::new(config);

    if let Ok(mut guard) = GLOBAL_CACHE.write() {
        *guard = Some(cache.clone());
    }

    cache
}

pub fn clear_cache() {
    if let Some(cache) = get_cache() {
        cache.clear();
    }
}

pub fn cache_stats() -> Option<CacheStats> {
    get_cache()?.stats()
}

pub fn cached_inference<F>(
    model_name: &str,
    inputs: &[Vec<f32>],
    compute_fn: F,
) -> Result<Vec<f32>, String>
where
    F: FnOnce() -> Result<Vec<f32>, String>,
{
    let cache = get_or_init_cache();

    if !cache.is_enabled() {
        return compute_fn();
    }

    let key = CacheKey::new(model_name.to_string(), inputs);

    if let Some(entry) = cache.get(&key) {
        #[cfg(feature = "log")]
        debug!("Cache hit for model: {}", model_name);
        return Ok(entry.output);
    }

    #[cfg(feature = "log")]
    debug!("Cache miss for model: {}", model_name);

    let output = compute_fn()?;

    cache.put(key, output.clone());

    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cache_key_hash_consistency() {
        let inputs = vec![vec![1.0, 2.0, 3.0], vec![4.0, 5.0]];
        let key1 = CacheKey::new("model".to_string(), &inputs);
        let key2 = CacheKey::new("model".to_string(), &inputs);
        assert_eq!(key1, key2);
    }

    #[test]
    fn test_cache_key_different_models() {
        let inputs = vec![vec![1.0, 2.0, 3.0]];
        let key1 = CacheKey::new("model1".to_string(), &inputs);
        let key2 = CacheKey::new("model2".to_string(), &inputs);
        assert_ne!(key1, key2);
    }

    #[test]
    fn test_cache_key_different_inputs() {
        let key1 = CacheKey::new("model".to_string(), &[vec![1.0, 2.0]]);
        let key2 = CacheKey::new("model".to_string(), &[vec![1.0, 2.0, 3.0]]);
        assert_ne!(key1, key2);
    }

    #[test]
    fn test_cache_put_and_get() {
        let mut cache = InferenceCache::new(CacheConfig::default());
        let key = CacheKey::new("model".to_string(), &[vec![1.0, 2.0, 3.0]]);
        let output = vec![4.0, 5.0, 6.0];

        cache.put(key.clone(), output.clone());

        let entry = cache.get(&key);
        assert!(entry.is_some());
        assert_eq!(entry.unwrap().output, output);
    }

    #[test]
    fn test_cache_miss() {
        let mut cache = InferenceCache::new(CacheConfig::default());
        let key = CacheKey::new("model".to_string(), &[vec![1.0, 2.0, 3.0]]);

        let entry = cache.get(&key);
        assert!(entry.is_none());

        let stats = cache.stats();
        assert_eq!(stats.misses, 1);
    }

    #[test]
    fn test_cache_hit_stats() {
        let mut cache = InferenceCache::new(CacheConfig::default());
        let key = CacheKey::new("model".to_string(), &[vec![1.0, 2.0, 3.0]]);

        cache.put(key.clone(), vec![4.0, 5.0, 6.0]);
        cache.get(&key);
        cache.get(&key);

        let stats = cache.stats();
        assert_eq!(stats.hits, 2);
        assert_eq!(stats.misses, 0);
        assert_eq!(stats.current_size, 1);
    }

    #[test]
    fn test_cache_eviction() {
        let config = CacheConfig {
            max_entries: 2,
            ..Default::default()
        };
        let mut cache = InferenceCache::new(config);

        let key1 = CacheKey::new("model".to_string(), &[vec![1.0]]);
        let key2 = CacheKey::new("model".to_string(), &[vec![2.0]]);
        let key3 = CacheKey::new("model".to_string(), &[vec![3.0]]);

        cache.put(key1.clone(), vec![1.0]);
        cache.put(key2.clone(), vec![2.0]);
        cache.put(key3.clone(), vec![3.0]);

        assert!(cache.get(&key1).is_none());
        assert!(cache.get(&key2).is_some());
        assert!(cache.get(&key3).is_some());

        let stats = cache.stats();
        assert_eq!(stats.evictions, 1);
    }

    #[test]
    fn test_cache_clear() {
        let mut cache = InferenceCache::new(CacheConfig::default());

        cache.put(CacheKey::new("model".to_string(), &[vec![1.0]]), vec![1.0]);
        cache.put(CacheKey::new("model".to_string(), &[vec![2.0]]), vec![2.0]);

        assert_eq!(cache.len(), 2);

        cache.clear();

        assert!(cache.is_empty());
        assert_eq!(cache.memory_usage(), 0);
    }

    #[test]
    fn test_cache_disabled() {
        let config = CacheConfig {
            enabled: false,
            ..Default::default()
        };
        let mut cache = InferenceCache::new(config);

        let key = CacheKey::new("model".to_string(), &[vec![1.0]]);
        cache.put(key.clone(), vec![1.0]);

        assert!(cache.get(&key).is_none());
        assert!(cache.is_empty());
    }

    #[test]
    fn test_cache_entry_expiration() {
        let config = CacheConfig {
            ttl_seconds: Some(1),
            ..Default::default()
        };
        let mut cache = InferenceCache::new(config);

        let key = CacheKey::new("model".to_string(), &[vec![1.0]]);
        cache.put(key.clone(), vec![1.0]);

        assert!(
            cache.get(&key).is_some(),
            "Entry should exist immediately after put"
        );

        std::thread::sleep(std::time::Duration::from_secs(2));

        assert!(
            cache.get(&key).is_none(),
            "Entry should be expired after TTL"
        );
    }

    #[test]
    fn test_cache_hit_rate() {
        let stats = CacheStats {
            hits: 75,
            misses: 25,
            evictions: 0,
            current_size: 10,
            max_size: 100,
        };

        assert!((stats.hit_rate() - 0.75).abs() < 0.001);
    }

    #[test]
    fn test_thread_safe_cache() {
        let cache = ThreadSafeCache::new(CacheConfig::default());
        let key = CacheKey::new("model".to_string(), &[vec![1.0, 2.0]]);

        cache.put(key.clone(), vec![3.0, 4.0]);

        let entry = cache.get(&key);
        assert!(entry.is_some());
        assert_eq!(entry.unwrap().output, vec![3.0, 4.0]);
    }

    #[test]
    fn test_cached_inference_function() {
        let mut call_count = 0;

        let result1 = cached_inference("model", &[vec![1.0, 2.0]], || {
            call_count += 1;
            Ok(vec![3.0, 4.0])
        })
        .unwrap();

        let result2 = cached_inference("model", &[vec![1.0, 2.0]], || {
            call_count += 1;
            Ok(vec![3.0, 4.0])
        })
        .unwrap();

        assert_eq!(result1, vec![3.0, 4.0]);
        assert_eq!(result2, vec![3.0, 4.0]);
        assert_eq!(call_count, 1);
    }

    #[test]
    fn test_prune_expired() {
        let config = CacheConfig {
            ttl_seconds: Some(1),
            ..Default::default()
        };
        let mut cache = InferenceCache::new(config);

        cache.put(CacheKey::new("model".to_string(), &[vec![1.0]]), vec![1.0]);
        cache.put(CacheKey::new("model".to_string(), &[vec![2.0]]), vec![2.0]);

        assert_eq!(cache.len(), 2, "Should have 2 entries initially");

        std::thread::sleep(std::time::Duration::from_secs(2));

        let pruned = cache.prune_expired();
        assert_eq!(pruned, 2, "Should prune 2 expired entries");
        assert!(cache.is_empty(), "Cache should be empty after pruning");
    }
}
