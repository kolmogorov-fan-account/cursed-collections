use std::cell;

#[derive(Debug)]
pub struct LazyArray<T>(cell::UnsafeCell<Vec<Option<T>>>);

impl<T> LazyArray<T> {
  pub fn new(size: usize) -> LazyArray<T> {
    let mut inner = Vec::new();
    inner.reserve(size);
    for _ in 0..size {
      inner.push(None);
    }
    LazyArray(cell::UnsafeCell::new(inner))
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
