# Lazy Array

`LazyArray` is a data structure that contains a fixed number of entries that are indexed by a `usize` from 0 up to 1 before its size. All entries start by being undefined, that means that trying to `get` them will return `None`. Once for each entry, they can be set and once set they can never change.

## Example Use Case

I had the idea of this library as I was parsing a binary format that contains a sequence of record that may reference each other to form an acyclic graph. The problem is that I can't _a priori_ know in which order to load the objects as any object can reference objects before and after itself. With `LazyArray` I can lazily load objects as I discover dependencies between them and store them so they are only loaded once.

## Why I Think It Might Be Safe

`LazyArray` is implemented by storing a `Vec` inside an `UnsafeCell`. I think it is fine for the following reasons.

* The number of entries is allocated as once when it is created, which means that the storage of entries should never move.
* If an entry is undefined, it is impossible to reference the `None` from outside.
* If an entry is defined, there may exist references to it, but its value can never change.
* It is not `Sync`.
