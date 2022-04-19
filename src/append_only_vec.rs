use std::cell::UnsafeCell;
use std::ptr::NonNull;
use std::{cell, mem, ops, ptr};

const SEGMENT_CAPACITY_LOG_2: usize = 5;
const SEGMENT_CAPACITY: usize = 1 << SEGMENT_CAPACITY_LOG_2;
const SEGMENT_CAPACITY_MASK: usize = SEGMENT_CAPACITY - 1;

struct Segment<T> {
    len: usize,
    elements: Box<[mem::MaybeUninit<T>; SEGMENT_CAPACITY]>,
}

impl<T> Segment<T> {
    unsafe fn new() -> Segment<T> {
        Segment {
            len: 0,
            elements: Box::new(mem::MaybeUninit::uninit().assume_init()),
        }
    }
}

impl<T> Drop for Segment<T> {
    fn drop(&mut self) {
        unsafe {
            for element in &mut self.elements[0..self.len] {
                ptr::drop_in_place(element.as_mut_ptr());
            }
        }
    }
}

impl<T> ops::Index<usize> for Segment<T> {
    type Output = mem::MaybeUninit<T>;

    fn index(&self, index: usize) -> &<Self as ops::Index<usize>>::Output {
        if self.len <= index {
            panic!("index out of bounds")
        }

        &self.elements[index]
    }
}

impl<T> std::ops::IndexMut<usize> for Segment<T> {
    fn index_mut(&mut self, index: usize) -> &mut <Self as ops::Index<usize>>::Output {
        if self.len <= index {
            panic!("index out of bounds")
        }

        &mut self.elements[index]
    }
}

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
    segments: cell::UnsafeCell<Vec<cell::UnsafeCell<Segment<T>>>>,
}

impl<T> AppendOnlyVec<T> {
    /// Creates an empty `AppendOnlyVec`.
    pub fn new() -> AppendOnlyVec<T> {
        AppendOnlyVec {
            segments: cell::UnsafeCell::new(vec![]),
        }
    }

    /// Consumes a `T`, appends it to the end of the vector, and returns a reference to the newly
    /// appended element.
    pub fn push(&self, element: T) -> &T {
        unsafe {
            let segments = &mut *self.segments().as_ptr();
            let last_segment: &mut Segment<T> = match segments
                .last_mut()
                .map(|s| ptr::NonNull::new_unchecked(s.get()))
            {
                Some(segment) if segment.as_ref().len < SEGMENT_CAPACITY => &mut *segment.as_ptr(),
                _ => {
                    let index = segments.len();
                    segments.push(cell::UnsafeCell::new(Segment::new()));
                    &mut *ptr::NonNull::new_unchecked(segments[index].get()).as_ptr()
                }
            };

            let len = last_segment.len;
            last_segment.len += 1;

            let element_ref = &mut last_segment[len];
            *element_ref = mem::MaybeUninit::new(element);
            element_ref.as_ptr().as_ref().unwrap()
        }
    }

    /// Returns the number of elements in the vector.
    pub fn len(&self) -> usize {
        unsafe {
            let segments = &*self.segments().as_ptr();
            if let Some(last_segment) = segments.last() {
                let last_segment = &*ptr::NonNull::new_unchecked(last_segment.get()).as_ptr();
                ((segments.len() - 1) << SEGMENT_CAPACITY_LOG_2) + last_segment.len
            } else {
                0
            }
        }
    }

    pub fn is_empty(&self) -> bool {
        unsafe { self.segments().as_ref().is_empty() }
    }

    unsafe fn segments(&self) -> NonNull<Vec<UnsafeCell<Segment<T>>>> {
        ptr::NonNull::new_unchecked(self.segments.get())
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
            let segments = &*self.segments().as_ptr();
            let segment =
                &*ptr::NonNull::new_unchecked(segments[index >> SEGMENT_CAPACITY_LOG_2].get())
                    .as_ptr();
            segment[index & SEGMENT_CAPACITY_MASK]
                .as_ptr()
                .as_ref()
                .unwrap()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{AppendOnlyVec, SEGMENT_CAPACITY};
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
}
