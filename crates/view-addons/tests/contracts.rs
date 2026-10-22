use std::{
    collections::BTreeMap,
    io::Cursor,
    time::{Duration, Instant},
};
use view_addons::*;
use view_controls::view_text::MemoryClipboard;
use view_core::*;
#[derive(Default)]
struct Doc {
    revision: u64,
    text: String,
    undo: Vec<String>,
    reads: usize,
}
impl Application for Doc {
    fn snapshot(&mut self) -> view_addons::Result<DocumentSnapshot> {
        self.reads += 1;
        Ok(DocumentSnapshot {
            revision: self.revision,
            value: serde_json::json!({"text":self.text}),
        })
    }
    fn edit(
        &mut self,
        name: &str,
        revision: u64,
        args: serde_json::Value,
    ) -> view_addons::Result<DocumentSnapshot> {
        if name != "replace" {
            return Err(Error::Denied);
        }
        if revision != self.revision {
            return Err(Error::Stale);
        }
        let replacement = args
            .get("replacement")
            .and_then(|v| v.as_str())
            .ok_or(Error::Denied)?;
        self.undo.push(self.text.clone());
        self.text = replacement.into();
        self.revision += 1;
        self.snapshot()
    }
}
fn validate(value: &serde_json::Value) -> view_addons::Result<()> {
    if value.get("replacement").and_then(|v| v.as_str()).is_none() {
        Err(Error::Denied)
    } else {
        Ok(())
    }
}
fn manifest(mode: &str) -> Manifest {
    Manifest {
        id: "review".into(),
        version: "1".into(),
        protocol: VERSION,
        executable: env!("CARGO_BIN_EXE_review-addon").into(),
        arguments: vec![mode.into()],
        scopes: [
            Scope::Ui,
            Scope::DocumentRead,
            Scope::DocumentEdit,
            Scope::Automation,
        ]
        .into(),
        slots: [Slot::SidePanel, Slot::Toolbar, Slot::Menu].into(),
        components: ["label", "button", "text"]
            .into_iter()
            .map(|s| ComponentRequirement {
                name: s.into(),
                version: 1,
                required: true,
                fallback: None,
            })
            .collect(),
        commands: ["accept".into()].into(),
    }
}
struct Fixture {
    rt: Runtime<(), ()>,
    root: NodeId,
    window: WindowId,
    addon: Addon,
    doc: Doc,
    start: Instant,
}
impl Fixture {
    fn new(mode: &str, limits: Limits) -> Self {
        Self::with_manifest(
            manifest(mode),
            limits,
            Grants {
                scopes: manifest(mode).scopes,
                actions: [(
                    "replace".into(),
                    validate as fn(&serde_json::Value) -> view_addons::Result<()>,
                )]
                .into(),
                automation_commands: ["accept".into()].into(),
            },
            Registry::standard(),
        )
    }
    fn with_manifest(m: Manifest, limits: Limits, grants: Grants, registry: Registry) -> Self {
        let mut rt = Runtime::new(64).unwrap();
        let (window, root) = rt
            .open_window(Description::new("app", (), |_, _, _| Dirty::NONE))
            .unwrap();
        let owner = rt
            .own(root, ResourceKind::Task, || {})
            .unwrap()
            .cancellation();
        let mut parents = BTreeMap::new();
        for slot in [Slot::SidePanel, Slot::Toolbar, Slot::Menu] {
            parents.insert(
                slot.clone(),
                rt.mount(
                    root,
                    Description::new(format!("slot-{slot:?}"), (), |_, _, _| Dirty::NONE),
                )
                .unwrap(),
            );
        }
        let mut addon = Addon::install(m, grants, registry, parents, owner, limits).unwrap();
        addon.activate().unwrap();
        Self {
            rt,
            root,
            window,
            addon,
            doc: Doc {
                revision: 1,
                text: "teh".into(),
                ..Default::default()
            },
            start: Instant::now(),
        }
    }
    fn poll(&mut self) -> view_addons::Result<()> {
        let r = self
            .addon
            .poll(&mut self.rt, &mut self.doc, self.start.elapsed());
        self.rt.flush(&mut ()).unwrap();
        r
    }
    fn until(&mut self, condition: impl Fn(&Self) -> bool) {
        while !condition(self) {
            assert!(
                self.start.elapsed() < Duration::from_secs(15),
                "{:?} {:?}",
                self.addon.state(),
                self.addon.failure()
            );
            let _ = self.poll();
            std::thread::sleep(Duration::from_millis(2));
        }
    }
    fn ready(&mut self) {
        self.until(|s| s.addon.revision() == 1 && s.doc.reads > 0);
    }
    fn accept(&mut self, message: GuestMessage) -> view_addons::Result<()> {
        self.addon.accept(
            &mut self.rt,
            &mut self.doc,
            GuestEnvelope {
                generation: self.addon.generation(),
                message,
            },
        )
    }
    fn text(&self) -> String {
        let c = self.addon.ui().control("note").unwrap();
        let c = c.borrow();
        match &*c {
            Control::Text(t) => t.snapshot().text.text().into(),
            _ => panic!("text control"),
        }
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = self.addon.remove(&mut self.rt);
        let start = Instant::now();
        while !self
            .addon
            .receipt()
            .is_some_and(|r| r.exited && r.workers == 0)
            && start.elapsed() < Duration::from_secs(3)
        {
            std::thread::sleep(Duration::from_millis(2));
        }
    }
}

