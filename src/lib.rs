//! This is a fork of dtolnay's [`threadbound`](https://github.com/dtolnay/threadbound) crate, which allows implementing `Send` on non-`Copy` types.
//!
//! The reason that the original crate does not allow it is, if a `ThreadBound` object dropped on another thread from where it was created, it cannot be handled in any way.
//! Instead, this crate **aborts** if that happens: so be very cautious when using this crate on multi-threaded environemnt.
//!
//! ---
//!
//! [![github]](https://github.com/dtolnay/threadbound)&ensp;[![crates-io]](https://crates.io/crates/threadbound)&ensp;[![docs-rs]](https://docs.rs/threadbound)
//!
//! [github]: https://img.shields.io/badge/github-8da0cb?style=for-the-badge&labelColor=555555&logo=github
//! [crates-io]: https://img.shields.io/badge/crates.io-fc8d62?style=for-the-badge&labelColor=555555&logo=rust
//! [docs-rs]: https://img.shields.io/badge/docs.rs-66c2a5?style=for-the-badge&labelColor=555555&logo=docs.rs
//!
//! <br>
//!
//! [`ThreadAlone<T>`] is a wrapper that binds a value to its original thread.
//! The wrapper gets to be [`Sync`] and [`Send`] but only the original thread on
//! which the ThreadAlone was constructed can retrieve the underlying value.
//!
//! [`ThreadAlone<T>`]: struct.ThreadAlone.html
//! [`Sync`]: https://doc.rust-lang.org/std/marker/trait.Sync.html
//! [`Send`]: https://doc.rust-lang.org/std/marker/trait.Send.html
//!
//! # Example
//!
//! ```
//! use std::marker::PhantomData;
//! use std::rc::Rc;
//! use std::sync::Arc;
//! use threadalone::ThreadAlone;
//!
//! // Neither Send nor Sync. Maybe the index points into a
//! // thread-local interner.
//! #[derive(Copy, Clone)]
//! struct Span {
//!     index: u32,
//!     marker: PhantomData<Rc<()>>,
//! }
//!
//! // Error types are always supposed to be Send and Sync.
//! // We can use ThreadAlone to make it so.
//! struct Error {
//!     span: ThreadAlone<Span>,
//!     message: String,
//! }
//!
//! fn main() {
//!     let err = Error {
//!         span: ThreadAlone::new(Span {
//!             index: 99,
//!             marker: PhantomData,
//!         }),
//!         message: "fearless concurrency".to_owned(),
//!     };
//!
//!     // Original thread can see the contents.
//!     assert_eq!(err.span.get_ref().unwrap().index, 99);
//!
//!     let err = Arc::new(err);
//!     let err2 = err.clone();
//!     std::thread::spawn(move || {
//!         // Other threads cannot get access. Maybe they use
//!         // a default value or a different codepath.
//!         assert!(err2.span.get_ref().is_none());
//!     });
//!
//!     // Original thread can still see the contents.
//!     assert_eq!(err.span.get_ref().unwrap().index, 99);
//! }
//! ```

#![doc(html_root_url = "https://docs.rs/threadalone/0.1.0")]
#![allow(clippy::doc_markdown)]

use std::fmt::{self, Debug};
use std::io::{stderr, Write};
use std::pin::Pin;
use std::thread::{self, ThreadId};

use pin_project::{pin_project, pinned_drop};

/// ThreadAlone is a Sync-maker and Send-maker that allows accessing a value
/// of type T only from the original thread on which the ThreadAlone was
/// constructed.
///
/// Refer to the [crate-level documentation] for a usage example.
///
/// [crate-level documentation]: index.html
#[pin_project(PinnedDrop)]
pub struct ThreadAlone<T> {
    #[pin]
    value: Option<T>,
    thread_id: ThreadId,
}

unsafe impl<T> Sync for ThreadAlone<T> {}

// Unlike the original ThreadBound type, there is no T: Copy predicate, as dropping
// on another thread will never happen (in single-threaded context) and will
// abort if it really happens.
unsafe impl<T> Send for ThreadAlone<T> {}

impl<T> ThreadAlone<T> {
    /// Binds a value to the current thread. The wrapper can be sent around to
    /// other threads, but no other threads will be able to access the
    /// underlying value.
    pub fn new(value: T) -> Self {
        ThreadAlone {
            value: Some(value),
            thread_id: thread::current().id(),
        }
    }

    /// Accesses a reference to the underlying value if this is its original
    /// thread, otherwise `None`.
    pub fn get_ref(&self) -> Option<&T> {
        if thread::current().id() == self.thread_id {
            Some(self.value.as_ref().unwrap())
        } else {
            None
        }
    }

    /// Accesses a mutable reference to the underlying value if this is its
    /// original thread, otherwise `None`.
    pub fn get_mut(&mut self) -> Option<&mut T> {
        if thread::current().id() == self.thread_id {
            Some(self.value.as_mut().unwrap())
        } else {
            None
        }
    }

    /// Extracts ownership of the underlying value if this is its original
    /// thread, otherwise `None`.
    pub fn into_inner(mut self) -> Option<T> {
        if thread::current().id() == self.thread_id {
            Some(self.value.take().unwrap())
        } else {
            None
        }
    }
}

impl<T: Default> Default for ThreadAlone<T> {
    fn default() -> Self {
        ThreadAlone::new(Default::default())
    }
}

impl<T: Debug> Debug for ThreadAlone<T> {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        match self.get_ref() {
            Some(value) => Debug::fmt(value, formatter),
            None => formatter.write_str("unknown"),
        }
    }
}

#[pinned_drop]
impl<T> PinnedDrop for ThreadAlone<T> {
    fn drop(self: Pin<&mut Self>) {
        if thread::current().id() != self.thread_id {
            _ = writeln!(stderr(), "called Drop on another thread");
            std::process::abort();
        }
    }
}
