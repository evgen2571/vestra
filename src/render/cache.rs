//! Byte-budgeted least-recently-used resource cache.

use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, Default, serde::Serialize)]
pub struct CacheStats {
    pub requests: u64,
    pub budget_bytes: u64,
    pub current_bytes: u64,
    pub peak_bytes: u64,
    pub hits: u64,
    pub misses: u64,
    pub insertions: u64,
    pub evictions: u64,
    pub oversized_entries_skipped: u64,
}

struct Entry<V> {
    value: V,
    bytes: u64,
    last_used: u64,
}

/// A deterministic LRU cache. Ties cannot occur because every access advances
/// the monotonic clock, so eviction order is stable across runs.
pub struct ByteLruCache<K: Ord + Clone, V> {
    entries: BTreeMap<K, Entry<V>>,
    clock: u64,
    stats: CacheStats,
}

impl<K: Ord + Clone, V> ByteLruCache<K, V> {
    #[must_use]
    pub fn new(budget_bytes: u64) -> Self {
        Self {
            entries: BTreeMap::new(),
            clock: 0,
            stats: CacheStats {
                budget_bytes,
                ..CacheStats::default()
            },
        }
    }

    pub fn get(&mut self, key: &K) -> Option<&V> {
        self.clock = self.clock.wrapping_add(1);
        self.stats.requests += 1;
        match self.entries.get_mut(key) {
            Some(entry) => {
                entry.last_used = self.clock;
                self.stats.hits += 1;
                Some(&entry.value)
            }
            None => {
                self.stats.misses += 1;
                None
            }
        }
    }

    /// Resolves one logical request. A request records exactly one hit or one
    /// miss, including a miss that cannot be cached because it exceeds budget.
    pub fn get_or_insert_with(
        &mut self,
        key: K,
        bytes: u64,
        create: impl FnOnce() -> V,
    ) -> Option<&V> {
        if self.entries.contains_key(&key) {
            return self.get(&key);
        }
        self.clock = self.clock.wrapping_add(1);
        self.stats.requests += 1;
        self.stats.misses += 1;
        if bytes > self.stats.budget_bytes {
            self.stats.oversized_entries_skipped += 1;
            return None;
        }
        while self.stats.current_bytes.saturating_add(bytes) > self.stats.budget_bytes {
            let Some(lru_key) = self
                .entries
                .iter()
                .min_by_key(|(_, entry)| entry.last_used)
                .map(|(key, _)| key.clone())
            else {
                break;
            };
            let removed = self
                .entries
                .remove(&lru_key)
                .expect("LRU key was selected from cache");
            self.stats.current_bytes -= removed.bytes;
            self.stats.evictions += 1;
        }
        self.stats.current_bytes += bytes;
        self.stats.peak_bytes = self.stats.peak_bytes.max(self.stats.current_bytes);
        self.stats.insertions += 1;
        self.entries.insert(
            key.clone(),
            Entry {
                value: create(),
                bytes,
                last_used: self.clock,
            },
        );
        Some(
            &self
                .entries
                .get(&key)
                .expect("inserted cache entry is present")
                .value,
        )
    }

    pub fn insert(&mut self, key: K, value: V, bytes: u64) {
        if bytes > self.stats.budget_bytes {
            self.stats.oversized_entries_skipped += 1;
            return;
        }
        if let Some(previous) = self.entries.remove(&key) {
            self.stats.current_bytes -= previous.bytes;
        }
        while self.stats.current_bytes.saturating_add(bytes) > self.stats.budget_bytes {
            let Some(lru_key) = self
                .entries
                .iter()
                .min_by_key(|(_, entry)| entry.last_used)
                .map(|(key, _)| key.clone())
            else {
                break;
            };
            let removed = self
                .entries
                .remove(&lru_key)
                .expect("LRU key was selected from cache");
            self.stats.current_bytes -= removed.bytes;
            self.stats.evictions += 1;
        }
        self.clock = self.clock.wrapping_add(1);
        self.stats.current_bytes += bytes;
        self.stats.peak_bytes = self.stats.peak_bytes.max(self.stats.current_bytes);
        self.entries.insert(
            key,
            Entry {
                value,
                bytes,
                last_used: self.clock,
            },
        );
    }

    #[must_use]
    pub fn stats(&self) -> CacheStats {
        self.stats
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn evicts_the_least_recently_used_entry_by_bytes() {
        let mut cache = ByteLruCache::new(10);
        cache.insert("a", 1, 4);
        cache.insert("b", 2, 4);
        assert_eq!(cache.get(&"a"), Some(&1));
        cache.insert("c", 3, 4);
        assert_eq!(cache.get(&"a"), Some(&1));
        assert_eq!(cache.get(&"b"), None);
        assert_eq!(cache.get(&"c"), Some(&3));
        assert_eq!(cache.stats().evictions, 1);
        assert_eq!(cache.stats().current_bytes, 8);
    }

    #[test]
    fn skips_entries_larger_than_the_budget() {
        let mut cache = ByteLruCache::new(4);
        cache.insert("large", 1, 5);
        assert_eq!(cache.len(), 0);
        assert_eq!(cache.stats().oversized_entries_skipped, 1);
    }

    #[test]
    fn each_get_or_insert_request_is_counted_once() {
        let mut cache = ByteLruCache::new(8);
        assert_eq!(cache.get_or_insert_with("a", 4, || 1), Some(&1));
        assert_eq!(cache.get_or_insert_with("a", 4, || 2), Some(&1));
        let stats = cache.stats();
        assert_eq!(
            (stats.requests, stats.hits, stats.misses, stats.insertions),
            (2, 1, 1, 1)
        );
    }
}
