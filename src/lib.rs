//! Collections that (seem) to break Rust safety.
//!
//! Mutating a `Vec` as we still hold references to it's element is forbidden by the borrow checker.
//! The reason is that the vector could grow and move its elements to a new buffer. Our poor
//! references would point then  point to invalid memory. But, what if you know you don't need more
//! elements? `Vec` doesn't know about it, so that is where `cursed-collections` comes to the
//! rescue. This create offers different collections that offer an extremely narrow interface in
//! exchange of doing things that are unusual in safe rust.
//!
//! All collections in this crate are extremely cursed, yet respect the safety guaranties of Rust…
//! assuming they are bug free!

mod append_only_vec;
mod lazy_array;

pub use crate::append_only_vec::AppendOnlyVec;
pub use crate::lazy_array::LazyArray;
