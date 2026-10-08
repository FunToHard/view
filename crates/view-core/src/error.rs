use std::fmt;

use crate::{ArenaHandle, ArenaId};

/// Foundation failures. Further runtime contracts may introduce more variants.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum CoreError {
    /// Process-local identity space is exhausted.
    IdentityExhausted,
    /// Arena cannot represent another slot.
    CapacityExhausted,
    /// A required bounded queue is full; retry with the returned action.
    QueueFull,
    /// The operation conflicts with the parent's structural ownership.
    StructuralOwnership,
    /// Sibling descriptions contain the same key.
    DuplicateKey,
    /// The requested component state has another Rust type.
    StateTypeMismatch,
    /// An owned request has been cancelled, superseded or already delivered.
    InactiveRequest,
    /// A presentation acknowledgement is ahead of committed state or goes backwards.
    InvalidPresentation,
    /// A revision cannot advance without reusing an older value.
    RevisionExhausted,
    /// A slot cannot be reused; the owner must retire it permanently.
    GenerationExhausted,
    /// A handle refers to an absent slot or an obsolete occupant.
    StaleHandle {
        /// Rejected handle, retained for diagnostics.
        handle: ArenaHandle,
    },
    /// A handle belongs to another arena in this runtime session.
    WrongArena {
        /// Arena receiving the operation.
        expected: ArenaId,
        /// Arena named by the supplied handle.
        actual: ArenaId,
    },
}

impl fmt::Display for CoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::IdentityExhausted => f.write_str("identity space exhausted"),
            Self::CapacityExhausted => f.write_str("arena capacity exhausted"),
            Self::QueueFull => f.write_str("bounded queue full"),
            Self::StructuralOwnership => {
                f.write_str("operation conflicts with structural ownership")
            }
            Self::DuplicateKey => f.write_str("duplicate sibling key"),
            Self::StateTypeMismatch => f.write_str("component state type mismatch"),
            Self::InactiveRequest => f.write_str("request is no longer active"),
            Self::InvalidPresentation => f.write_str("invalid presentation acknowledgement"),
            Self::RevisionExhausted => f.write_str("revision exhausted"),
            Self::GenerationExhausted => f.write_str("slot generation exhausted; retire the slot"),
            Self::StaleHandle { handle } => write!(f, "stale arena handle: {handle:?}"),
            Self::WrongArena { expected, actual } => {
                write!(f, "wrong arena: expected {expected:?}, received {actual:?}")
            }
        }
    }
}

impl std::error::Error for CoreError {}
