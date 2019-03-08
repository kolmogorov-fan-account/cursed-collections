use std::{cell, mem, ops};

const SEGMENT_CAPACITY_LOG_2: usize = 5;
const SEGMENT_CAPACITY: usize = 1 << SEGMENT_CAPACITY_LOG_2;
const SEGMENT_CAPACITY_MASK: usize = SEGMENT_CAPACITY - 1;

struct Segment<T> {
  len: usize,
  elements: Box<[mem::ManuallyDrop<T>; SEGMENT_CAPACITY]>,
}

impl<T> Segment<T> {
  unsafe fn new() -> Segment<T> {
    Segment {
      len: 0,
      elements: Box::new(mem::uninitialized()),
    }
  }

  fn is_full(&self) -> bool {
    self.len >= SEGMENT_CAPACITY
  }
}

impl<T> Drop for Segment<T> {
  fn drop(&mut self) {
    unsafe {
      for element in &mut self.elements[0..self.len] {
        mem::ManuallyDrop::drop(element);
      }
    }
  }
}

impl<T> ops::Index<usize> for Segment<T> {
  type Output = mem::ManuallyDrop<T>;

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
///   Array(&'buffers [MyData<'buffers>]),
///   String(&'buffers str),
/// }
///
/// fn main() {
///   let string_buf = AppendOnlyVec::<String>::new();
///   let array_buf = AppendOnlyVec::<Vec<MyData>>::new();
///
///   let my_key = MyData::String(string_buf.push("name".into()));
///   let my_name = MyData::String(string_buf.push("Simon".into()));
///   let my_array = MyData::Array(array_buf.push(vec![my_key, my_name]));
///
///   match my_array {
///     MyData::Array(&[MyData::String("name"), MyData::String(name)]) => {
///       println!("Hello, {}", name)
///     }
///     _ => println!("Hello!"),
///   }
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
      let last_segment = self.get_segment_with_spare_capacity();

      let len = last_segment.len;
      last_segment.len += 1;

      let element_ref = &mut last_segment[len];
      mem::replace(element_ref, mem::ManuallyDrop::new(element));
      element_ref
    }
  }

  /// Returns the number of elements in the vector.
  pub fn len(&self) -> usize {
    unsafe {
      let segments = self.segments();
      if let Some(last_segment) = segments.last() {
        ((segments.len() - 1) << SEGMENT_CAPACITY_LOG_2) + (&*last_segment.get()).len
      } else {
        0
      }
    }
  }

  unsafe fn get_segment_at(&self, index: usize) -> &Segment<T> {
    let segments = self.segments();
    &*segments[index].get()
  }

  unsafe fn get_segment_with_spare_capacity(&self) -> &mut Segment<T> {
    let segments = self.segments();
    match segments.last_mut() {
      None => self.add_segment(),
      Some(segment) => {
        if (*segment.get()).is_full() {
          self.add_segment()
        } else {
          &mut *segment.get()
        }
      }
    }
  }

  unsafe fn add_segment(&self) -> &mut Segment<T> {
    let segments = self.segments();
    segments.push(cell::UnsafeCell::new(Segment::new()));
    &mut *segments.last_mut().unwrap().get()
  }

  unsafe fn segments(&self) -> &mut Vec<cell::UnsafeCell<Segment<T>>> {
    &mut *self.segments.get()
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
      let segment = self.get_segment_at(index >> SEGMENT_CAPACITY_LOG_2);
      &segment[index & SEGMENT_CAPACITY_MASK]
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
    &vec[2];
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
