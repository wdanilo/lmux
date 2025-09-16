use std::fmt::Debug;
use std::hash::Hash;
use indexmap::IndexMap;

// ================
// === HashTree ===
// ================

#[derive(Clone, Debug)]
pub struct HashTree<K, V> {
    pub value: Option<V>,
    pub children: IndexMap<K, HashTree<K, V>>,
}

impl<K, V> Default for HashTree<K, V> {
    fn default() -> Self {
        Self { value: Default::default(), children: Default::default() }
    }
}

impl<K, V> HashTree<K, V> {
    pub fn new() -> Self {
        Default::default()
    }

    pub fn get(&self, path: &[K]) -> Option<&V>
    where K: Eq + Hash {
        if path.is_empty() {
            self.value.as_ref()
        } else {
            let child_key = &path[0];
            self.children.get(child_key).and_then(|child| child.get(&path[1..]))
        }
    }

    pub fn get_mut(&mut self, path: &[K]) -> Option<&mut V>
    where K: Eq + Hash {
        if path.is_empty() {
            self.value.as_mut()
        } else {
            let child_key = &path[0];
            self.children.get_mut(child_key).and_then(|child| child.get_mut(&path[1..]))
        }
    }

    pub fn get_or_insert_with
    (&mut self, path: &[K], f: impl FnOnce() -> V) -> &mut V
    where K: Clone + Eq + Hash {
        if path.is_empty() {
            self.value.get_or_insert_with(f)
        } else {
            let child_key = &path[0];
            let child = self.children.entry(child_key.clone()).or_default();
            child.get_or_insert_with(&path[1..], f)
        }
    }
}

// === Iterator for &HashTree ===

pub struct Iter<'a, K, V> {
    stack: Vec<(&'a HashTree<K, V>, Vec<&'a K>)>,
}

impl<'a, K, V> Iterator for Iter<'a, K, V>
where K: Eq + Hash {
    type Item = (Vec<&'a K>, &'a V);
    fn next(&mut self) -> Option<Self::Item> {
        while let Some((node, path)) = self.stack.pop() {
            for (key, child) in node.children.iter().rev() {
                let mut child_path = path.clone();
                child_path.push(key);
                self.stack.push((child, child_path));
            }

            if let Some(value) = &node.value {
                return Some((path, value));
            }
        }
        None
    }
}

impl<'a, K, V> IntoIterator for &'a HashTree<K, V>
where K: Eq + Hash {
    type Item = (Vec<&'a K>, &'a V);
    type IntoIter = Iter<'a, K, V>;
    fn into_iter(self) -> Self::IntoIter {
        Iter {
            stack: vec![(self, Vec::new())],
        }
    }
}

impl<'a, K, V> HashTree<K, V>
where K: Eq + Hash {
    pub fn iter(&'a self) -> Iter<'a, K, V> {
        self.into_iter()
    }
}

// === Iterator for &mut HashTree ===

pub struct IterMut<'a, K, V> {
    stack: Vec<(&'a mut HashTree<K, V>, Vec<&'a K>)>,
}

impl<'a, K, V> Iterator for IterMut<'a, K, V>
where K: Eq + Hash + Clone {
    type Item = (Vec<&'a K>, &'a mut V);
    fn next(&mut self) -> Option<Self::Item> {
        while let Some((node, path)) = self.stack.pop() {
            for (key, child) in node.children.iter_mut().rev() {
                let mut child_path = path.clone();
                child_path.push(key);
                self.stack.push((child, child_path));
            }

            if let Some(value) = &mut node.value {
                return Some((path, value));
            }
        }
        None
    }
}


impl<'a, K, V> IntoIterator for &'a mut HashTree<K, V>
where K: Clone + Eq + Hash {
    type Item = (Vec<&'a K>, &'a mut V);
    type IntoIter = IterMut<'a, K, V>;
    fn into_iter(self) -> Self::IntoIter {
        IterMut {
            stack: vec![(self, Vec::new())],
        }
    }
}

impl<'a, K, V> HashTree<K, V>
where K: Clone + Eq + Hash {
    pub fn iter_mut(&'a mut self) -> IterMut<'a, K, V> {
        self.into_iter()
    }
}


// === Iterator for HashTree ===

pub struct IntoIter<K, V> {
    stack: Vec<(Vec<K>, HashTree<K, V>)>,
}

impl<K, V> Iterator for IntoIter<K, V>
where K: Clone + Eq + Hash {
    type Item = (Vec<K>, V);

    fn next(&mut self) -> Option<Self::Item> {
        while let Some((path, node)) = self.stack.pop() {
            for (k, child) in node.children.into_iter().rev() {
                let mut child_path = path.clone();
                child_path.push(k);
                self.stack.push((child_path, child));
            }

            if let Some(value) = node.value {
                return Some((path, value));
            }
        }
        None
    }
}

impl<K, V> IntoIterator for HashTree<K, V>
where K: Clone + Eq + Hash {
    type Item = (Vec<K>, V);
    type IntoIter = IntoIter<K, V>;

    fn into_iter(self) -> Self::IntoIter {
        IntoIter {
            stack: vec![(Vec::new(), self)],
        }
    }
}
