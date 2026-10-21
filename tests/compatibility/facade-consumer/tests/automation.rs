#![cfg(feature = "automation")]
use view::{Description, Dirty, Runtime};
use view_automation::{Operation, Request, Scope, Session, VERSION};
use view_testing::Harness;
#[test]
fn automation_is_optional_and_consumes_public_runtime_without_native_or_mcp() {
    let mut runtime = Runtime::<(), ()>::new(8).unwrap();
    runtime
        .open_window(Description::new("root", (), |_, _, _| Dirty::NONE))
        .unwrap();
    runtime.flush(&mut ()).unwrap();
    let mut session = Session::new(
        "consumer".into(),
        Harness::new(runtime, (), 8),
        vec![Scope::Inspect],
        8,
    )
    .unwrap();
    let response = session.execute(Request {
        version: VERSION,
        session: "consumer".into(),
        id: 1,
        operation: Operation::Discover,
    });
    assert_eq!(
        response.result.unwrap()["capabilities"]["physical_input"],
        false
    );
}
