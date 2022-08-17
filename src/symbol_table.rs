use ::alloc::{alloc, string::String, vec};
use core::borrow::Borrow;
use core::{cell, fmt, hash, marker, mem, ptr, slice, str};
use hashbrown::HashSet;

const LARGE_SYMBOL_THRESHOLD: usize = 1 << 9;
const SEGMENT_CAPACITY: usize = 1 << 12;

#[allow(clippy::assertions_on_constants)]
const _: () = assert!(
    LARGE_SYMBOL_THRESHOLD < SEGMENT_CAPACITY,
    "a small symbol must always fit in a fresh segment",
);

#[repr(transparent)]
struct SymbolKey(*const str);

impl PartialEq for SymbolKey {
    fn eq(&self, other: &Self) -> bool {
        unsafe { *self.0 == *other.0 }
    }
}

impl Eq for SymbolKey {}

impl hash::Hash for SymbolKey {
    fn hash<H: hash::Hasher>(&self, state: &mut H) {
        unsafe { (*self.0).hash(state) }
    }
}

/// Like a `&str`, but with constant time equality comparison.
///
/// It is a distinct type from `&str` to avoid confusion where an interned string could be compared
/// to an uninterned string and give a confusing false negative.
#[repr(transparent)]
#[derive(Copy, Clone)]
pub struct Symbol<'table> {
    ptr: *const str,
    _p: marker::PhantomData<&'table str>,
}

impl<'table> Symbol<'table> {
    fn new(ptr: *const str) -> Self {
        Self {
            ptr,
            _p: marker::PhantomData,
        }
    }
}

impl<'table> PartialEq for Symbol<'table> {
    fn eq(&self, other: &Self) -> bool {
        ptr::eq(self.ptr, other.ptr)
    }
}

impl<'table> Eq for Symbol<'table> {}

impl<'table> AsRef<str> for Symbol<'table> {
    fn as_ref(&self) -> &str {
        unsafe { &*self.ptr }
    }
}

impl<'table> fmt::Debug for Symbol<'table> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        unsafe { write!(f, "{:?}@{:p}", &*self.ptr, self.ptr) }
    }
}

impl<'table> fmt::Display for Symbol<'table> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        unsafe { f.write_str(&*self.ptr) }
    }
}

impl<'table> Borrow<str> for Symbol<'table> {
    fn borrow(&self) -> &str {
        self.as_ref()
    }
}

const BUFFER_LAYOUT: alloc::Layout = alloc::Layout::new::<[u8; SEGMENT_CAPACITY]>();

/// A set of strings. Unlike a regular set, strings are stored contiguously in pages to reduce
/// memory usage.
pub struct SymbolTable {
    lookup: cell::UnsafeCell<HashSet<SymbolKey>>,
    small_symbols: cell::UnsafeCell<vec::Vec<*const u8>>,
    large_symbols: cell::UnsafeCell<vec::Vec<(*const u8, usize, usize)>>,
    tail: cell::Cell<*mut u8>,
    tail_offset: cell::Cell<usize>,
}

impl SymbolTable {
    /// Create an empty table.
    ///
    /// Unlike many types in `alloc`, this allocates right away.
    pub fn new() -> Self {
        unsafe {
            Self {
                lookup: cell::UnsafeCell::new(HashSet::new()),
                small_symbols: cell::UnsafeCell::new(vec![]),
                large_symbols: cell::UnsafeCell::new(vec![]),
                tail: cell::Cell::new(alloc::alloc(BUFFER_LAYOUT)),
                tail_offset: cell::Cell::new(0),
            }
        }
    }

    /// Adds a symbol to the table if it does not exist.
    ///
    /// # Example
    ///
    /// ```
    /// # use cursed_collections::SymbolTable;
    /// let table = SymbolTable::new();
    /// assert_eq!(table.intern("my symbol"), table.intern("my symbol"));
    /// ```
    pub fn intern(&self, text: impl Into<String> + AsRef<str>) -> Symbol {
        unsafe {
            let lookup = &mut *self.lookup.get();
            if let Some(&SymbolKey(ptr)) = lookup.get(&SymbolKey(text.as_ref())) {
                return Symbol::new(ptr);
            }

            let symbol @ Symbol { ptr, .. } = self.gensym(text);
            lookup.insert(SymbolKey(ptr));
            symbol
        }
    }

