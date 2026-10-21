use view::*;
fn main() -> Result<(), CoreError> {
    let mut ui = Runtime::<usize, usize>::new(8)?;
    let (_, root) = ui.open_window(
        Description::new("region", (), |_, m: &mut usize, a| {
            *m += a;
            Dirty::BUILD
        })
        .structure(Structure::Immediate),
    )?;
    ui.set_region_builder(root, |m, _| {
        vec![Description::new("value", (), |_, _, _| Dirty::NONE).name(m.to_string())]
    })?;
    let mut model = 0;
    ui.flush(&mut model)?;
    ui.enqueue(root, 1).map_err(|e| e.error)?;
    ui.flush(&mut model)?;
    ui.flush(&mut model)?;
    assert_eq!(model, 1);
    Ok(())
}
