//! Backend-independent foundation contracts for view.
//!
//! IDs are runtime-local values, not document keys, authorization tokens or
//! cross-process addresses. Constructing an ID does not establish liveness.
//! Generational storage and the owner-thread runtime validate access and own
//! component lifecycle, actions, invalidation and coherent structural snapshots.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

mod arena;
mod error;
pub mod focus;
pub mod geometry;
pub mod hit_test;
mod identity;
pub mod input;
pub mod layout;
mod revision;
mod runtime;
mod runtime_types;
pub mod scroll;
pub mod semantics;

pub use arena::Arena;
pub use error::CoreError;
pub use focus::*;
pub use geometry::*;
pub use hit_test::*;
pub use identity::{ArenaHandle, ArenaId, Generation, WindowId};
pub use input::*;
pub use layout::*;
pub use revision::Revision;
pub use runtime::Runtime;
pub use runtime_types::*;
pub use scroll::*;
pub use semantics::*;
