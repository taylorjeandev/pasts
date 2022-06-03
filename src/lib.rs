// Copyright © 2019-2022 The Pasts Contributors.
//
// Licensed under any of:
// - Apache License, Version 2.0 (https://www.apache.org/licenses/LICENSE-2.0)
// - MIT License (https://mit-license.org/)
// - Boost Software License, Version 1.0 (https://www.boost.org/LICENSE_1_0.txt)
// At your choosing (See accompanying files LICENSE_APACHE_2_0.txt,
// LICENSE_MIT.txt and LICENSE_BOOST_1_0.txt).
//
//! Minimal and simpler alternative to the futures crate.
//!
//! # Optional Features
//! The *`std`* feature is enabled by default, disable it to use on no-std.
//!
//! The *`web`* feature is disabled by default, enable it to use pasts within
//! the javascript DOM.
//!
//! # Getting Started
//!
//! Add the following to your **`./Cargo.toml`**:
//! ```toml
//! autobins = false
//!
//! [[bin]]
//! name = "app"
//! path = "app/main.rs"
//!
//! [dependencies]
//! pasts = "0.11"
//! aysnc-std = "1.11"
//! ```
//!
//! Create **`./app/main.rs`**:
//! ```rust,no_run
//! // Shim for providing async main
//! #[allow(unused_imports)]
//! use self::main::*;
//!
//! mod main {
//!     include!("../src/main.rs");
//!
//!     pub(super) mod main {
//!         pub(in crate) async fn main() {
//!             super::main().await
//!         }
//!     }
//! }
//!
//! fn main() {
//!     pasts::Executor::default().spawn(Box::pin(self::main::main::main()));
//! }
//! ```
//!
//! ## Multi-Tasking On Multiple Iterators of Futures
//! This example runs two timers in parallel using the `async-std` crate
//! counting from 0 to 6.  The "one" task will always be run for count 6 and
//! stop the program, although which task will run for count 5 may be either
//! "one" or "two" because they trigger at the same time.
//!
//! ```rust,no_run
//! # #[allow(unused_imports)]
//! # use self::main::*;
//! # mod main {
//! #
#![doc = include_str!("main.rs")]
//! #
//! #     pub(super) mod main {
//! #         pub(in crate) async fn main() {
//! #             super::main().await
//! #         }
//! #     }
//! # }
//! #
//! # fn main() {
//! #     pasts::Executor::default().spawn(Box::pin(self::main::main::main()));
//! # }
//! ```
#![cfg_attr(not(feature = "std"), no_std)]
#![doc(
    html_logo_url = "https://ardaku.github.io/mm/logo.svg",
    html_favicon_url = "https://ardaku.github.io/mm/icon.svg",
    html_root_url = "https://docs.rs/pasts"
)]
#![forbid(unsafe_code)]
#![warn(
    anonymous_parameters,
    missing_copy_implementations,
    missing_debug_implementations,
    missing_docs,
    nonstandard_style,
    rust_2018_idioms,
    single_use_lifetimes,
    trivial_casts,
    trivial_numeric_casts,
    unreachable_pub,
    unused_extern_crates,
    unused_qualifications,
    variant_size_differences
)]

extern crate alloc;

mod exec;
mod join;
mod noti;

use self::prelude::*;
pub use self::{
    exec::{Executor, Sleep, SpawnLocal},
    join::Join,
    noti::{Loop, Notifier, PollNextFn, Task},
};

/// An owned dynamically typed [`Notifier`] for use in cases where you can't
/// statically type your result or need to add some indirection.
///
/// Requires non-ZST allocator.
pub type BoxNotifier<'a, T> =
    Pin<Box<dyn Notifier<Event = T> + Unpin + Send + 'a>>;

/// [`BoxNotifier`], but without the [`Send`] requirement.
///
/// Requires non-ZST allocator.
pub type LocalNotifier<'a, T> = Pin<Box<dyn Notifier<Event = T> + Unpin + 'a>>;

/// An owned dynamically typed [`Task`] for use in cases where you can't
/// statically type your result or need to add some indirection.
///
/// Requires non-ZST allocator.
pub type BoxTask<'a, T> = Task<dyn Future<Output = T> + Send + 'a>;

/// [`BoxTask`], but without the [`Send`] requirement.
///
/// Requires non-ZST allocator.
pub type LocalTask<'a, T> = Task<dyn Future<Output = T> + 'a>;

pub mod prelude {
    //! Items that are almost always needed.

    #[doc(no_inline)]
    pub use alloc::boxed::Box;
    #[doc(no_inline)]
    pub use core::{
        future::Future,
        pin::Pin,
        task::{
            Context as TaskCx,
            Poll::{self, Pending, Ready},
        },
    };

    #[doc(no_inline)]
    pub use crate::{BoxNotifier, LocalNotifier, Notifier};
}
