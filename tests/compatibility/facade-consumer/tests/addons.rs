#![cfg(feature = "addons")]
use view::addons::*;
#[test]
fn public_addon_wire_contract_has_no_native_ui_runtime_requirement() {
    let node = UiNode::text("review-note", "Note", "owned by host");
    let message = GuestEnvelope {
        generation: 7,
        message: GuestMessage::Snapshot {
            base: 3,
            nodes: vec![node],
        },
    };
    let mut bytes = vec![];
    write_frame(&mut bytes, &message).unwrap();
    let decoded: GuestEnvelope = read_frame(&mut std::io::Cursor::new(bytes)).unwrap();
    assert_eq!(decoded.generation, 7);
    match decoded.message {
        GuestMessage::Snapshot { base, nodes } => {
            assert_eq!(base, 3);
            assert_eq!(nodes[0].key, "review-note");
        }
        _ => panic!("snapshot"),
    }
}
