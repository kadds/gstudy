use std::{borrow::Borrow, collections::{HashMap, HashSet}, hash::Hash};


#[derive(Default)]
pub struct FramedCache<K: Hash + Eq + PartialEq + Clone, V> {
    map: HashMap<K, V>,
    used: HashSet<K>,
    frame: u64,
}

impl<K: Hash + Eq + PartialEq + Clone, V> FramedCache<K, V> {
    pub fn new() -> Self {
        Self {
            map: HashMap::new(),
            used: HashSet::new(),
            frame: 0,
        }
    }

    pub fn recall(&mut self) {
        self.frame += 1;
        if self.frame % 16 != 0 {
            return;
        }

        let mut removal = vec![];

        for key in self.map.keys() {
            if !self.used.contains(key) {
                removal.push(key.clone());
            }
        }
        for key in removal {
            self.map.remove(&key);
        }

        self.used.clear();
    }

    pub fn get_or<S: Into<K>, F: FnOnce(&K) -> V>(&mut self, key: S, f: F) -> &V {
        let key = key.into();
        self.used.insert(key.clone());
        self.map.entry(key.clone()).or_insert_with(|| f(&key))
    }

    pub fn get_mut_or<S: Into<K>, F: FnOnce(&K) -> V>(&mut self, key: S, f: F) -> &mut V {
        let key = key.into();
        self.used.insert(key.clone());
        self.map.entry(key.clone()).or_insert_with(|| f(&key))
    }

    pub fn get<Q: ?Sized>(&self, key: &Q) -> Option<&V>
    where
        K: Borrow<Q>,
        Q: Hash + Eq,
    {
        self.map.get(key)
    }

    pub fn get_mut<Q: ?Sized>(&mut self, key: &Q) -> Option<&mut V>
    where
        K: Borrow<Q>,
        Q: Hash + Eq,
    {
        self.map.get_mut(key)
    }

    pub fn iter(&self) -> impl Iterator<Item = (&K, &V)> {
        self.map.iter()
    }

    pub fn iter_mut(&mut self) -> impl Iterator<Item = (&K, &mut V)> {
        self.map.iter_mut()
    }
}