#[test]
fn actual_guest_review_command_edits_once_and_host_undo_is_independent() {
    let mut f = Fixture::new("--guest", Limits::default());
    f.ready();
    let command = f.addon.commands().pop().unwrap();
    assert_eq!(command.name, "review.accept");
    f.addon.invoke(&command).unwrap();
    f.until(|s| s.doc.undo.len() == 1);
    assert_eq!(f.doc.text, "the");
    assert_eq!(f.doc.undo.pop().unwrap(), "teh");
    assert!(f.addon.diagnostics().contains("trusted native"));
    f.addon.deactivate(&mut f.rt).unwrap();
    f.until(|s| {
        s.addon
            .receipt()
            .is_some_and(|r| r.exited && r.workers == 0)
    });
    assert!(f.addon.commands().is_empty());
    assert_eq!(f.addon.pending(), 0);
    assert!(
        f.rt.snapshot()
            .nodes
            .iter()
            .all(|n| !n.key.starts_with("addon/"))
    );
    assert!(f.addon.invoke(&command).is_err());
}
#[test]
fn framing_rejects_partial_oversized_unknown_and_trailing_messages() {
    assert!(matches!(
        read_frame::<GuestEnvelope>(&mut Cursor::new(((MAX_FRAME + 1) as u32).to_le_bytes())),
        Err(Error::Limit)
    ));
    assert!(read_frame::<GuestEnvelope>(&mut Cursor::new(vec![10, 0, 0, 0, b'{'])).is_err());
    for body in [
        br#"{"generation":1,"message":{"kind":"complete","event":1},"extra":1}"#.as_slice(),
        b"{} {}",
    ] {
        let mut bytes = (body.len() as u32).to_le_bytes().to_vec();
        bytes.extend_from_slice(body);
        assert!(read_frame::<GuestEnvelope>(&mut Cursor::new(bytes)).is_err());
    }
}
#[test]
fn real_process_failure_modes_revoke_ui_commands_and_reap_workers() {
    for mode in [
        "--wrong-version",
        "--oversized",
        "--malformed",
        "--crash",
        "--disconnect",
        "--late-generation",
        "--flood",
    ] {
        let mut f = Fixture::new(mode, Limits::default());
        f.until(|s| s.addon.state() == State::Failed);
        assert!(f.addon.failure().is_some(), "{mode}");
        assert!(f.addon.commands().is_empty());
        assert_eq!(f.addon.pending(), 0);
        assert!(
            f.rt.snapshot()
                .nodes
                .iter()
                .all(|n| !n.key.starts_with("addon/"))
        );
        f.until(|s| {
            s.addon
                .receipt()
                .is_some_and(|r| r.exited && r.workers == 0)
        });
        assert_eq!(f.doc.text, "teh");
    }
}
#[test]
fn ui_patch_is_atomic_and_stale_base_requests_resynchronization() {
    let mut f = Fixture::new("--guest", Limits::default());
    f.ready();
    let old = f.addon.ui().node("note").unwrap();
    f.accept(GuestMessage::Snapshot {
        base: 0,
        nodes: vec![],
    })
    .unwrap();
    assert_eq!(f.addon.revision(), 1);
    let mut unknown = UiNode::label("unknown", "bad");
    unknown.component = "native-dll".into();
    assert!(
        f.accept(GuestMessage::Patch {
            base: 1,
            changes: vec![
                Patch::Put {
                    node: UiNode::label("added", "must not appear")
                },
                Patch::Put { node: unknown }
            ]
        })
        .is_err()
    );
    assert_eq!(f.addon.revision(), 1);
    assert!(f.addon.ui().node("added").is_none());
    assert_eq!(f.addon.ui().node("note"), Some(old));
    f.accept(GuestMessage::Patch {
        base: 1,
        changes: vec![Patch::Remove {
            key: "suggestion".into(),
        }],
    })
    .unwrap();
    assert_eq!(f.addon.revision(), 2);
    assert_eq!(f.addon.ui().node("note"), Some(old));
}
#[test]
fn local_text_uses_shared_editing_semantics_and_survives_guest_echo() {
    let mut f = Fixture::new("--unack", Limits::default());
    f.ready();
    let note = f.addon.ui().node("note").unwrap();
    f.rt.set_focus(f.window, Some(note)).unwrap();
    f.rt.flush(&mut ()).unwrap();
    let mut clip = MemoryClipboard::default();
    f.addon
        .input(&mut f.rt, "note", Input::Text("a🙂".into(), 1), &mut clip)
        .unwrap();
    // No poll/IPC round trip between local editing operations.
    f.addon
        .input(
            &mut f.rt,
            "note",
            Input::Key(KeyboardEvent {
                key: Key::Named(NamedKey::Backspace),
                phase: KeyPhase::Down,
                modifiers: Modifiers::default(),
                timestamp: 2,
            }),
            &mut clip,
        )
        .unwrap();
    assert_eq!(f.text(), "a");
    f.addon
        .input(
            &mut f.rt,
            "note",
            Input::Preedit("日本".into(), Some((0, 6))),
            &mut clip,
        )
        .unwrap();
    f.accept(GuestMessage::Patch {
        base: 1,
        changes: vec![Patch::Put {
            node: UiNode::text("note", "Renamed note", ""),
        }],
    })
    .unwrap();
    assert_eq!(f.text(), "a");
    f.addon
        .input(&mut f.rt, "note", Input::Commit("日".into(), 3), &mut clip)
        .unwrap();
    assert_eq!(f.text(), "a日");
    f.rt.flush(&mut ()).unwrap();
    let semantics = f.rt.semantic_snapshot(f.window).unwrap();
    assert!(
        semantics.nodes.iter().any(|n| n.id == note
            && n.role == Role::TextInput
            && n.value.as_deref() == Some("a日"))
    );
    let changed = UiNode::text("note", "Renamed note", "stale overwrite");
    f.accept(GuestMessage::Patch {
        base: 2,
        changes: vec![Patch::Put { node: changed }],
    })
    .unwrap();
    assert_eq!(f.addon.revision(), 2);
    assert_eq!(f.text(), "a日");
    f.rt.capture_pointer(f.window, note).unwrap();
    f.addon.deactivate(&mut f.rt).unwrap();
    f.rt.flush(&mut ()).unwrap();
    assert_eq!(f.rt.focused(f.window).unwrap(), None);
    assert_eq!(f.rt.captured(f.window).unwrap(), None);
}
#[test]
fn document_scopes_revision_schema_and_request_dedup_are_enforced() {
    let mut f = Fixture::new("--guest", Limits::default());
    f.ready();
    let edit = |request, revision, action: &str, args| GuestMessage::EditDocument {
        request,
        revision,
        action: action.into(),
        arguments: args,
    };
    f.accept(edit(
        100,
        0,
        "replace",
        serde_json::json!({"replacement":"wrong"}),
    ))
    .unwrap();
    assert!(f.doc.undo.is_empty());
    f.accept(edit(
        100,
        1,
        "replace",
        serde_json::json!({"replacement":"duplicate"}),
    ))
    .unwrap();
    assert!(f.doc.undo.is_empty());
    f.accept(edit(
        101,
        1,
        "ungranted",
        serde_json::json!({"replacement":"wrong"}),
    ))
    .unwrap();
    assert!(f.doc.undo.is_empty());
    f.accept(edit(
        102,
        1,
        "replace",
        serde_json::json!({"unknown":"wrong"}),
    ))
    .unwrap();
    assert!(f.doc.undo.is_empty());
    f.accept(edit(
        103,
        1,
        "replace",
        serde_json::json!({"replacement":"valid"}),
    ))
    .unwrap();
    assert_eq!(f.doc.undo.len(), 1);
    assert_eq!(f.doc.text, "valid");
    f.accept(edit(
        104,
        1,
        "replace",
        serde_json::json!({"replacement":"late"}),
    ))
    .unwrap();
    assert_eq!(f.doc.undo.len(), 1);
    let mut m = manifest("--guest");
    m.scopes.remove(&Scope::DocumentEdit);
    m.scopes.remove(&Scope::Automation);
    let grants = Grants {
        scopes: m.scopes.clone(),
        actions: [(
            "replace".into(),
            validate as fn(&serde_json::Value) -> view_addons::Result<()>,
        )]
        .into(),
        automation_commands: ["accept".into()].into(),
    };
    let mut denied = Fixture::with_manifest(m, Limits::default(), grants, Registry::standard());
    denied.ready();
    denied
        .accept(edit(
            2,
            1,
            "replace",
            serde_json::json!({"replacement":"denied"}),
        ))
        .unwrap();
    assert!(denied.doc.undo.is_empty());
    assert!(denied.addon.commands().is_empty());
}
#[test]
fn timeout_restart_disable_owner_unmount_and_late_generation_cleanup() {
    let mut f = Fixture::new("--silent", Limits::default());
    assert_eq!(
        f.addon.poll(&mut f.rt, &mut f.doc, Duration::from_secs(6)),
        Err(Error::Timeout)
    );
    let start = Instant::now();
    while !f
        .addon
        .receipt()
        .is_some_and(|r| r.exited && r.workers == 0)
    {
        assert!(start.elapsed() < Duration::from_secs(5));
        std::thread::sleep(Duration::from_millis(2));
    }
    let mut f = Fixture::new("--guest", Limits::default());
    f.ready();
    let generation = f.addon.generation();
    let command = f.addon.commands().pop().unwrap();
    f.addon.deactivate(&mut f.rt).unwrap();
    f.until(|s| s.addon.state() == State::Installed);
    f.doc.reads = 0;
    f.addon.activate().unwrap();
    f.ready();
    assert_ne!(f.addon.generation(), generation);
    assert_eq!(
        f.addon.accept(
            &mut f.rt,
            &mut f.doc,
            GuestEnvelope {
                generation,
                message: GuestMessage::Snapshot {
                    base: 0,
                    nodes: vec![]
                }
            }
        ),
        Err(Error::Stale)
    );
    assert!(f.addon.invoke(&command).is_err());
    f.addon.disable(&mut f.rt).unwrap();
    assert_eq!(f.addon.state(), State::Disabled);
    assert!(f.addon.activate().is_err());
    f.addon.enable().unwrap();
    f.rt.close_window(f.window).unwrap();
    f.poll().unwrap();
    assert!(f.addon.activate().is_err());
}
#[test]
fn component_fallback_registration_limits_and_modal_policy_are_explicit() {
    let mut m = manifest("--guest");
    m.components.push(ComponentRequirement {
        name: "fancy".into(),
        version: 9,
        required: false,
        fallback: Some("label".into()),
    });
    let grants = Grants {
        scopes: m.scopes.clone(),
        ..Default::default()
    };
    let mut f = Fixture::with_manifest(m, Limits::default(), grants, Registry::standard());
    f.ready();
    let mut node = UiNode::label("custom", "fallback");
    node.component = "fancy".into();
    f.accept(GuestMessage::Patch {
        base: 1,
        changes: vec![Patch::Put { node }],
    })
    .unwrap();
    assert!(f.addon.ui().node("custom").is_some());
    let huge: Vec<_> = (0..129)
        .map(|i| UiNode::label(&format!("n{i}"), "x"))
        .collect();
    assert_eq!(
        f.accept(GuestMessage::Snapshot {
            base: 2,
            nodes: huge
        }),
        Err(Error::Limit)
    );
    assert_eq!(f.addon.revision(), 2);
    let modal =
        f.rt.mount(f.root, Description::new("modal", (), |_, _, _| Dirty::NONE))
            .unwrap();
    f.rt.set_modal_scope(f.window, Some(modal)).unwrap();
    f.rt.flush(&mut ()).unwrap();
    assert_eq!(
        f.addon.input(
            &mut f.rt,
            "accept",
            Input::Activate,
            &mut MemoryClipboard::default()
        ),
        Err(Error::Denied)
    );
}

