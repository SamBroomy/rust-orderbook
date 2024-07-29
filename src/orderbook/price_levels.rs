use std::cmp::{max, min};
use std::collections::HashMap;
use std::hash::Hash;

#[derive(Debug)]
/// SparseVec is a data structure that is similar to a Vec, but it allows for "holes" in the data.
/// We could just use a Vec but the issue is we have pointers to the indexes of the Vec.
/// After the price levels change, this could mean that price levels are empty and we have levels containing no orders,
/// leaving redundant data being stored.
///
/// SparseVec allows us to store the data in a HashMap, and we can still iterate over the data in the order of the keys.
pub struct SparseVec<K, V>
where
    K: Eq + Hash + Ord + Clone,
{
    data: HashMap<K, V>,
    len: usize,
    max_index: Option<K>,
    min_index: Option<K>,
}

impl<K, V> Default for SparseVec<K, V>
where
    K: Eq + Hash + Ord + Clone,
{
    fn default() -> Self {
        SparseVec {
            data: HashMap::new(),
            len: 0,
            max_index: None,
            min_index: None,
        }
    }
}

impl<K, V> SparseVec<K, V>
where
    K: Eq + Hash + Ord + Clone,
{
    pub fn with_capacity(capacity: usize) -> Self {
        SparseVec {
            data: HashMap::with_capacity(capacity),
            ..Default::default()
        }
    }

    pub fn insert(&mut self, index: K, value: V) {
        if self.data.insert(index.clone(), value).is_none() {
            self.len = self.len.max(self.data.len());
            self.max_index = Some(
                self.max_index
                    .take()
                    .map_or(index.clone(), |m| max(m, index.clone())),
            );
            self.min_index = Some(
                self.min_index
                    .take()
                    .map_or(index.clone(), |m| min(m, index)),
            );
        }
    }

    pub fn remove(&mut self, index: &K) -> Option<V> {
        let result = self.data.remove(index);
        if result.is_some() {
            if Some(index) == self.max_index.as_ref() {
                self.max_index = self.data.keys().max().cloned();
            }
            if Some(index) == self.min_index.as_ref() {
                self.min_index = self.data.keys().min().cloned();
            }
            self.len = self.len.max(self.data.len());
            // if index == self.len - 1 {
            //     self.len = self.max_index.map_or(0, |max| max + 1);
            // }
        }
        result
    }

    pub fn get(&self, index: &K) -> Option<&V> {
        self.data.get(index)
    }

    pub fn get_mut(&mut self, index: &K) -> Option<&mut V> {
        self.data.get_mut(index)
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    pub fn max_index(&self) -> Option<K> {
        self.max_index.clone()
    }

    pub fn min_index(&self) -> Option<K> {
        self.min_index.clone()
    }

    pub fn iter(&self) -> impl Iterator<Item = (&K, &V)> {
        self.data.iter()
    }
}
