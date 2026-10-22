use view::*;
fn main() -> Result<(), CoreError> {
    let mut ui = Runtime::<usize, usize>::new(8)?;
    let make = |key| {
        Description::new(key, (), |_, m: &mut usize, a| {
            *m += a;
            Dirty::BUILD
        })
    };
    let (first, a) = ui.open_window(make("main"))?;
    let (_, b) = ui.open_window(make("detached"))?;
    let mut document = 0;
    ui.enqueue(a, 1).map_err(|e| e.error)?;
    ui.enqueue(b, 2).map_err(|e| e.error)?;
    ui.flush(&mut document)?;
    ui.close_window(first)?;
    assert_eq!(document, 3);
    Ok(())
}
