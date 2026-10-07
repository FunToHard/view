# Rust authoring API

Status: selected syntax/state baseline P-02/P-13/P-19. Revision: 0.4. Exact implementation signatures are qualified through compiler-tested examples in the project plan.

## Design objective

Capture Flutter's readable nested composition while preserving ordinary Rust authoring, rustfmt, IDE navigation, and understandable compiler errors. Flutter's immutable descriptions and persistent runtime elements are useful precedent, not a requirement to reproduce its implementation. [Flutter architecture](https://docs.flutter.dev/resources/architectural-overview)

All examples below are hypothetical API sketches. No named type, function, macro, crate, or trait has been implemented or compile-checked.

## Low-friction authoring for humans and AI tools

Goal: a Rust-capable developer or model with no view-specific training can use a short, version-matched reference and ordinary compiler feedback to write correct code. Zero friction or reliable API guessing cannot be guaranteed by syntax alone. Optimize discoverability and correction instead of assuming the model has memorized the framework.

Selected authoring rules:

- Ordinary Rust builders, functions, structs, enums, match/if and iterators form the complete authoring surface. No mandatory custom language or procedural macro for normal UI.
- One canonical spelling for each common operation: creation, properties, children, stable keys, events, and state binding. Avoid several nearly synonymous DSLs in tutorials.
- Constructors require essential values; builder methods set optional properties. Defaults, units, ownership, and event timing are documented together.
- Named public types hide deep internal tuple/closure/generic plumbing at suitable boundaries. Conditional children use explicit Element conversion with examples and useful diagnostics; a typed Either optimization is deferred until measurements justify it.
- State/effects are visible. Building describes UI; event handlers dispatch actions; resources deliver versioned results. Avoid hidden effects, implicit global context, or dependencies on call order that examples conceal.
- Standard controls include standard behavior. An example TextInput must not require the author to invent deletion/selection handlers.
- Third-party controls follow the same conventions and can be described by the same documentation/tooling interface.

A compact onboarding pack should contain a working minimal app, a form, keyed dynamic children, loading/error branches, shared state, an immediate region, a custom control crate, and a viewport. Each example declares its exact version/features and imports; placeholders are labeled and are not advertised as runnable.

Once implementation exists, primary runnable examples become compiler-checked fixtures and appropriate rustdoc tests. Rustdoc can test code examples, but illustrative pseudocode and intentionally failing examples must be categorized explicitly. The current design sketches are not yet those tests. [Rustdoc tests](https://doc.rust-lang.org/rustdoc/write-documentation/documentation-tests.html)

## Discoverable API reference

Publish a small quick reference plus versioned control/property/event/capability descriptions. Generate compatible portions from shared metadata where practical; do not let a hand-written model-only API diverge from Rust signatures. Diagnostic messages should identify the missing bound/property, expected ownership, and a minimal valid example.

Optional read-only developer tools can describe an installed control, retrieve a version-matched example, and show required features or unavailable platform capabilities. MCP is one access path; local docs and normal Cargo tooling remain sufficient. Installing or using an AI client is not required to build applications.

Test onboarding with readers/models given only that pack: build a form, branch on a resource, consume a community control, and repair a deliberate error. Track compile success, behavior tests, invented API calls, correction steps, and documentation gaps. Report the task, reference version and evaluation conditions; do not translate a small evaluation into a claim that every untrained model can write view correctly.

The builder syntax is the canonical baseline. Compiler-tested API exercises will refine signatures and ergonomics without introducing a competing default DSL. Familiarity alone does not establish compile time, generated code size, borrow ergonomics, or accessibility behavior.

## Canonical builders and heterogeneous tuples

```rust
fn inspector(model: &InspectorModel) -> impl View {
    Column::new((
        Text::new("Transform").size(20.0),
        NumberField::new(model.position.x)
            .label("Position X")
            .on_change(Action::SetPositionX),
        Button::new("Reset")
            .test_id("reset-transform")
            .on_press(Action::ResetTransform),
    ))
    .gap(12.0)
    .padding(16.0)
}
```

A tuple-to-children trait can accept differing fixed child types. Dynamic children require an iterator/collection abstraction, and conditional branches require an explicit sum/erasure strategy. The design must account for compile time and code size, not just allocation count.

Network-driven UI is an important branch case: Loading, Ready, and Failed return different concrete widgets. The selected Element conversion lets ordinary match/if remain usable. An effect updates the model and invalidates its consumers; build/layout do not start requests. The [network and branching document](11-network-ui-and-web.md) includes the full example and branch-lifetime policy.

## Property structs for complex configuration

```rust
Column::with_props(ColumnProps {
    gap: 12.0,
    padding: Insets::all(16.0),
    ..Default::default()
})
.children((
    Text::new("Transform"),
    Button::new("Reset").on_press(Action::ResetTransform),
))
```

This retains named fields with ordinary Rust syntax, at the cost of more type/default syntax. It can coexist with builders if there is one canonical property model. Rust does not provide Dart-style named function arguments; a literal imitation would need macros or a different surface.

An optional macro DSL is deferred until the ordinary Rust API works. It must lower into the same public contracts, not become the only way to author third-party widgets.

## Retained and immediate examples

```rust
// Retained creation and later mutation through a runtime context.
let status = ui.mount(parent, Text::new("Ready"));
ui.update(status, |text| text.set_content("Importing..."));

// An on-demand immediate update region, separate from pure builds.
ui.run_region(inspector_region, model_revision, |ui, model| {
    if ui.button("reset", "Reset").clicked() {
        model.dispatch(Action::ResetTransform);
    }
    ui.label("selection", model.selection_label());
});
```

The region's owner registers how it can be scheduled again. A component registry stores owned state/callbacks and receives current model context only during dispatch; it does not retain an arbitrary borrowed closure indefinitely. Typed actions cross back to the application dispatcher. This is the selected ownership model; no global Rc<RefCell<_>> application model is required.

Immediate responses have an event sequence and are consumed at most once. Pure measurement and paint callbacks cannot consume them. If an action changes the region's own model, a subsequent pure description pass or a new update transaction reflects the result without replaying the action.

## Rich application composition

```rust
fn workspace(model: &Workspace) -> impl View {
    SplitPane::horizontal(
        SceneViewport::new(model.scene_handle())
            .camera(model.camera_handle())
            .on_pick(Action::SelectObject)
            .overlay(TransformGizmo::new(model.selection())),
        Inspector::new(model.selection()),
    )
    .ratio(0.75)
}
```

The viewport mounts a render provider; it does not copy the scene into widget state. A corresponding Canvas2D view should support document coordinates, pan/zoom, picking, and semantic items. An external engine can provide the viewport instead of using the first-party scene module.

A 3D object selection action can update the retained inspector; dragging a retained numeric control can update the scene. Undo belongs to the application's command/document layer. A drag may preview many changes while committing one undoable command.

## State and handler contracts

- Typed actions are the default. Owned callbacks adapt component events to actions without forcing applications into a single global enum.
- Component state survives compatible keyed rebuilds; document state has an independent lifetime.
- Callback storage must not require every application model to be wrapped in Rc<RefCell<_>> or Arc<Mutex<_>>.
- Type erasure is permitted at deliberate runtime boundaries. A fully generic tree is not an assumed performance win.
- Styles have typed values. Inheritance and local overrides must have documented resolution and invalidation rules.
- Layout constraints remain explicit where needed; native defaults can supply sensible sizes and text metrics.
- Application commands, shortcuts, semantic activation, and automation can share action routing without conflating physical input with direct actions.

## Required API review exercises

During each owning milestone, review and compile-test complete examples before stabilizing the affected public API:

1. A validating form with async save, cancellation, focus, and IME.
2. A keyed list that reorders without moving selection/edit state to another item.
3. An editor with shared document state in two windows.
4. A continuous 3D viewport next to an idle immediate inspector.
5. A brush gesture with preview, pointer capture, cancellation, and one undo step.
6. A third-party widget with custom layout, paint, semantics, and automation.
7. Network search with out-of-order replies, refresh over retained content, cancellation, and retry.
8. Application-defined automation commands sharing the UI action/undo path.
9. A Markdown preview control published/consumed as a separate crate.
10. A runtime add-on panel using a guest SDK with an explicit host protocol boundary.

Acceptance criteria: no hidden frame loop, repeated effect, mandatory global mutable state, or application-side platform patch is needed to express these workflows. Unresolved ergonomics become design questions rather than implementation shortcuts.

See [control crates and conformance](13-controls-and-conformance.md) for the behavior expected from standard controls and [runtime add-ons](14-runtime-addons.md) for the different boundary needed by independently installed extensions.
