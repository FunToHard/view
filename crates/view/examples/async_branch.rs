use view::{authoring::*, resources::*, *};
fn branch(resource: &Resource<Vec<String>>) -> Element<(), ()> {
    if let Some(items) = resource.value() {
        Column::new("results", keyed(items, |s| Text::new(s, s).into_element())).into_element()
    } else {
        Text::new("status", format!("{:?}", resource.status())).into_element()
    }
}
fn main() -> Result<(), CoreError> {
    let mut ui = Runtime::<(), ()>::new(8)?;
    let (_, root) = ui.open_window(
        Description::new("root", (), |_, _, _| Dirty::NONE).structure(Structure::Declarative),
    )?;
    let lifetime = ui
        .own(root, ResourceKind::Subscription, || {})?
        .cancellation();
    let mut data = Resource::new(root, lifetime, Policy::default(), 1)?;
    let request = data.begin("catalog")?;
    reconcile(&mut ui, root, (branch(&data),))?;
    data.complete(&request, vec!["One".into()])?;
    reconcile(&mut ui, root, (branch(&data),))?;
    ui.flush(&mut ())?;
    Ok(())
}
