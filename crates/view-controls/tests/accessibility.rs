#![cfg(feature = "accessibility")]
use accesskit::{Action, ActionData, ActionRequest, TreeId};
use view_controls::{accessibility::*, *};
use view_core::*;
use view_text::{EditorConfig, EditorSession, Selection, TextIndex};
fn request(target: accesskit::NodeId, action: Action, data: Option<ActionData>) -> ActionRequest {
    ActionRequest {
        action,
        target_tree: TreeId::ROOT,
        target_node: target,
        data,
    }
}
#[test]
fn semantic_updates_actions_graphemes_and_generational_removal() {
    let mut rt: Runtime<(), ()> = Runtime::new(16).unwrap();
    let (_, root) = rt
        .open_window(Description::new("root", (), |_, _, _| Dirty::NONE))
        .unwrap();
    let id = rt
        .mount(root, Description::new("field", (), |_, _, _| Dirty::NONE))
        .unwrap();
    let mut field = TextField::new("Name", "a🦀e\u{301}", EditorConfig::default()).unwrap();
    field.set_focused(true, 0);
    let revision = field.snapshot().text.revision();
    field
        .editor_mut()
        .select(
            revision,
            Selection {
                anchor: TextIndex(1),
                focus: TextIndex(5),
            },
        )
        .unwrap();
    let root_state = ControlSemantics::new(SemanticNode::new(root, Role::Window));
    let mut entries = vec![
        Entry {
            parent: None,
            state: root_state,
        },
        Entry {
            parent: Some(root),
            state: field.semantics(
                id,
                LogicalRect::new(LogicalPoint::new(10.0, 20.0), LogicalSize::new(100.0, 30.0)),
            ),
        },
    ];
    let mut bridge = SemanticBridge::default();
    let rev = Revision::INITIAL;
    let tree = bridge
        .commit(rev, &entries, ScaleFactor::new(2.0).unwrap())
        .unwrap();
    let native = bridge.id(id).unwrap();
    assert_eq!(tree.focus, native);
    let node = &tree.nodes.iter().find(|(id, _)| *id == native).unwrap().1;
    assert_eq!(node.bounds().unwrap().x0, 20.0);
    let selection = node.text_selection().unwrap();
    assert_eq!(selection.anchor.character_index, 1);
    assert_eq!(selection.focus.character_index, 2);
    let run = &tree
        .nodes
        .iter()
        .find(|(id, _)| *id == selection.anchor.node)
        .unwrap()
        .1;
    assert_eq!(run.character_lengths(), &[1, 4, 3]);
    let routed = bridge
        .route(
            rev,
            &request(
                native,
                Action::SetTextSelection,
                Some(ActionData::SetTextSelection(*selection)),
            ),
        )
        .unwrap();
    assert_eq!(
        routed.action,
        ControlAction::SetSelection(Selection {
            anchor: TextIndex(1),
            focus: TextIndex(5)
        })
    );
    assert!(
        bridge
            .route(rev, &request(native, Action::Click, None))
            .is_err()
    );
    field.set_modes(true, false);
    entries[1].state = field.semantics(id, entries[1].state.node.bounds);
    let rev2 = rev.checked_next().unwrap();
    bridge.commit(rev2, &entries, ScaleFactor::ONE).unwrap();
    assert!(
        bridge
            .route(rev, &request(native, Action::Focus, None))
            .is_err()
    );
    assert!(
        bridge
            .route(
                rev2,
                &request(
                    native,
                    Action::SetValue,
                    Some(ActionData::Value("bad".into()))
                )
            )
            .is_err()
    );
    assert!(
        bridge
            .route(
                rev2,
                &request(
                    native,
                    Action::SetTextSelection,
                    Some(ActionData::SetTextSelection(*selection))
                )
            )
            .is_ok()
    );
    rt.unmount(id).unwrap();
    entries.pop();
    bridge
        .commit(rev2.checked_next().unwrap(), &entries, ScaleFactor::ONE)
        .unwrap();
    assert!(bridge.id(id).is_none());
    assert!(
        bridge
            .route(rev2, &request(native, Action::Focus, None))
            .is_err()
    );
    let replacement = rt
        .mount(root, Description::new("field", (), |_, _, _| Dirty::NONE))
        .unwrap();
    assert_ne!(replacement, id);
}
#[test]
fn malformed_updates_are_atomic_and_disabled_actions_rejected() {
    let mut rt: Runtime<(), ()> = Runtime::new(16).unwrap();
    let (_, root) = rt
        .open_window(Description::new("root", (), |_, _, _| Dirty::NONE))
        .unwrap();
    let mut button = Button::new("Save");
    button.behavior.set_disabled(true);
    let entry = Entry {
        parent: None,
        state: button.semantics(root, LogicalRect::ZERO),
    };
    let mut bridge = SemanticBridge::default();
    let rev = Revision::INITIAL;
    bridge
        .commit(rev, std::slice::from_ref(&entry), ScaleFactor::ONE)
        .unwrap();
    let id = bridge.id(root).unwrap();
    assert_eq!(
        bridge.route(rev, &request(id, Action::Click, None)),
        Err(ControlError::Disabled)
    );
    assert!(
        bridge
            .commit(
                rev.checked_next().unwrap(),
                &[entry.clone(), entry],
                ScaleFactor::ONE
            )
            .is_err()
    );
    assert_eq!(bridge.revision(), Some(rev));
}
