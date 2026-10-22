use view::{viewport::*, *};
fn main() -> Result<(), CoreError> {
    let mut ui = Runtime::<(), ()>::new(8)?;
    let (_, root) = ui.open_window(
        Description::new("viewport", (), |_, _, _| Dirty::NONE)
            .layout(LogicalRect::from_xywh(0., 0., 100., 100.)),
    )?;
    ui.flush(&mut ())?;
    let mut frames = ViewportScheduler::new(1);
    frames.register(root, FramePolicy::OnDemand)?;
    assert_eq!(
        frames.poll(ui.snapshot(), std::time::Duration::ZERO, |_| true)?,
        vec![root]
    );
    assert!(
        frames
            .poll(ui.snapshot(), std::time::Duration::ZERO, |_| true)?
            .is_empty()
    );
    Ok(())
}