#[test]
fn rate_task_pending_and_byte_limits_are_enforced_at_the_boundary() {
    let mut rate = Fixture::new(
        "--guest",
        Limits {
            messages_per_second: 1,
            ..Default::default()
        },
    );
    rate.until(|f| f.addon.state() == State::Failed);
    assert_eq!(rate.addon.failure(), Some(&Error::Limit));
    let mut f = Fixture::new(
        "--unack",
        Limits {
            pending: 1,
            task: Duration::from_secs(1),
            ..Default::default()
        },
    );
    f.ready();
    let note = f.addon.ui().node("note").unwrap();
    f.rt.set_focus(f.window, Some(note)).unwrap();
    f.rt.flush(&mut ()).unwrap();
    let mut clipboard = MemoryClipboard::default();
    f.addon
        .input(
            &mut f.rt,
            "note",
            Input::Text("a".into(), 1),
            &mut clipboard,
        )
        .unwrap();
    f.addon
        .input(
            &mut f.rt,
            "note",
            Input::Text("b".into(), 2),
            &mut clipboard,
        )
        .unwrap();
    assert_eq!(f.addon.pending(), 1);
    assert_eq!(f.text(), "ab");
    assert_eq!(
        f.addon
            .input(&mut f.rt, "accept", Input::Activate, &mut clipboard),
        Err(Error::Limit)
    );
    f.poll().unwrap();
    assert_eq!(f.addon.pending(), 1);
    f.addon
        .poll(&mut f.rt, &mut f.doc, Duration::from_secs(10))
        .unwrap();
    assert_eq!(f.addon.pending(), 0);
    f.accept(GuestMessage::Validation {
        event: 1,
        key: "note".into(),
        value_revision: 3,
        error: Some("late".into()),
    })
    .unwrap();
    let control = f.addon.ui().control("note").unwrap();
    assert!(matches!(&*control.borrow(),Control::Text(t) if !t.invalid));
    let oversized = UiNode::label("large", &"x".repeat(4097));
    assert_eq!(
        f.accept(GuestMessage::Patch {
            base: 1,
            changes: vec![Patch::Put { node: oversized }]
        }),
        Err(Error::Limit)
    );
    assert!(f.addon.ui().node("large").is_none());
}

