use view::{authoring::*, *};
fn main() -> Result<(), CoreError> {
    let mut ui = Runtime::<(), ()>::new(8)?;
    let (_, root) = ui.open_window(
        Description::new("app", (), |_, _, _| Dirty::NONE).structure(Structure::Declarative),
    )?;
    reconcile(
        &mut ui,
        root,
        (Column::new(
            "content",
            (
                Text::new("title", "Hello view"),
                Text::new("body", "Pure Rust composition"),
            ),
        ),),
    )?;
    ui.flush(&mut ())?;
    assert_eq!(ui.snapshot().nodes.len(), 4);
    Ok(())
}
