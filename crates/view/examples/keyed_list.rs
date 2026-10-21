use view::{authoring::*, *};
fn main() -> Result<(), CoreError> {
    let mut ui = Runtime::<(), ()>::new(8)?;
    let (_, root) = ui.open_window(
        Description::new("app", (), |_, _, _| Dirty::NONE).structure(Structure::Declarative),
    )?;
    let rows = |keys| keyed(keys, |key| Text::new(key, key).into_element());
    let ids = reconcile(&mut ui, root, rows(["a", "b"]))?;
    assert_eq!(
        reconcile(&mut ui, root, rows(["b", "a"]))?,
        vec![ids[1], ids[0]]
    );
    Ok(())
}
