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

  pub fn put(&self, index: usize, t: T) {
    match &mut unsafe { &mut *self.0.get() }[index] {
      empty @ &mut None => *empty = Some(t),
      _ => (),
    }
  }

  pub fn get(&self, index: usize) -> Option<&T> {
    if let Some(ref element) = unsafe { &*self.0.get() }[index] {
      Some(element)
    } else {
      None
    }
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

    lazy_array.put(7, 112233);

    for i in 0..10 {
      assert_eq!(lazy_array.get(i), if i == 7 { Some(&112233) } else { None })
    }
  }

  #[test]
  #[should_panic]
  fn cannot_put_out_of_bounds() {
    let lazy_array = LazyArray::<i32>::new(10);
    lazy_array.put(10, 112233);
  }
}