#[test]
fn required_component_incompatibility_custom_schema_and_active_owner_cleanup() {
    let mut m = manifest("--guest");
    m.components.push(ComponentRequirement {
        name: "badge".into(),
        version: 1,
        required: true,
        fallback: None,
    });
    let mut rt: Runtime<(), ()> = Runtime::new(4).unwrap();
    let (_, root) = rt
        .open_window(Description::new("app", (), |_, _, _| Dirty::NONE))
        .unwrap();
    let owner = rt
        .own(root, ResourceKind::Task, || {})
        .unwrap()
        .cancellation();
    let parents = m.slots.iter().cloned().map(|s| (s, root)).collect();
    let grants = Grants {
        scopes: m.scopes.clone(),
        ..Default::default()
    };
    assert!(matches!(
        Addon::install(
            m.clone(),
            grants,
            Registry::standard(),
            parents,
            owner,
            Limits::default()
        ),
        Err(Error::Incompatible(_))
    ));
    let mut registry = Registry::standard();
    registry
        .register(
            "badge",
            ComponentSchema {
                version: 1,
                kind: Kind::Label,
                decode: |v| {
                    #[derive(serde::Deserialize)]
                    #[serde(deny_unknown_fields)]
                    struct Badge {
                        caption: String,
                    }
                    let b: Badge = serde_json::from_value(v.clone())
                        .map_err(|e| Error::Invalid(e.to_string()))?;
                    Ok(Props {
                        label: b.caption,
                        value: None,
                        command: None,
                        value_revision: None,
                    })
                },
            },
        )
        .unwrap();
    let grants = Grants {
        scopes: m.scopes.clone(),
        ..Default::default()
    };
    let mut f = Fixture::with_manifest(m, Limits::default(), grants, registry);
    f.ready();
    let node = UiNode {
        key: "badge".into(),
        slot: Slot::SidePanel,
        component: "badge".into(),
        props: serde_json::json!({"caption":"Registered by host"}),
    };
    f.accept(GuestMessage::Patch {
        base: 1,
        changes: vec![Patch::Put { node }],
    })
    .unwrap();
    assert!(f.addon.ui().node("badge").is_some());
    f.rt.close_window(f.window).unwrap();
    f.poll().unwrap();
    assert!(matches!(
        f.addon.state(),
        State::Deactivating | State::Installed
    ));
    assert_eq!(f.addon.pending(), 0);
    assert!(f.addon.ui().node("badge").is_none());
}

