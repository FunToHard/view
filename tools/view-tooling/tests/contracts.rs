use serde_json::{Value, json};
use view_automation::*;
fn request(id: u64, operation: Operation) -> Request {
    Request {
        version: VERSION,
        session: "test".into(),
        id,
        operation,
    }
}
fn target(s: &Session<view_tooling::Document, view_tooling::Action>) -> Target {
    s.resolve(&Selector {
        test_id: Some("document".into()),
        ..Default::default()
    })
    .unwrap()
}
#[test]
fn schemas_scope_stale_targets_and_deduplication_use_one_reducer() {
    let mut s = view_tooling::fixture("test".into()).unwrap();
    let old = target(&s);
    let mutation = request(
        1,
        Operation::Command {
            name: "document.set".into(),
            arguments: json!({"value":7}),
            target: old.clone(),
        },
    );
    let first = s.execute(mutation.clone());
    assert!(first.result.is_ok());
    assert_eq!(s.execute(mutation.clone()), first);
    assert_eq!(s.harness.model().undo.len(), 1);
    let mut reused = mutation;
    reused.operation = Operation::Snapshot;
    assert_eq!(s.execute(reused).result.unwrap_err().code, Code::Invalid);
    assert_eq!(
        s.execute(request(
            2,
            Operation::Command {
                name: "document.undo".into(),
                arguments: json!({}),
                target: old
            }
        ))
        .result
        .unwrap_err()
        .code,
        Code::Stale
    );
    let t = target(&s);
    assert!(
        s.execute(request(
            3,
            Operation::Command {
                name: "document.undo".into(),
                arguments: json!({}),
                target: t
            }
        ))
        .result
        .is_ok()
    );
    assert_eq!(s.harness.model().value, 0);
    let t = target(&s);
    assert_eq!(
        s.execute(request(
            4,
            Operation::Command {
                name: "document.set".into(),
                arguments: json!({"value":"bad"}),
                target: t
            }
        ))
        .result
        .unwrap_err()
        .code,
        Code::Invalid
    );
    assert_eq!(s.harness.model().value, 0);
    s.disconnect();
    assert_eq!(
        s.execute(request(5, Operation::Snapshot))
            .result
            .unwrap_err()
            .code,
        Code::UnknownOutcome
    );
}
#[test]
fn cli_and_rust_have_identical_discovery_errors_and_command_results() {
    let mut rust = view_tooling::fixture("test".into()).unwrap();
    let mut cli = view_tooling::fixture("test".into()).unwrap();
    // Runtime arena identities are intentionally distinct. Resolve in each attachment.
    let queries = vec![
        request(1, Operation::Discover),
        request(
            2,
            Operation::Resolve {
                selector: Selector {
                    name: Some("missing".into()),
                    ..Default::default()
                },
            },
        ),
    ];
    let input = queries
        .iter()
        .map(|r| serde_json::to_string(r).unwrap() + "\n")
        .collect::<String>();
    let mut output = vec![];
    view_tooling::serve(&mut cli, std::io::Cursor::new(input), &mut output).unwrap();
    let replies: Vec<Response> = String::from_utf8(output)
        .unwrap()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    for (q, r) in queries.into_iter().zip(replies) {
        assert_eq!(rust.execute(q), r);
    }
    let mut cli = view_tooling::fixture("test".into()).unwrap();
    let t = target(&cli);
    let q = request(
        3,
        Operation::Command {
            name: "document.set".into(),
            arguments: json!({"value":31}),
            target: t,
        },
    );
    let mut output = vec![];
    view_tooling::serve(
        &mut cli,
        std::io::Cursor::new(serde_json::to_string(&q).unwrap() + "\n"),
        &mut output,
    )
    .unwrap();
    assert_eq!(cli.harness.model().value, 31);
    let response: Response = serde_json::from_slice(&output).unwrap();
    assert_eq!(response.result.unwrap(), json!({"value":31}));
}
#[test]
fn operations_waits_captures_and_replay_report_actual_boundaries() {
    let mut s = view_tooling::fixture("test".into()).unwrap();
    let op = s.start_operation().unwrap();
    s.progress(op, 50).unwrap();
    assert_eq!(
        s.execute(request(
            1,
            Operation::Wait {
                predicate: Predicate::Idle,
                turns: 2
            }
        ))
        .result
        .unwrap_err()
        .code,
        Code::Timeout
    );
    assert!(
        s.execute(request(2, Operation::Cancel { operation: op }))
            .result
            .is_ok()
    );
    assert_eq!(
        s.finish_operation(op, json!({})).unwrap_err().code,
        Code::Cancelled
    );
    let root = &s.harness.runtime().snapshot().nodes[0];
    let window = root.window;
    assert_eq!(
        s.execute(request(
            3,
            Operation::Capture {
                window: window_id(window),
                revision: s.revision()
            }
        ))
        .result
        .unwrap_err()
        .code,
        Code::Unsupported
    );
    s.publish_presentation(window, s.revision(), "offscreen_readback")
        .unwrap();
    assert_eq!(
        s.execute(request(
            4,
            Operation::Wait {
                predicate: Predicate::Presentation {
                    window: window_id(window),
                    revision: s.revision(),
                    boundary: "compositor_observed".into()
                },
                turns: 1
            }
        ))
        .result
        .unwrap_err()
        .code,
        Code::Timeout
    );
    s.record_boundary("network", json!({"fixture":"offline-v1"}))
        .unwrap();
    let records = vec![
        json!({"boundary":"network","fixture":"offline-v1"}),
        json!({"request":request(10,Operation::Advance{millis:5})}),
    ];
    assert!(
        replay(&mut s, &records, |_, _, _| Err(Error::new(
            Code::NotFound,
            "fixture unavailable"
        )))
        .is_err()
    );
    let replayed = replay(&mut s, &records, |kind, value, _| {
        assert_eq!(kind, "network");
        assert_eq!(value, "offline-v1");
        Ok(())
    })
    .unwrap();
    assert!(replayed[0].result.is_ok());
    let mut output = vec![];
    assert!(
        view_tooling::serve(
            &mut s,
            std::io::Cursor::new(vec![b' '; MAX_MESSAGE + 1]),
            &mut output
        )
        .is_err()
    );
}
#[test]
fn passive_binary_does_not_open_a_transport() {
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_view-tooling"))
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
}
#[test]
fn malformed_json_is_a_protocol_error_not_diagnostic_stdout() {
    let mut s = view_tooling::fixture("test".into()).unwrap();
    let mut output = vec![];
    view_tooling::serve(&mut s, std::io::Cursor::new(b"{oops}\n"), &mut output).unwrap();
    let value: Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(value["result"]["Err"]["code"], "invalid");
}

