use std::mem;

#[derive(Clone, Debug)]
pub struct Cache<K: Eq, V: Eq + Clone, const SIZE: usize> {
    data: [(K, V); SIZE],
    index: usize,
}

impl<K: Eq, V: Eq + Clone, const SIZE: usize> Cache<K, V, SIZE> {
    pub fn new() -> Cache<K, V, SIZE> {
        Cache {
            data: unsafe { mem::zeroed() },
            index: 0,
        }
    }

    // pub fn get(&self, key: K) -> Option<&V> {
    //     for item in &self.data {
    //         if item.0 == key {
    //             return Some(&item.1);
    //         }
    //     }
    //
    //     return None;
    // }

    fn put(&mut self, data: (K, V)) {
        self.data[self.index] = data;

        self.index += 1;
        self.index %= SIZE;
    }

    // update value, or add if it didn't exist
    pub fn update(&mut self, key: K, new_value: V) -> Option<V> {
        for item in &mut self.data {
            if item.0 == key {
                if item.1 != new_value {
                    item.1 = new_value;
                    return Some(item.1.clone());
                } else {
                    return None;
                }
            }
        }

        // wasn't found
        self.put((key, new_value));

        return None;
    }
}