#[test]
fn button_key_repeat_and_multiple_activations_preserve_action_once() {
    let mut f = Fixture::new("--unack", Limits::default());
    f.ready();
    let button = f.addon.ui().node("accept").unwrap();
    f.rt.set_focus(f.window, Some(button)).unwrap();
    f.rt.flush(&mut ()).unwrap();
    let mut clipboard = MemoryClipboard::default();
    for phase in [KeyPhase::Down, KeyPhase::Repeat, KeyPhase::Up] {
        f.addon
            .input(
                &mut f.rt,
                "accept",
                Input::Key(KeyboardEvent {
                    key: Key::Named(NamedKey::Enter),
                    phase,
                    modifiers: Modifiers::default(),
                    timestamp: 1,
                }),
                &mut clipboard,
            )
            .unwrap();
    }
    assert_eq!(f.addon.pending(), 1);
    f.addon
        .input(&mut f.rt, "accept", Input::Activate, &mut clipboard)
        .unwrap();
    assert_eq!(f.addon.pending(), 2);
}

#[test]
fn typing_undo_group_and_blur_composition_use_existing_editor_lifecycle() {
    let mut f = Fixture::new("--unack", Limits::default());
    f.ready();
    let note = f.addon.ui().node("note").unwrap();
    f.rt.set_focus(f.window, Some(note)).unwrap();
    f.rt.flush(&mut ()).unwrap();
    let mut clipboard = MemoryClipboard::default();
    f.addon
        .input(
            &mut f.rt,
            "note",
            Input::Text("a".into(), 1),
            &mut clipboard,
        )
        .unwrap();
    f.addon
        .input(
            &mut f.rt,
            "note",
            Input::Text("b".into(), 2),
            &mut clipboard,
        )
        .unwrap();
    f.addon
        .input(
            &mut f.rt,
            "note",
            Input::Key(KeyboardEvent {
                key: Key::Character("z".into()),
                phase: KeyPhase::Down,
                modifiers: Modifiers {
                    ctrl: true,
                    ..Default::default()
                },
                timestamp: 3,
            }),
            &mut clipboard,
        )
        .unwrap();
    assert_eq!(f.text(), "");
    f.addon
        .input(
            &mut f.rt,
            "note",
            Input::Preedit("仮".into(), Some((0, 3))),
            &mut clipboard,
        )
        .unwrap();
    f.rt.set_focus(f.window, f.addon.ui().node("accept"))
        .unwrap();
    f.poll().unwrap();
    let control = f.addon.ui().control("note").unwrap();
    assert!(
        matches!(&*control.borrow(),Control::Text(t) if t.snapshot().composition.is_none() && !t.focused())
    );
}

