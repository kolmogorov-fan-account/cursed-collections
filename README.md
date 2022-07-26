# Cursed Collections

Collections that (seem) to break Rust safety.
* `SymbolTable`: a memory-efficient set of `String`, where its members can be equality compared in constant time.
* `AppendOnlyVec`: a sequence where elements can be appended even when you hold reference to previous elements.
* `LazyArray`: an array where elements can be initialized at a later time, even where reference to other, initialized
  elements exist.

## Safety

All collections in this crate are implemented with unsafe code. While I cannot be 100% sure the interface they offer is
safe, I use the following techniques to increase my confidence.
* Property based testing with [quickcheck](https://github.com/BurntSushi/quickcheck).
* Dynamic analysis with [Miri](https://github.com/rust-lang/miri).

## Documentation

https://docs.rs/cursed-collections/
