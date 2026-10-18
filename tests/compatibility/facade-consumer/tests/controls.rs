#![cfg(feature = "controls")]
use view::{
    Description, Dirty, Runtime,
    controls::{
        TextCommand, TextInput,
        view_text::{Clipboard, TextError},
    },
};
struct ClipboardFixture(String);
impl Clipboard for ClipboardFixture {
    fn read(&mut self) -> Result<String, TextError> {
        Ok(self.0.clone())
    }
    fn write(&mut self, s: &str) -> Result<(), TextError> {
        self.0 = s.into();
        Ok(())
    }
}
#[test]
fn public_control_is_runtime_owned_and_readonly_copyable() {
    let mut runtime: Runtime<(), ()> = Runtime::new(8).unwrap();
    let (_, root) = runtime
        .open_window(Description::new(
            "field",
            TextInput::new("name", "reader").unwrap(),
            |_, _, _| Dirty::PAINT,
        ))
        .unwrap();
    let mut clipboard = ClipboardFixture(String::new());
    runtime
        .update(root, Dirty::PAINT, |field: &mut TextInput| {
            field.0.set_modes(true, false);
            field
                .0
                .command(TextCommand::SelectAll, &mut clipboard, 0)
                .unwrap();
            field
                .0
                .command(TextCommand::Copy, &mut clipboard, 0)
                .unwrap();
            assert!(field.0.insert_text("blocked", 0).is_err());
        })
        .unwrap();
    assert_eq!(clipboard.0, "reader");
}
#[cfg(feature = "accessibility")]
#[test]
fn public_semantic_bridge_needs_no_native_loop() {
    let bridge = view::controls::accessibility::SemanticBridge::default();
    assert_eq!(bridge.revision(), None);
}
#[cfg(feature = "controls-render")]
#[test]
fn public_frame_is_deferred_until_explicit_gpu_submission() {
    let items = view::controls::frame(
        view::LogicalRect::new(view::LogicalPoint::ZERO, view::LogicalSize::new(50.0, 20.0)),
        &view::controls::Style::default(),
    );
    assert_eq!(items.len(), 2);
}
