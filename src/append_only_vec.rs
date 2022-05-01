use ::alloc::{alloc, vec};
use core::{cell, cmp, ops, ptr};

/// A collection onto which new values can be appended, while still keeping references to previous
/// values valid.
///
/// # Example
///
/// This is useful as a buffer on the side of another data structure that is built incrementally.
/// For example, let's imagine we want to parse a JSON-like data format that contains only arrays
/// and strings.
///
/// The advantage of having slices and `str`s instead of `Vec`s and `String`s is that you'd then be
/// to directly pattern match against values of this type.
///
/// ```
/// # use cursed_collections::AppendOnlyVec;
/// enum MyData<'buffers> {
///     Array(&'buffers [MyData<'buffers>]),
///     String(&'buffers str),
/// }
///
/// let string_buf = AppendOnlyVec::<String>::new();
/// let array_buf = AppendOnlyVec::<Vec<MyData>>::new();
///
/// let my_key = MyData::String(string_buf.push("name".into()));
/// let my_name = MyData::String(string_buf.push("Simon".into()));
/// let my_array = MyData::Array(array_buf.push(vec![my_key, my_name]));
///
/// match my_array {
///     MyData::Array(&[MyData::String("name"), MyData::String(name)]) => {
///         println!("Hello, {}", name)
///     }
///     _ => println!("Hello!"),
/// }
/// ```
pub struct AppendOnlyVec<T> {
    segments: cell::UnsafeCell<vec::Vec<*mut T>>,
    tail: cell::UnsafeCell<*mut T>,
    tail_size: cell::Cell<usize>,
    layout: alloc::Layout,
}

const SEGMENT_CAPACITY_LOG_2: usize = 5;
const SEGMENT_CAPACITY: usize = 1 << SEGMENT_CAPACITY_LOG_2;
const SEGMENT_CAPACITY_MASK: usize = SEGMENT_CAPACITY - 1;

impl<T> AppendOnlyVec<T> {
    /// Creates an empty `AppendOnlyVec`.
    pub fn new() -> AppendOnlyVec<T> {
        AppendOnlyVec {
            segments: cell::UnsafeCell::new(vec![]),
            tail: cell::UnsafeCell::new(ptr::null_mut()),
            tail_size: cell::Cell::new(0),
            layout: alloc::Layout::new::<[T; SEGMENT_CAPACITY]>(),
        }
    }

    /// Consumes a `T`, appends it to the end of the vector, and returns a reference to the newly
    /// appended element.
    pub fn push(&self, value: T) -> &T {
        unsafe {
            let tail = self.tail.get();
            if (*tail).is_null() {
                ptr::write(tail, alloc::alloc(self.layout) as *mut T)
            }

            let tail_size = self.tail_size.get();
            let dst = (*tail).add(tail_size);
            ptr::write(dst, value);

            let next_tail_size = tail_size + 1;
            self.tail_size.set(if next_tail_size == SEGMENT_CAPACITY {
                let tail = ptr::replace(tail, ptr::null_mut());
                (*self.segments.get()).push(tail);
                0
            } else {
                next_tail_size
            });

            &*dst
        }
    }

    /// Returns the number of elements in the vector.
    pub fn len(&self) -> usize {
        unsafe { (*self.segments.get()).len() * SEGMENT_CAPACITY + self.tail_size.get() }
    }

    pub fn is_empty(&self) -> bool {
        unsafe { self.tail_size.get() == 0 && (*self.segments.get()).is_empty() }
    }
}

impl<T> Drop for AppendOnlyVec<T> {
    fn drop(&mut self) {
        unsafe {
            let tail = *self.tail.get();
            if !tail.is_null() {
                for i in 0..self.tail_size.get() {
                    ptr::drop_in_place(tail.add(i))
                }
                alloc::dealloc(tail as _, self.layout);
            }
            for segment in (*self.segments.get()).drain(..) {
                for i in 0..SEGMENT_CAPACITY {
                    ptr::drop_in_place(segment.add(i));
                }
                alloc::dealloc(segment as _, self.layout);
            }
        }
    }
}

impl<T> Default for AppendOnlyVec<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> ops::Index<usize> for AppendOnlyVec<T> {
    type Output = T;

    fn index(&self, index: usize) -> &Self::Output {
        unsafe {
            let segment_offset = index & SEGMENT_CAPACITY_MASK;
            let segment_index = (index & !SEGMENT_CAPACITY_MASK) >> SEGMENT_CAPACITY_LOG_2;

            match segment_index.cmp(&(*self.segments.get()).len()) {
                cmp::Ordering::Less => {
                    let segment = *(*self.segments.get()).get_unchecked(segment_index);
                    &*segment.add(segment_offset)
                }
                cmp::Ordering::Equal if segment_offset < self.tail_size.get() => {
                    &*(*self.tail.get()).add(segment_offset)
                }
                _ => panic!("out of bounds, buddy"),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{AppendOnlyVec, SEGMENT_CAPACITY};
    use quickcheck_macros::quickcheck;
    use std::ptr;

    #[test]
    fn it_works() {
        let vec = AppendOnlyVec::<String>::new();
        let s1 = vec.push("hello".into());
        let s2 = vec.push("bye".into());
        assert_eq!(&String::from("hello"), s1);
        assert_eq!(&String::from("bye"), s2);
    }

    #[test]
    fn references_still_valid_after_another_segment_is_created() {
        let vec = AppendOnlyVec::<String>::new();
        let mut references = Vec::<&String>::new();
        for i in 0..(SEGMENT_CAPACITY + 1) {
            references.push(vec.push(format!("{}", i)));
        }

        assert_eq!(&"0", &references[0]);
        assert!(ptr::eq(&vec[0], references[0]));
        assert_eq!(
            format!("{}", SEGMENT_CAPACITY).as_str(),
            references[SEGMENT_CAPACITY].as_str()
        );
    }

    #[test]
    fn index() {
        let vec = AppendOnlyVec::<String>::new();
        vec.push("hello".into());
        vec.push("bye".into());

        assert_eq!(vec[0], "hello");
        assert_eq!(vec[1], "bye");
    }

    #[test]
    #[should_panic]
    fn index_out_of_bounds() {
        let vec = AppendOnlyVec::<String>::new();
        vec.push("hello".into());
        vec.push("bye".into());
        let _ = &vec[2];
    }

    #[test]
    fn len_empty() {
        let vec = AppendOnlyVec::<String>::new();
        assert_eq!(0, vec.len())
    }

    #[test]
    fn len_1() {
        let vec = AppendOnlyVec::<String>::new();
        vec.push("hello".into());
        assert_eq!(1, vec.len())
    }

    #[test]
    fn len_multiple_segments() {
        let vec = AppendOnlyVec::<String>::new();
        for i in 0..(SEGMENT_CAPACITY + 1) {
            vec.push(format!("{}", i));
        }
        assert_eq!(33, vec.len())
    }

    #[quickcheck]
    #[cfg_attr(miri, ignore)]
    fn is_same_as_vector_once_fully_initialized(expected: Vec<String>) -> bool {
        let actual = AppendOnlyVec::new();
        for value in &expected {
            actual.push(value.clone());
        }
        (0..expected.len()).all(|i| actual[i] == expected[i])
    }
}
