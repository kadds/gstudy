use std::{
    collections::{vec_deque, HashMap, VecDeque},
    hash::Hash,
    marker::PhantomData,
    sync::{
        atomic::AtomicU32,
        mpsc::{channel, Receiver, Sender},
        Arc, Condvar, Mutex,
    },
    thread::ScopedJoinHandle,
    time::Instant,
    vec,
};

#[allow(unused)]
pub fn any_as_u8_slice<T: Sized>(p: &T) -> &[u8] {
    unsafe {
        ::std::slice::from_raw_parts((p as *const T) as *const u8, ::std::mem::size_of::<T>())
    }
}

#[allow(unused)]
pub fn any_as_u8_slice_array<T: Sized>(p: &[T]) -> &[u8] {
    unsafe {
        ::std::slice::from_raw_parts(
            (p.as_ptr() as *const T) as *const u8,
            ::std::mem::size_of::<T>() * p.len(),
        )
    }
}

#[allow(unused)]
pub fn any_as_x_slice_array<X: Sized, T: Sized>(p: &[T]) -> &[X] {
    unsafe {
        ::std::slice::from_raw_parts(
            (p.as_ptr() as *const T) as *const X,
            p.len() * ::std::mem::size_of::<T>() / ::std::mem::size_of::<X>(),
        )
    }
}

type SString = smartstring::alias::String;
pub struct StringIdMap<T> {
    str2id: HashMap<SString, T>,
    id2str: HashMap<T, SString>,
}

impl<T> StringIdMap<T>
where
    T: Hash + Eq + PartialEq + Copy,
{
    pub fn new() -> Self {
        Self {
            str2id: HashMap::new(),
            id2str: HashMap::new(),
        }
    }

    pub fn insert(&mut self, id: T, str: &str) {
        let s: SString = str.into();
        self.str2id.insert(s.clone(), id);
        self.id2str.insert(id, s);
    }

    pub fn value(&self, id: T) -> Option<&str> {
        self.id2str.get(&id).map(|v| v.as_str())
    }

    pub fn id_by_name(&self, name: &str) -> Option<T> {
        self.str2id.get(name).map(|v| *v)
    }

    pub fn remove(&mut self, id: T) {
        if let Some(v) = self.id2str.remove(&id) {
            self.str2id.remove(&v);
        }
    }
}

pub struct StringIdAllocMap<T>
where
    T: num_traits::PrimInt,
{
    m: StringIdMap<T>,
    last_id: T,
}

impl<T> StringIdAllocMap<T>
where
    T: num_traits::PrimInt + Hash,
{
    pub fn new() -> Self {
        Self {
            m: StringIdMap::new(),
            last_id: T::zero(),
        }
    }
    pub fn new_with_begin(beg: T) -> Self {
        Self {
            m: StringIdMap::new(),
            last_id: beg,
        }
    }

    pub fn alloc_or_get(&mut self, name: &str) -> T {
        if let Some(id) = self.m.id_by_name(name) {
            return id;
        }

        let id = self.last_id;
        self.last_id = self.last_id.add(T::from(1).unwrap());
        self.m.insert(id, name);
        id
    }

    pub fn get(&mut self, id: T) -> Option<&str> {
        self.m.value(id)
    }

    pub fn get_by_name(&mut self, name: &str) -> Option<T> {
        self.m.id_by_name(name)
    }

    pub fn dealloc(&mut self, id: T) {
        self.m.remove(id)
    }
}

pub struct OrderedMapIter<'a, K, V> {
    r: &'a HashMap<K, V>,
    i: std::slice::Iter<'a, K>,
}

impl<'a, K, V> Iterator for OrderedMapIter<'a, K, V>
where
    K: Hash + Eq + PartialEq,
{
    type Item = (&'a K, &'a V);

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(i) = self.i.next() {
            let v = self.r.get(i).unwrap();
            return Some((i, v));
        }
        None
    }
}
