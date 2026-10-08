use std::{collections::HashSet, num::NonZeroU64};

use view_core::{ArenaHandle, ArenaId, CoreError, Generation, Revision, WindowId};

#[test]
fn revision_exhaustion_does_not_alias_an_earlier_observation() {
    let initial = Revision::INITIAL;
    let changed = initial.checked_next().unwrap();
    assert!(changed > initial);
    assert_eq!(initial.get(), 0);
    let last = Revision::from_raw(u64::MAX - 1).checked_next().unwrap();
    assert_eq!(last.get(), u64::MAX);
    assert_eq!(last.checked_next(), Err(CoreError::RevisionExhausted));
    assert_eq!(last.get(), u64::MAX);
}

#[test]
fn identity_distinguishes_arena_slot_and_incarnation() {
    let first_arena = ArenaId::new(NonZeroU64::new(1).unwrap());
    let second_arena = ArenaId::new(NonZeroU64::new(2).unwrap());
    let first = ArenaHandle::from_parts(first_arena, 0, Generation::INITIAL);
    let next_slot = ArenaHandle::from_parts(first_arena, 1, Generation::INITIAL);
    let other_arena = ArenaHandle::from_parts(second_arena, 0, Generation::INITIAL);
    let reused =
        ArenaHandle::from_parts(first_arena, 0, Generation::INITIAL.checked_next().unwrap());
    assert_eq!(
        HashSet::from([first, next_slot, other_arena, reused]).len(),
        4
    );
    assert_ne!(WindowId::from_handle(first), WindowId::from_handle(reused));
    // These are identity comparisons, not evidence of an implemented arena.
}

#[test]
fn generation_exhaustion_requires_retirement() {
    let penultimate = Generation::new(NonZeroU64::new(u64::MAX - 1).unwrap());
    let last = penultimate.checked_next().unwrap();
    assert_eq!(last.get().get(), u64::MAX);
    assert_eq!(last.checked_next(), Err(CoreError::GenerationExhausted));
    assert_ne!(last, Generation::INITIAL);
}
