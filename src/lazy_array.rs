use std::cell;

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
/// let heap = LazyArray::<Entry>::new(10);
/// let entry_0 = heap.get_or_insert(3, Entry { value: 123, next: None });
/// let entry_1 = heap.get_or_insert(6, Entry { value: 456, next: Some(entry_0) });
/// assert_eq!(Some(123), entry_1.next.map(|inner| inner.value));
/// assert_eq!(None, heap.get(2));
/// ```
#[derive(Debug)]
pub struct LazyArray<T>(cell::UnsafeCell<Box<[Option<T>]>>);

impl<T> LazyArray<T> {
    pub fn new(size: usize) -> LazyArray<T> {
        let mut init = Vec::<Option<T>>::with_capacity(size);
        for _ in 0..size {
            init.push(None)
        }
        LazyArray(cell::UnsafeCell::new(init.into_boxed_slice()))
    }

    pub fn get_or_insert(&self, index: usize, t: T) -> &T {
        (&mut unsafe { &mut *self.0.get() }[index]).get_or_insert(t)
    }

    pub fn get(&self, index: usize) -> Option<&T> {
        if let Some(ref element) = unsafe { &*self.0.get() }[index] {
            Some(element)
        } else {
            None
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
