// Compile against the facade without workspace qualification dependencies.
use std::num::NonZeroU64;
use view::{ArenaHandle, ArenaId, CoreError, Generation, Revision, WindowId};
use view::{Description, Dirty, Runtime};

fn main() -> Result<(), CoreError> {
    let arena = ArenaId::new(NonZeroU64::new(1).unwrap());
    let handle = ArenaHandle::from_parts(arena, 0, Generation::INITIAL);
    let window = WindowId::from_handle(handle);
    assert_eq!(window.handle().arena(), arena);
    assert_eq!(window.handle().index(), 0);
    assert_eq!(window.handle().generation(), Generation::INITIAL);
    assert_eq!(Revision::INITIAL.checked_next()?.get(), 1);
    let mut runtime = Runtime::new(4)?;
    let (_, root) = runtime.open_window(Description::new(
        "root",
        (),
        |_, model: &mut i32, action: i32| {
            *model += action;
            Dirty::NONE
        },
    ))?;
    let mut model = 0;
    runtime
        .enqueue(root, 7)
        .map_err(|rejected| rejected.error)?;
    assert_eq!(runtime.flush(&mut model)?.dispatched, 1);
    assert_eq!(model, 7);
    assert!(!runtime.has_work());
    Ok(())
}
