use view::{authoring::*, controls::*, *};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut ui = Runtime::<String, String>::new(8)?;
    let (_, root) = ui.open_window(
        Description::new("form", (), |_, model: &mut String, value: String| {
            if !value.trim().is_empty() {
                *model = value;
            }
            Dirty::BUILD
        })
        .structure(Structure::Declarative),
    )?;
    let field = TextInput::new("Name", "")?.0;
    let ids = reconcile(
        &mut ui,
        root,
        (
            Text::new("title", "Profile"),
            Description::new("name", field, |_, _, _| Dirty::NONE)
                .role(Role::TextInput)
                .focusable(true),
        ),
    )?;
    ui.update(ids[1], Dirty::PAINT, |field: &mut TextField| {
        field.insert_text("Ada", 0)
    })??;
    let value = ui
        .state::<TextField>(ids[1])?
        .snapshot()
        .text
        .text()
        .to_owned();
    ui.enqueue(root, value).map_err(|e| e.error)?;
    let mut model = String::new();
    ui.flush(&mut model)?;
    assert_eq!(model, "Ada");
    Ok(())
}
