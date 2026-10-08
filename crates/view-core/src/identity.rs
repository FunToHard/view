use std::num::NonZeroU64;

use crate::CoreError;

/// Identity of an arena within a runtime session.
///
/// The owning runtime must issue distinct values and never reuse them during
/// that session, even when an arena is destroyed. Allocation is not implemented
/// by this value type. IDs from different sessions must not be mixed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ArenaId(NonZeroU64);

impl ArenaId {
    /// Wrap an ID issued by the owning runtime; does not register an arena.
    pub const fn new(value: NonZeroU64) -> Self {
        Self(value)
    }

    /// Return the runtime-local arena number.
    pub const fn get(self) -> NonZeroU64 {
        self.0
    }
}

/// Nonzero incarnation of an arena slot.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Generation(NonZeroU64);

impl Generation {
    /// Generation of a slot's first occupant.
    pub const INITIAL: Self = Self(NonZeroU64::MIN);

    /// Reconstruct a generation; does not validate slot liveness.
    pub const fn new(value: NonZeroU64) -> Self {
        Self(value)
    }

    /// Return the incarnation number.
    pub const fn get(self) -> NonZeroU64 {
        self.0
    }

    /// Advance on reuse; an exhausted slot must be retired, never wrapped.
    pub fn checked_next(self) -> Result<Self, CoreError> {
        self.0
            .checked_add(1)
            .map(Self)
            .ok_or(CoreError::GenerationExhausted)
    }
}

/// An arena-qualified slot and generation, with no reference to its contents.
///
/// Consumers must validate arena, slot occupancy and generation before access.
/// Copying a handle neither extends lifetime nor keeps an object mounted.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ArenaHandle {
    arena: ArenaId,
    index: u32,
    generation: Generation,
}

impl ArenaHandle {
    /// Reconstruct a handle. Raw parts are untrusted until checked by the arena.
    pub const fn from_parts(arena: ArenaId, index: u32, generation: Generation) -> Self {
        Self {
            arena,
            index,
            generation,
        }
    }

    /// Arena that owns the slot.
    pub const fn arena(self) -> ArenaId {
        self.arena
    }

    /// Zero-based slot index; its range does not imply allocated capacity.
    pub const fn index(self) -> u32 {
        self.index
    }

    /// Incarnation that must match the live occupant.
    pub const fn generation(self) -> Generation {
        self.generation
    }
}

/// Framework window identity, independent of a native window handle.
///
/// Issued by the runtime's window arena. Closing/recreating a window changes its
/// generation even if a native window handle is reused. This type does not create
/// a window and is not interchangeable with an ordinary arena handle.
///
/// ```compile_fail
/// use view_core::{ArenaHandle, WindowId};
/// fn requires_window(_: WindowId) {}
/// fn wrong_kind(handle: ArenaHandle) { requires_window(handle); }
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct WindowId(ArenaHandle);

impl WindowId {
    /// Reconstruct an ID issued by a window arena; liveness is still unchecked.
    pub const fn from_handle(handle: ArenaHandle) -> Self {
        Self(handle)
    }

    /// Return the handle for diagnostics and validation by the window registry.
    pub const fn handle(self) -> ArenaHandle {
        self.0
    }
}
