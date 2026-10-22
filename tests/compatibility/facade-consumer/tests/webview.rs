#![cfg(feature = "webview")]
use view::webview::*;
#[test]
fn public_webview_contract_is_available_without_native_bindings() {
    let bridge = Bridge::new(SessionId(1));
    assert_eq!(bridge.epoch(), 1);
    assert_eq!(Lifetime::default(), Lifetime::DisposeAfterUse);
    let placement = Placement {
        bounds: view::LogicalRect::from_xywh(2., 4., 20., 10.),
        scale: 1.5,
        transformed: false,
        masked: false,
    };
    assert_eq!(placement.pixels().unwrap(), [3, 6, 33, 21]);
}
