use ::alloc::alloc;
use core::{mem, ptr, slice};

/// A collection with a size defined at creation, but where entries are initialized later.
///
/// It is useful when you are reading nodes of an acyclic graph, where entries can be read as you
/// need them.
///
/// # Example
///
/// ```
/// # use cursed_collections::LazyArray;
/// #
/// #[derive(Debug, Eq, PartialEq)]
/// struct Entry<'heap> {
///     value: i32,
///     next: Option<&'heap Entry<'heap>>,
/// }
///
/// fn do_something<'heap>(heap: &'heap LazyArray<Entry<'heap>>) {
///     let entry_0 = heap.get_or_insert(3, Entry { value: 123, next: None });
///     let entry_1 = heap.get_or_insert(6, Entry { value: 456, next: Some(entry_0) });
///     assert_eq!(Some(123), entry_1.next.map(|inner| inner.value));
///     assert_eq!(None, heap.get(2));
/// }
/// ```
#[derive(Debug)]
pub struct LazyArray<T> {
    buffer: *mut Option<T>,
    capacity: usize,
    layout: alloc::Layout,
}

impl<T> LazyArray<T> {
    pub fn new(capacity: usize) -> Self {
        unsafe {
            let layout = alloc::Layout::array::<Option<T>>(capacity).expect("size overflow");
            let buffer = alloc::alloc(layout);
            {
                let slice =
                    slice::from_raw_parts_mut(buffer as *mut mem::MaybeUninit<Option<T>>, capacity);
                for i in slice {
                    *i = mem::MaybeUninit::new(None);
                }
            }
            Self {
                buffer: buffer as *mut Option<T>,
                capacity,
                layout,
            }
        }
    }

    fn entry(&self, index: usize) -> *mut Option<T> {
        unsafe {
            assert!(index < self.capacity);
            self.buffer.add(index)
        }
    }

    pub fn get_or_insert(&self, index: usize, t: T) -> &T {
        unsafe {
            // We cannot use Option::get_or_insert because we need to construct a &mut, which is
            // unsound if it is already initialized because there may be & existing!
            let entry = self.entry(index);
            match *entry {
                None => {
                    ptr::write(entry, Some(t));
                    (*entry).as_ref().unwrap_unchecked()
                }
                Some(ref v) => v,
            }
        }
    }

    pub fn get(&self, index: usize) -> Option<&T> {
        assert!(index < self.capacity);
        unsafe { (*self.buffer.add(index)).as_ref() }
    }
}

impl<T> Drop for LazyArray<T> {
    fn drop(&mut self) {
        unsafe {
            alloc::dealloc(self.buffer as *mut u8, self.layout);
        }
    }
}

impl<T> Default for LazyArray<T> {
    fn default() -> Self {
        Self::new(0)
    }
}

#[cfg(test)]
mod tests {
    use super::LazyArray;

    #[test]
    fn it_works() {
        let lazy_array = LazyArray::<i32>::new(10);
        for i in 0..10 {
            assert_eq!(lazy_array.get(i), None)
        }

        assert_eq!(lazy_array.get_or_insert(7, 112233), &112233);

        for i in 0..10 {
            assert_eq!(lazy_array.get(i), if i == 7 { Some(&112233) } else { None })
        }
    }

    #[test]
    fn cannot_insert_twice() {
        let lazy_array = LazyArray::<i32>::new(10);
        assert_eq!(lazy_array.get_or_insert(7, 112233), &112233);
        assert_eq!(lazy_array.get_or_insert(7, 445566), &112233);
    }

    #[test]
    #[should_panic]
    fn cannot_put_out_of_bounds() {
        let lazy_array = LazyArray::<i32>::new(10);
        lazy_array.get_or_insert(10, 112233);
    }
}
