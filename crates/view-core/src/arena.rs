use std::{
    num::NonZeroU64,
    sync::atomic::{AtomicU64, Ordering},
};

use crate::{ArenaHandle, ArenaId, CoreError, Generation};

// Identity issuance only, never application state. Values are never recycled.
static NEXT_ARENA: AtomicU64 = AtomicU64::new(1);

struct Slot<T> {
    generation: Generation,
    value: Option<T>,
}

/// Generational storage with a process-unique arena namespace.
///
/// Removal invalidates existing handles before a slot can be reused. An exhausted
/// generation retires the slot permanently. This arena has no UI ownership policy.
pub struct Arena<T> {
    id: ArenaId,
    slots: Vec<Slot<T>>,
    free: Vec<u32>,
}

impl<T> Arena<T> {
    /// Allocate a fresh namespace without platform dependencies.
    pub fn new() -> Result<Self, CoreError> {
        let raw = NEXT_ARENA
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |n| n.checked_add(1))
            .map_err(|_| CoreError::IdentityExhausted)?;
        Ok(Self {
            id: ArenaId::new(NonZeroU64::new(raw).expect("allocator begins at one")),
            slots: Vec::new(),
            free: Vec::new(),
        })
    }

    /// Namespace checked on every access.
    pub fn id(&self) -> ArenaId {
        self.id
    }

    /// Insert a value, reusing a vacant slot only with a new generation.
    pub fn insert(&mut self, value: T) -> Result<ArenaHandle, CoreError> {
        let index = if let Some(index) = self.free.pop() {
            self.slots[index as usize].value = Some(value);
            index
        } else {
            let index =
                u32::try_from(self.slots.len()).map_err(|_| CoreError::CapacityExhausted)?;
            self.slots.push(Slot {
                generation: Generation::INITIAL,
                value: Some(value),
            });
            index
        };
        Ok(ArenaHandle::from_parts(
            self.id,
            index,
            self.slots[index as usize].generation,
        ))
    }

    fn index(&self, handle: ArenaHandle) -> Result<usize, CoreError> {
        if handle.arena() != self.id {
            return Err(CoreError::WrongArena {
                expected: self.id,
                actual: handle.arena(),
            });
        }
        let index = handle.index() as usize;
        match self.slots.get(index) {
            Some(slot) if slot.generation == handle.generation() && slot.value.is_some() => {
                Ok(index)
            }
            _ => Err(CoreError::StaleHandle { handle }),
        }
    }

    /// Resolve a live handle, rejecting foreign, removed and recycled identities.
    pub fn get(&self, handle: ArenaHandle) -> Result<&T, CoreError> {
        Ok(self.slots[self.index(handle)?]
            .value
            .as_ref()
            .expect("validated occupancy"))
    }

    /// Resolve a live handle for mutation.
    pub fn get_mut(&mut self, handle: ArenaHandle) -> Result<&mut T, CoreError> {
        let index = self.index(handle)?;
        Ok(self.slots[index]
            .value
            .as_mut()
            .expect("validated occupancy"))
    }

    /// Remove a value and revoke the handle before any later insertion.
    pub fn remove(&mut self, handle: ArenaHandle) -> Result<T, CoreError> {
        let index = self.index(handle)?;
        let slot = &mut self.slots[index];
        let value = slot.value.take().expect("validated occupancy");
        if let Ok(next) = slot.generation.checked_next() {
            slot.generation = next;
            self.free.push(handle.index());
        }
        Ok(value)
    }

    /// Iterate live entries in slot order, independent of hash seeds.
    pub fn iter(&self) -> impl Iterator<Item = (ArenaHandle, &T)> {
        self.slots.iter().enumerate().filter_map(|(index, slot)| {
            slot.value.as_ref().map(|value| {
                (
                    ArenaHandle::from_parts(self.id, index as u32, slot.generation),
                    value,
                )
            })
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exhausted_slot_is_retired() {
        let mut arena = Arena::new().unwrap();
        let old = arena.insert(1).unwrap();
        let last = Generation::new(NonZeroU64::new(u64::MAX).unwrap());
        arena.slots[0].generation = last;
        let handle = ArenaHandle::from_parts(arena.id(), old.index(), last);
        assert_eq!(arena.remove(handle).unwrap(), 1);
        let next = arena.insert(2).unwrap();
        assert_ne!(next.index(), handle.index());
        assert!(arena.get(handle).is_err());
    }
}