#[test]
fn contributed_field_runs_the_shared_single_line_conformance_profile() {
    use view_controls::view_text::{CaseResult, Profile, run_profile};
    let mut f = Fixture::new("--unack", Limits::default());
    f.ready();
    let control = f.addon.ui().control("note").unwrap();
    let mut control = control.borrow_mut();
    let Control::Text(field) = &mut *control else {
        panic!("host text engine");
    };
    let report = run_profile(
        field.as_mut(),
        Profile::SingleLineV1,
        "addon/host-text",
        std::env::consts::OS,
    );
    assert!(
        !report
            .cases
            .iter()
            .any(|(_, case)| matches!(case, CaseResult::Failed(_)))
    );
    assert!(
        !report.certified(),
        "headless profile must preserve native coverage gaps"
    );
}

#[cfg(feature = "render")]
#[test]
fn painter_preserves_clip_and_blink_and_rejects_unsupported_transforms() {
    let mut f = Fixture::new("--unack", Limits::default());
    f.ready();
    let note = f.addon.ui().node("note").unwrap();
    f.rt.set_focus(f.window, Some(note)).unwrap();
    f.rt.flush(&mut ()).unwrap();
    f.addon
        .input(
            &mut f.rt,
            "note",
            Input::Text("a".into(), 1),
            &mut MemoryClipboard::default(),
        )
        .unwrap();
    f.rt.flush(&mut ()).unwrap();
    let mut fonts = view_controls::view_text::CosmicBackend::from_fonts(
        [include_bytes!("../../../tests/fixtures/fonts/NotoSans-Regular.ttf").to_vec()],
        8,
    )
    .unwrap();
    f.rt.set_clip(f.root, Some(LogicalRect::from_xywh(0., 0., 12., 12.)))
        .unwrap();
    f.rt.flush(&mut ()).unwrap();
    let items = f
        .addon
        .ui_mut()
        .paint(
            &f.rt,
            &mut fonts,
            &view_controls::Style::default(),
            ScaleFactor::new(1.).unwrap(),
            750,
        )
        .unwrap();
    assert!(!items.is_empty());
    for item in items {
        if let view_wgpu::Paint::Draw(draw) = item {
            assert!(draw.clip.unwrap().size.width <= 12.);
        }
    }
    let control = f.addon.ui().control("note").unwrap();
    assert!(matches!(&*control.borrow(),Control::Text(t) if !t.editor().caret_visible(750)));
    f.rt.set_transform(f.root, Transform2D::new([2., 0., 0., 2., 0., 0.]).unwrap())
        .unwrap();
    f.rt.flush(&mut ()).unwrap();
    assert!(matches!(
        f.addon.ui_mut().paint(
            &f.rt,
            &mut fonts,
            &view_controls::Style::default(),
            ScaleFactor::new(1.).unwrap(),
            750
        ),
        Err(Error::Incompatible(_))
    ));
}
