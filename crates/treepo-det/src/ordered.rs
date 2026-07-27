//! Collections whose iteration order is a function of their contents.
//!
//! `AC-DET-3`: "No wall-clock, locale, filesystem-ordering, or hardware-dependent value
//! enters the generative pipeline."
//!
//! A `HashMap` violates that in a way nothing about the code makes visible. Its iteration
//! order depends on a per-process random seed, so a loop over one produces a different
//! order on every run of the same binary on the same machine. If any generated value
//! depends on that order — an accumulation, an index, an "which one won" tie-break — the
//! tree changes shape between runs, and it does so intermittently.
//!
//! `clippy.toml` bans `HashMap` and `HashSet` outright. These are what to reach for
//! instead: `BTreeMap` and `BTreeSet` under names that say why they are there.
//!
//! The cost is real and worth paying. B-tree lookup is `O(log n)` against a hash map's
//! `O(1)`, and treepo does not have a data structure hot enough to notice: the expensive
//! phase is rare by construction (`P10`), and the continuous phase reads baked weights
//! rather than maps (`F-THR-2`).

use alloc::collections::{BTreeMap, BTreeSet, btree_map, btree_set};
use core::fmt;
use core::ops::{Deref, DerefMut};

/// An ordered map. Iteration follows key order, on every run and every machine.
///
/// Derefs to [`BTreeMap`], so the whole standard API is available.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct OrderedMap<K, V>(BTreeMap<K, V>);

impl<K: Ord, V> OrderedMap<K, V> {
    /// An empty map.
    #[must_use]
    pub const fn new() -> Self {
        Self(BTreeMap::new())
    }

    /// Wraps an existing [`BTreeMap`].
    #[must_use]
    pub const fn from_btree(inner: BTreeMap<K, V>) -> Self {
        Self(inner)
    }

    /// Unwraps to the underlying [`BTreeMap`].
    #[must_use]
    pub fn into_btree(self) -> BTreeMap<K, V> {
        self.0
    }
}

impl<K, V> Deref for OrderedMap<K, V> {
    type Target = BTreeMap<K, V>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<K, V> DerefMut for OrderedMap<K, V> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl<K: Ord, V> Default for OrderedMap<K, V> {
    fn default() -> Self {
        Self::new()
    }
}

impl<K: fmt::Debug, V: fmt::Debug> fmt::Debug for OrderedMap<K, V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl<K: Ord, V> FromIterator<(K, V)> for OrderedMap<K, V> {
    fn from_iter<I: IntoIterator<Item = (K, V)>>(iter: I) -> Self {
        Self(BTreeMap::from_iter(iter))
    }
}

impl<K: Ord, V> Extend<(K, V)> for OrderedMap<K, V> {
    fn extend<I: IntoIterator<Item = (K, V)>>(&mut self, iter: I) {
        self.0.extend(iter);
    }
}

impl<K, V> IntoIterator for OrderedMap<K, V> {
    type Item = (K, V);
    type IntoIter = btree_map::IntoIter<K, V>;
    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

impl<'a, K, V> IntoIterator for &'a OrderedMap<K, V> {
    type Item = (&'a K, &'a V);
    type IntoIter = btree_map::Iter<'a, K, V>;
    fn into_iter(self) -> Self::IntoIter {
        self.0.iter()
    }
}

impl<'a, K, V> IntoIterator for &'a mut OrderedMap<K, V> {
    type Item = (&'a K, &'a mut V);
    type IntoIter = btree_map::IterMut<'a, K, V>;
    fn into_iter(self) -> Self::IntoIter {
        self.0.iter_mut()
    }
}

/// An ordered set. Iteration follows element order, on every run and every machine.
///
/// Derefs to [`BTreeSet`], so the whole standard API is available.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct OrderedSet<T>(BTreeSet<T>);

impl<T: Ord> OrderedSet<T> {
    /// An empty set.
    #[must_use]
    pub const fn new() -> Self {
        Self(BTreeSet::new())
    }

    /// Wraps an existing [`BTreeSet`].
    #[must_use]
    pub const fn from_btree(inner: BTreeSet<T>) -> Self {
        Self(inner)
    }

    /// Unwraps to the underlying [`BTreeSet`].
    #[must_use]
    pub fn into_btree(self) -> BTreeSet<T> {
        self.0
    }
}

impl<T> Deref for OrderedSet<T> {
    type Target = BTreeSet<T>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T> DerefMut for OrderedSet<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl<T: Ord> Default for OrderedSet<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: fmt::Debug> fmt::Debug for OrderedSet<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl<T: Ord> FromIterator<T> for OrderedSet<T> {
    fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Self {
        Self(BTreeSet::from_iter(iter))
    }
}

impl<T: Ord> Extend<T> for OrderedSet<T> {
    fn extend<I: IntoIterator<Item = T>>(&mut self, iter: I) {
        self.0.extend(iter);
    }
}

impl<T> IntoIterator for OrderedSet<T> {
    type Item = T;
    type IntoIter = btree_set::IntoIter<T>;
    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

impl<'a, T> IntoIterator for &'a OrderedSet<T> {
    type Item = &'a T;
    type IntoIter = btree_set::Iter<'a, T>;
    fn into_iter(self) -> Self::IntoIter {
        self.0.iter()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec::Vec;

    #[test]
    fn map_iterates_in_key_order_regardless_of_insertion_order() {
        let forwards: OrderedMap<u32, &str> = [(1, "a"), (2, "b"), (3, "c")].into_iter().collect();
        let backwards: OrderedMap<u32, &str> = [(3, "c"), (2, "b"), (1, "a")].into_iter().collect();

        let keys = |m: &OrderedMap<u32, &str>| m.keys().copied().collect::<Vec<_>>();
        assert_eq!(keys(&forwards), alloc::vec![1, 2, 3]);
        assert_eq!(keys(&forwards), keys(&backwards));
        assert_eq!(forwards, backwards);
    }

    #[test]
    fn map_derefs_to_the_full_btree_api() {
        let mut map = OrderedMap::new();
        map.insert("src", 3u32);
        map.insert("docs", 1);
        *map.get_mut("src").expect("present") += 1;

        assert_eq!(map.get("src"), Some(&4));
        assert_eq!(map.len(), 2);
        assert!(map.contains_key("docs"));
        assert_eq!(map.remove("docs"), Some(1));
        assert!(!map.is_empty());

        let borrowed: Vec<_> = (&map).into_iter().collect();
        assert_eq!(borrowed, alloc::vec![(&"src", &4)]);
    }

    #[test]
    fn set_iterates_in_element_order() {
        let set: OrderedSet<&str> = ["zebra", "apple", "mango"].into_iter().collect();
        assert_eq!(
            set.iter().copied().collect::<Vec<_>>(),
            alloc::vec!["apple", "mango", "zebra"]
        );
        assert!(set.contains("apple"));

        let mut mutable = OrderedSet::new();
        mutable.insert(2u8);
        mutable.insert(1);
        assert_eq!(mutable.into_iter().collect::<Vec<_>>(), alloc::vec![1, 2]);
    }
}
