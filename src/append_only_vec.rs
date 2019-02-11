use std::{cell, mem, ops, ptr};

const SEGMENT_CAPACITY_LOG_2: usize = 5;
const SEGMENT_CAPACITY: usize = 1 << SEGMENT_CAPACITY_LOG_2;
const SEGMENT_CAPACITY_MASK: usize = SEGMENT_CAPACITY - 1;

struct Segment<T> {
  len: usize,
  elements: Option<Box<[T; SEGMENT_CAPACITY]>>,
}

impl<T> Segment<T> {
  unsafe fn new() -> Segment<T> {
    Segment {
      len: 0,
      elements: Some(Box::new(mem::uninitialized())),
    }
  }

  fn is_full(&self) -> bool {
    self.len >= SEGMENT_CAPACITY
  }

  /// Retrieves a reference to a slot within the segment, that may be unintialized memory.
  ///
  /// # Safety
  ///
  /// The returned reference points to initialized memory if and only if the given `index` is
  /// smaller than `self.len`.
  unsafe fn get_slot(&self, index: usize) -> &T {
    if let Some(ref elements) = self.elements {
      &elements[index]
    } else {
      panic!()
    }
  }

  /// Retrieves a mutable reference to a slot within the segment, that may be unintialized memory.
  ///
  /// # Safety
  ///
  /// The returned reference points to initialized memory if and only if the given `index` is
  /// smaller than `self.len`.
  unsafe fn get_slot_mut(&mut self, index: usize) -> &mut T {
    if let Some(ref mut elements) = self.elements {
      &mut elements[index]
    } else {
      panic!()
    }
  }
}

impl<T> Drop for Segment<T> {
  fn drop(&mut self) {
    unsafe {
      if let Some(mut elements) = mem::replace(&mut self.elements, None) {
        for i in 0..self.len {
          ptr::drop_in_place(&mut elements[i]);
        }

        mem::forget(elements);
      }
    }
  }
}

impl<T> ops::Index<usize> for Segment<T> {
  type Output = T;

  fn index(&self, index: usize) -> &<Self as ops::Index<usize>>::Output {
    if self.len <= index {
      panic!("index out of bounds")
    }

    unsafe { self.get_slot(index) }
  }
}

impl<T> std::ops::IndexMut<usize> for Segment<T> {
  fn index_mut(&mut self, index: usize) -> &mut <Self as ops::Index<usize>>::Output {
    if self.len <= index {
      panic!("index out of bounds")
    }

    unsafe { self.get_slot_mut(index) }
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
/// use cursed_collections::AppendOnlyVec;
///
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

  /// Consumes a T, appends it to the end of the vector, and returns a reference to the newly
  /// appended element.
  pub fn push(&self, element: T) -> &T {
    unsafe {
      let last_segment = self.get_segment_with_spare_capacity();

      let len = last_segment.len;
      last_segment.len += 1;

      // A simple assignment is not enough because *element_ref = element would invoke drop on
      // the previous value of *element_ref, which is uninitialized memory.
      let element_ref = last_segment.get_slot_mut(len);
      mem::forget(mem::replace(element_ref, element));
      element_ref
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
}
