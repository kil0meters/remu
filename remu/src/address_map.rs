// maps 4096k page files

// memory layout:
// |9|9|9|9|12|

// https://www.bazhenov.me/posts/faster-binary-search-in-rust/
fn binary_search_branchless(data: &[u32], target: u32) -> usize {
    let mut idx = 1;
    while idx < data.len() {
        let el = data[idx];
        idx = 2 * idx + usize::from(el < target);
    }
    idx >>= idx.trailing_ones() + 1;
    usize::from(data[idx] == target) * idx
}

pub type Page = [u8; PAGE_SIZE as usize];
const EMPTY_PAGE: Page = [0; PAGE_SIZE as usize];

#[derive(Clone)]
struct TreeNode {
    index: u16,
    data: AddressEntry,
}

#[derive(Clone)]
enum AddressEntry {
    Leaf { page: Box<Page> },
    Branch { map: Box<AddressMap> },
}

#[derive(Clone)]
pub struct AddressMap {
    data: Vec<TreeNode>,
}

const MAXDEPTH: u8 = 3;

impl AddressMap {
    pub fn new() -> Self {
        Self { data: vec![] }
    }

    fn get_idx(addr: u64, depth: u8) -> u16 {
        debug_assert!(addr < (1 << 48));
        return ((addr >> (12 + 9 * (MAXDEPTH - depth))) & 0b111111111) as u16;
    }

    fn get_mut_internal(&mut self, addr: u64, depth: u8) -> Option<&mut Page> {
        debug_assert!(depth <= MAXDEPTH);
        let idx = Self::get_idx(addr, depth);

        for node in &mut self.data {
            if node.index == idx {
                return match node.data {
                    AddressEntry::Leaf { ref mut page } => Some(page.as_mut()),
                    AddressEntry::Branch { ref mut map } => map.get_mut_internal(addr, depth + 1),
                };
            } else if node.index > idx {
                return None;
            }
        }

        None

        // let res = match self.data.binary_search_by_key(&idx, |x| x.index) {
        //     Ok(res) => res,
        //     Err(_) => {
        //         return None;
        //     }
        // };
        //
        // match unsafe { self.data.get_unchecked_mut(res) }.data {
        //     AddressEntry::Leaf { ref mut page } => Some(page.as_mut()),
        //     AddressEntry::Branch { ref mut map } => map.get_mut_internal(addr, depth + 1),
        // }
    }

    pub fn get_mut(&mut self, addr: u64) -> Option<&mut Page> {
        self.get_mut_internal(addr, 0)
    }

    fn get_internal(&self, addr: u64, depth: u8) -> Option<&Page> {
        debug_assert!(depth <= MAXDEPTH);
        let idx = Self::get_idx(addr, depth);

        let res = match self.data.binary_search_by_key(&idx, |x| x.index) {
            Ok(res) => res,
            Err(_) => return None,
        };

        match self.data.get(res).expect("unreachable").data {
            AddressEntry::Leaf { ref page } => Some(page.as_ref()),
            AddressEntry::Branch { ref map } => map.get_internal(addr, depth + 1),
        }
    }

    pub fn get(&self, addr: u64) -> Option<&Page> {
        return self.get_internal(addr, 0);
    }

    fn insert_internal(&mut self, addr: u64, depth: u8) {
        debug_assert!(depth <= MAXDEPTH);
        let idx = Self::get_idx(addr, depth);

        match self.data.binary_search_by_key(&idx, |x| x.index) {
            Ok(res) => match self.data[res].data {
                AddressEntry::Leaf { ref mut page } => *page = Box::new(EMPTY_PAGE),
                AddressEntry::Branch { ref mut map } => map.insert_internal(addr, depth + 1),
            },
            Err(new_idx) => {
                let data = if depth == MAXDEPTH {
                    AddressEntry::Leaf {
                        page: Box::new(EMPTY_PAGE),
                    }
                } else {
                    let mut map = Box::new(AddressMap::new());
                    map.insert_internal(addr, depth + 1);

                    AddressEntry::Branch { map }
                };

                self.data.insert(
                    new_idx,
                    TreeNode {
                        index: idx as u16,
                        data,
                    },
                );
            }
        };
    }

    pub fn insert(&mut self, addr: u64) {
        self.insert_internal(addr, 0)
    }
}
