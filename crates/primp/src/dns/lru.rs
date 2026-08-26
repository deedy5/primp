//! Tiny inline LRU cache: bounded-capacity, O(1) `get`/`put`/`pop`.
//! `get_mut` promotes to MRU; `peek`/`peek_mut` do not. Implemented with
//! parallel `Vec`s plus a `HashMap`, no `unsafe`, no extra deps.

use std::borrow::Borrow;
use std::hash::Hash;
use std::num::NonZeroUsize;

use foldhash::{HashMap, HashMapExt};

pub(crate) struct LruCache<K, V> {
    keys: Vec<Option<K>>,
    values: Vec<Option<V>>,
    prev: Vec<Option<usize>>,
    next: Vec<Option<usize>>,
    free: Vec<usize>,
    map: HashMap<K, usize>,
    head: Option<usize>,
    tail: Option<usize>,
    cap: NonZeroUsize,
    len: usize,
}

impl<K, V> LruCache<K, V>
where
    K: Hash + Eq + Clone,
{
    pub(crate) fn new(cap: NonZeroUsize) -> Self {
        Self {
            keys: Vec::new(),
            values: Vec::new(),
            prev: Vec::new(),
            next: Vec::new(),
            free: Vec::new(),
            map: HashMap::new(),
            head: None,
            tail: None,
            cap,
            len: 0,
        }
    }

    #[cfg(test)]
    pub(crate) fn len(&self) -> usize {
        self.len
    }

    #[cfg(test)]
    pub(crate) fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Returns `Some(&mut V)` if `key` is present, promoting the entry
    /// to MRU. Returns `None` otherwise.
    pub(crate) fn get_mut<Q>(&mut self, key: &Q) -> Option<&mut V>
    where
        K: Borrow<Q>,
        Q: Hash + Eq + ?Sized,
    {
        let slot = *self.map.get(key)?;
        self.detach(slot);
        self.attach_mru(slot);
        self.values[slot].as_mut()
    }

    /// Returns `Some(&V)` if `key` is present. Does not promote.
    /// Accepts any `&Q` where `K: Borrow<Q>` so callers can pass
    /// `&str` for `K = String` without an extra allocation.
    pub(crate) fn peek<Q>(&self, key: &Q) -> Option<&V>
    where
        K: Borrow<Q>,
        Q: Hash + Eq + ?Sized,
    {
        self.values[*self.map.get(key)?].as_ref()
    }

    /// Returns `Some(&mut V)` if `key` is present. Does not promote.
    #[cfg(test)]
    pub(crate) fn peek_mut<Q>(&mut self, key: &Q) -> Option<&mut V>
    where
        K: Borrow<Q>,
        Q: Hash + Eq + ?Sized,
    {
        self.values[*self.map.get(key)?].as_mut()
    }

    /// Insert or update `key -> value`, promoting the entry to MRU.
    /// If the cache is at capacity, evicts the LRU entry.
    pub(crate) fn put(&mut self, key: K, value: V) {
        if let Some(&slot) = self.map.get(&key) {
            *self.values[slot]
                .as_mut()
                .expect("slot in map has no value") = value;
            self.detach(slot);
            self.attach_mru(slot);
            return;
        }
        let slot = self.alloc_slot();
        self.keys[slot] = Some(key.clone());
        self.values[slot] = Some(value);
        self.map.insert(key, slot);
        self.attach_mru(slot);
        self.len += 1;
        if self.len > self.cap.get() {
            self.evict_lru();
        }
    }

    /// Remove `key` from the cache, returning the value if present.
    pub(crate) fn pop<Q>(&mut self, key: &Q) -> Option<V>
    where
        K: Borrow<Q>,
        Q: Hash + Eq + ?Sized,
    {
        let slot = self.map.remove(key)?;
        self.detach(slot);
        let value = self.values[slot].take();
        self.keys[slot] = None;
        self.free.push(slot);
        self.len -= 1;
        value
    }

    fn alloc_slot(&mut self) -> usize {
        if let Some(slot) = self.free.pop() {
            slot
        } else {
            let slot = self.keys.len();
            self.keys.push(None);
            self.values.push(None);
            self.prev.push(None);
            self.next.push(None);
            slot
        }
    }

    fn evict_lru(&mut self) {
        let Some(slot) = self.head else { return };
        self.detach(slot);
        if let Some(k) = self.keys[slot].take() {
            self.map.remove(&k);
        }
        self.values[slot] = None;
        self.free.push(slot);
        self.len -= 1;
    }

    /// Detach `slot` from the doubly-linked list. The slot is left
    /// isolated; its `prev`/`next` are cleared.
    fn detach(&mut self, slot: usize) {
        let prev = self.prev[slot].take();
        let next = self.next[slot].take();
        match prev {
            Some(p) => self.next[p] = next,
            None => self.head = next,
        }
        match next {
            Some(n) => self.prev[n] = prev,
            None => self.tail = prev,
        }
    }

    /// Attach `slot` as the new MRU (tail).
    fn attach_mru(&mut self, slot: usize) {
        self.prev[slot] = self.tail;
        self.next[slot] = None;
        match self.tail {
            Some(old) => self.next[old] = Some(slot),
            None => self.head = Some(slot),
        }
        self.tail = Some(slot);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cap(n: usize) -> NonZeroUsize {
        NonZeroUsize::new(n).unwrap()
    }

    #[test]
    fn empty_cache_has_no_entries() {
        let mut c: LruCache<String, i32> = LruCache::new(cap(4));
        assert_eq!(c.len(), 0);
        assert!(c.is_empty());
        assert!(c.peek("a").is_none());
        assert!(c.peek_mut("a").is_none());
        assert!(c.get_mut("a").is_none());
        assert!(c.pop("a").is_none());
    }

    #[test]
    fn put_then_get_returns_value_and_promotes() {
        let mut c = LruCache::new(cap(3));
        c.put("a".to_string(), 1);
        c.put("b".to_string(), 2);
        // LRU order: a (LRU), b (MRU).
        assert_eq!(c.get_mut(&"a".to_string()), Some(&mut 1));
        // After get_mut: b (LRU), a (MRU).
        c.put("c".to_string(), 3);
        // len=3, at cap, no eviction yet (a promoted so still in).
        assert_eq!(c.len(), 3);
        c.put("d".to_string(), 4);
        // Evict LRU: b.
        assert_eq!(c.len(), 3);
        assert!(c.peek(&"b".to_string()).is_none());
        assert!(c.peek(&"a".to_string()).is_some());
        assert!(c.peek(&"c".to_string()).is_some());
        assert!(c.peek(&"d".to_string()).is_some());
    }

    #[test]
    fn peek_does_not_promote() {
        let mut c = LruCache::new(cap(2));
        c.put("a".to_string(), 1);
        c.put("b".to_string(), 2);
        // LRU order: a, b. peek should not change order.
        assert_eq!(c.peek(&"a".to_string()), Some(&1));
        assert_eq!(c.peek_mut(&"a".to_string()), Some(&mut 1));
        c.put("c".to_string(), 3);
        // a is still LRU (peek didn't promote), so a is evicted.
        assert!(c.peek(&"a".to_string()).is_none());
        assert!(c.peek(&"b".to_string()).is_some());
        assert!(c.peek(&"c".to_string()).is_some());
    }

    #[test]
    fn get_mut_promotes() {
        let mut c = LruCache::new(cap(2));
        c.put("a".to_string(), 1);
        c.put("b".to_string(), 2);
        assert_eq!(c.get_mut(&"a".to_string()), Some(&mut 1));
        c.put("c".to_string(), 3);
        // a was promoted, so b is LRU → b is evicted.
        assert!(c.peek(&"b".to_string()).is_none());
        assert!(c.peek(&"a".to_string()).is_some());
        assert!(c.peek(&"c".to_string()).is_some());
    }

    #[test]
    fn pop_removes_entry_and_returns_value() {
        let mut c = LruCache::new(cap(3));
        c.put("a".to_string(), 1);
        c.put("b".to_string(), 2);
        c.put("c".to_string(), 3);
        assert_eq!(c.pop(&"b".to_string()), Some(2));
        assert_eq!(c.len(), 2);
        assert!(c.peek(&"b".to_string()).is_none());
        assert!(c.peek(&"a".to_string()).is_some());
        assert!(c.peek(&"c".to_string()).is_some());
    }

    #[test]
    fn put_existing_key_updates_value_and_promotes() {
        let mut c = LruCache::new(cap(3));
        c.put("a".to_string(), 1);
        c.put("b".to_string(), 2);
        c.put("c".to_string(), 3);
        // Order: a (LRU), b, c (MRU). Re-insert a.
        c.put("a".to_string(), 99);
        assert_eq!(c.peek(&"a".to_string()), Some(&99));
        // Order: b (LRU), c, a (MRU). len still 3, no eviction.
        assert_eq!(c.len(), 3);
        c.put("d".to_string(), 4);
        // b evicted (LRU).
        assert!(c.peek(&"b".to_string()).is_none());
        assert_eq!(c.peek(&"a".to_string()), Some(&99));
    }

    #[test]
    fn len_stays_bounded_across_many_cycles() {
        let mut c = LruCache::new(cap(8));
        for i in 0..1000 {
            c.put(format!("k{i}"), i);
        }
        assert_eq!(c.len(), 8);
    }

    #[test]
    fn pop_then_reinsert_works() {
        let mut c = LruCache::new(cap(2));
        c.put("a".to_string(), 1);
        c.put("b".to_string(), 2);
        assert_eq!(c.pop(&"a".to_string()), Some(1));
        assert_eq!(c.len(), 1);
        c.put("c".to_string(), 3);
        assert_eq!(c.len(), 2);
        assert!(c.peek(&"a".to_string()).is_none());
        assert!(c.peek(&"b".to_string()).is_some());
        assert!(c.peek(&"c".to_string()).is_some());
    }
}