#[test]
fn background_tasks_are_bounded_pollable_and_same_session_retries_do_not_restart() {
    let mut s = view_tooling::fixture("test".into()).unwrap();
    let t = target(&s);
    let q = request(
        1,
        Operation::StartTask {
            name: "document.preview".into(),
            arguments: json!({}),
            target: t,
        },
    );
    let first = s.execute(q.clone());
    let id = first.result.as_ref().unwrap()["operation"]
        .as_u64()
        .unwrap();
    assert_eq!(s.execute(q), first);
    for n in 2..100 {
        let value = s
            .execute(request(n, Operation::Poll { operation: id }))
            .result
            .unwrap();
        if value["status"] == "complete" {
            assert_eq!(value["result"]["value"], 0);
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
    panic!("worker failed to complete within bounded polling deadline");
}

#[test]
fn oversized_worker_output_fails_without_retaining_the_payload() {
    let mut s = view_tooling::fixture("test".into()).unwrap();
    s.register_task::<Value>(
        CommandDescriptor {
            name: "document.large".into(),
            description: "size boundary fixture".into(),
            scope: Scope::Inspect,
            arguments: Schema::default(),
            result: Schema {
                fields: [(
                    "text".into(),
                    Field::String {
                        max_bytes: MAX_MESSAGE * 2,
                    },
                )]
                .into(),
            },
        },
        |_, _| {
            Ok(Box::new(|_| {
                Ok(json!({"text":"x".repeat(MAX_MESSAGE + 1)}))
            }))
        },
    )
    .unwrap();
    let t = target(&s);
    let op = s
        .execute(request(
            1,
            Operation::StartTask {
                name: "document.large".into(),
                arguments: json!({}),
                target: t,
            },
        ))
        .result
        .unwrap()["operation"]
        .as_u64()
        .unwrap();
    for n in 2..100 {
        let value = s
            .execute(request(n, Operation::Poll { operation: op }))
            .result
            .unwrap();
        if value["status"] == "failed" {
            assert_eq!(value["result"]["error"]["code"], "limit");
            assert!(serde_json::to_vec(&value).unwrap().len() < 1024);
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
    panic!("oversized worker did not terminate within deadline");
}

#[test]
fn capture_capacity_allows_replacing_an_existing_window_receipt() {
    let mut s = view_tooling::fixture("test".into()).unwrap();
    let mut windows = vec![s.harness.runtime().snapshot().nodes[0].window];
    for i in 1..8 {
        windows.push(
            s.harness
                .runtime_mut()
                .open_window(view_core::Description::new(
                    format!("window-{i}"),
                    (),
                    |_, _, _| view_core::Dirty::NONE,
                ))
                .unwrap()
                .0,
        );
    }
    s.harness.step().unwrap();
    for w in &windows {
        s.publish_presentation(*w, s.revision(), "offscreen_readback")
            .unwrap();
        s.publish_capture(Capture {
            window: window_id(*w),
            revision: s.revision(),
            width: 1,
            height: 1,
            rgba: vec![0, 0, 0, 255],
            provenance: "offscreen_readback".into(),
            overhead_micros: 1,
        })
        .unwrap();
    }
    s.publish_capture(Capture {
        window: window_id(windows[0]),
        revision: s.revision(),
        width: 1,
        height: 1,
        rgba: vec![255, 0, 0, 255],
        provenance: "offscreen_readback".into(),
        overhead_micros: 2,
    })
    .unwrap();
    let result = s
        .execute(request(
            1,
            Operation::Capture {
                window: window_id(windows[0]),
                revision: s.revision(),
            },
        ))
        .result
        .unwrap();
    assert_eq!(result["rgba"], json!([255, 0, 0, 255]));
}
