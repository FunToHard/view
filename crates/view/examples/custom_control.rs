use view::{authoring::*, *};
fn badge<M, A>(key: &str, value: usize) -> Element<M, A> {
    Description::new(key, (), |_, _, _| Dirty::NONE)
        .role(Role::StaticText)
        .name(format!("Count: {value}"))
        .layout(LogicalRect::from_xywh(0., 0., 120., 28.))
        .into_element()
}
fn main() -> Result<(), CoreError> {
    let mut ui = Runtime::<(), ()>::new(8)?;
    let (_, root) = ui.open_window(
        Description::new("root", (), |_, _, _| Dirty::NONE).structure(Structure::Declarative),
    )?;
    reconcile(&mut ui, root, (badge("counter", 3),))?;
    ui.flush(&mut ())?;
    assert!(
        ui.snapshot()
            .nodes
            .iter()
            .any(|n| n.name.as_deref() == Some("Count: 3"))
    );
    Ok(())
}
