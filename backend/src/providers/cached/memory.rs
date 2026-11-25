//! Answers kept in memory for a while.

use std::collections::HashMap;
use std::hash::Hash;
use std::sync::Mutex;

/// Answers kept in memory, at most `max` of them, each until its own time.
pub(super) struct Memory<K, V> {
    max: usize,
    /// Answers with the Unix time each expires at.
    pub(super) kept: Mutex<HashMap<K, (i64, V)>>,
}

impl<K: Hash + Eq + Clone, V: Clone> Memory<K, V> {
    pub(super) fn new(max: usize) -> Self {
        Self {
            max,
            kept: Mutex::default(),
        }
    }

    pub(super) fn get(&self, key: &K, now: i64) -> Option<V> {
        let kept = self.kept.lock().unwrap();
        let (until, answer) = kept.get(key)?;
        (now < *until).then(|| answer.clone())
    }

    pub(super) fn put(&self, key: K, answer: V, now: i64, ttl: i64) {
        let mut kept = self.kept.lock().unwrap();
        if kept.len() >= self.max && !kept.contains_key(&key) {
            kept.retain(|_, (until, _)| now < *until);
        }
        if kept.len() >= self.max && !kept.contains_key(&key) {
            // Still full of fresh answers: the one that expires first makes room.
            let first = kept.iter().min_by_key(|(_, (until, _))| *until);
            if let Some(first) = first.map(|(key, _)| key.clone()) {
                kept.remove(&first);
            }
        }
        kept.insert(key, (now + ttl, answer));
    }
}