    /// Adds a symbol to the table. This symbol is always considered distinct from all other symbols
    /// even if they are textually identical.
    ///
    /// # Example
    ///
    /// ```
    /// # use cursed_collections::SymbolTable;
    /// let table = SymbolTable::new();
    /// assert_ne!(table.intern("my symbol"), table.gensym("my symbol"));
    /// ```
    ///
    /// # Name
    ///
    /// The name "`gensym`" is common within the Lisp family of languages where symbols are built in
    /// the language itself.
    pub fn gensym(&self, text: impl Into<String> + AsRef<str>) -> Symbol {
        unsafe {
            let text_len = text.as_ref().len();
            if text_len >= LARGE_SYMBOL_THRESHOLD {
                let large_symbol = mem::ManuallyDrop::new(text.into());
                let ptr = large_symbol.as_ptr();
                let size = large_symbol.len();
                (*self.large_symbols.get()).push((ptr, size, large_symbol.capacity()));
                return Symbol::new(str::from_utf8_unchecked(slice::from_raw_parts(ptr, size)));
            }

            if text_len + self.tail_offset.get() > SEGMENT_CAPACITY {
                self.tail_offset.set(0);
                let prev_tail = self.tail.replace(alloc::alloc(BUFFER_LAYOUT));
                (*self.small_symbols.get()).push(prev_tail);
            }

            let tail_offset = self.tail_offset.get();
            let dst = self.tail.get().add(tail_offset);
            ptr::copy_nonoverlapping(text.as_ref().as_ptr(), dst, text_len);
            self.tail_offset.replace(tail_offset + text_len);
            Symbol::new(str::from_utf8_unchecked(slice::from_raw_parts(
                dst, text_len,
            )))
        }
    }
}

impl Drop for SymbolTable {
    fn drop(&mut self) {
        unsafe {
            alloc::dealloc(self.tail.get(), BUFFER_LAYOUT);
            for segment in self.small_symbols.get_mut().drain(..) {
                alloc::dealloc(segment as *mut _, BUFFER_LAYOUT);
            }
            for (ptr, size, capacity) in self.large_symbols.get_mut().drain(..) {
                String::from_raw_parts(ptr as *mut _, size, capacity);
            }
        }
    }
}

impl Default for SymbolTable {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::{Symbol, SymbolTable, LARGE_SYMBOL_THRESHOLD};
    use quickcheck_macros::quickcheck;
    use std::{iter, ptr};

    #[test]
    fn two_symbols_are_different() {
        let table = SymbolTable::new();
        assert_ne!(table.intern("laura"), table.intern("maddy"));
    }

    #[test]
    fn empty_symbol_is_different_from_other_symbols() {
        {
            let table = SymbolTable::new();
            assert_ne!(table.intern(""), table.intern("laura"));
        }
        {
            let table = SymbolTable::new();
            assert_ne!(table.intern("laura"), table.intern(""));
        }
    }

    #[test]
    fn interning_a_single_null_byte_works() {
        let table = SymbolTable::new();
        assert_eq!(table.intern("\0"), table.intern("\0"));
    }

    #[test]
    fn interning_a_large_string() {
        let text = iter::repeat('a')
            .take(2 * LARGE_SYMBOL_THRESHOLD + 7)
            .collect::<String>();
        let table = SymbolTable::new();
        assert_eq!(table.intern(&text), table.intern(text));
    }

    #[test]
    fn interning_can_refer_to_previous_segment() {
        let table = SymbolTable::new();
        let symbol = table.intern("laura");
        for c in 'a'..'z' {
            table.intern(iter::repeat(c).take(234).collect::<String>());
        }
        assert_eq!(symbol, table.intern("laura"));
    }

    #[quickcheck]
    #[cfg_attr(miri, ignore)]
    fn interning_twice_returns_same_symbol(texts: Vec<String>) -> bool {
        let table = SymbolTable::new();
        let symbols = texts
            .iter()
            .map(|text| table.intern(text))
            .collect::<Vec<_>>();
        symbols.into_iter().zip(texts.into_iter()).into_iter().all(
            |(Symbol { ptr: expected, .. }, text)| {
                let Symbol { ptr: actual, .. } = table.intern(text);
                ptr::eq(expected, actual)
            },
        )
    }
}
