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
}

impl<K, V> Default for SparseVec<K, V>
where
    K: Eq + Hash + Ord + Clone,
{
    fn default() -> Self {
        SparseVec {
            data: HashMap::new(),
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
        }
    }

    pub fn insert(&mut self, index: K, value: V) -> Option<V> {
        self.data.insert(index, value)

        // if self.data.insert(index.clone(), value).is_none() {
        //     self.max_index = Some(
        //         self.max_index
        //             .take()
        //             .map_or(index.clone(), |m| max(m, index.clone())),
        //     );
        //     self.min_index = Some(
        //         self.min_index
        //             .take()
        //             .map_or(index.clone(), |m| min(m, index)),
        //     );
        // }
    }

    pub fn remove(&mut self, index: &K) -> Option<V> {
        self.data.remove(index)

        // let result = self.data.remove(index);
        // if result.is_some() {
        //     if Some(index) == self.max_index.as_ref() {
        //         self.max_index = self.data.keys().max().cloned();
        //     }
        //     if Some(index) == self.min_index.as_ref() {
        //         self.min_index = self.data.keys().min().cloned();
        //     }
        // }
        // result
    }

    pub fn get(&self, index: &K) -> Option<&V> {
        self.data.get(index)
    }

    pub fn get_mut(&mut self, index: &K) -> Option<&mut V> {
        self.data.get_mut(index)
    }

    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    pub fn max_index(&self) -> Option<K> {
        self.data.keys().max().cloned()
    }

    pub fn min_index(&self) -> Option<K> {
        self.data.keys().min().cloned()
    }

    pub fn iter(&self) -> impl Iterator<Item = (&K, &V)> {
        self.data.iter()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sparse_vec_insert() {
        let mut sv = SparseVec::<u64, u64>::default();
        sv.insert(5, 50);
        sv.insert(10, 100);
        assert_eq!(sv.get(&5), Some(&50));
        assert_eq!(sv.get(&10), Some(&100));
    }

    #[test]
    fn test_sparse_vec_remove() {
        let mut sv = SparseVec::<u64, u64>::default();
        sv.insert(5, 50);
        sv.insert(10, 100);
        assert_eq!(sv.remove(&5), Some(50));
        assert_eq!(sv.get(&5), None);
    }

    #[test]
    fn test_sparse_vec_max_min_index() {
        let mut sv = SparseVec::<u64, u64>::default();
        sv.insert(5, 50);
        sv.insert(10, 100);
        sv.insert(3, 30);
        assert_eq!(sv.max_index(), Some(10));
        assert_eq!(sv.min_index(), Some(3));
    }
}
