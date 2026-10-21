#![cfg(feature = "mcp")]
use serde_json::{Value, json};
use std::{
    io::{BufRead, BufReader, Write},
    process::{Child, ChildStdin, Command, Stdio},
    sync::mpsc,
    time::Duration,
};
struct Client {
    child: Child,
    input: ChildStdin,
    output: mpsc::Receiver<Value>,
}
impl Drop for Client {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
impl Client {
    fn start() -> Self {
        let mut child = Command::new(env!("CARGO_BIN_EXE_view-tooling"))
            .args(["--enable-session", "mcp-test", "--mcp"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .unwrap();
        let input = child.stdin.take().unwrap();
        let stdout = child.stdout.take().unwrap();
        let (tx, output) = mpsc::sync_channel(32);
        std::thread::spawn(move || {
            for line in BufReader::new(stdout).lines() {
                let Ok(line) = line else { break };
                let Ok(value) = serde_json::from_str(&line) else {
                    break;
                };
                if tx.send(value).is_err() {
                    break;
                }
            }
        });
        Self {
            child,
            input,
            output,
        }
    }
    fn send(&mut self, value: Value) {
        writeln!(self.input, "{value}").unwrap();
        self.input.flush().unwrap();
    }
    fn receive(&self, id: u64) -> Value {
        loop {
            let value = self
                .output
                .recv_timeout(Duration::from_secs(15))
                .expect("bounded MCP reply");
            if value["id"] == id {
                return value;
            }
        }
    }
    fn call(&mut self, id: u64, operation: Value) -> Value {
        self.send(json!({"jsonrpc":"2.0","id":id,"method":"tools/call","params":{"name":"view.request","arguments":{"version":1,"session":"mcp-test","id":id,"operation":operation}}}));
        let reply = self.receive(id);
        let text = reply["result"]["content"][0]["text"]
            .as_str()
            .expect("MCP tool text response");
        serde_json::from_str(text).unwrap()
    }
}
#[test]
fn official_stdio_sdk_negotiates_and_preserves_discovery_mutation_undo_and_errors() {
    let mut c = Client::start();
    c.send(json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"view-parity-test","version":"1"}}}));
    let init = c.receive(1);
    assert!(init.get("result").is_some(), "{init}");
    c.send(json!({"jsonrpc":"2.0","method":"notifications/initialized"}));
    c.send(json!({"jsonrpc":"2.0","id":2,"method":"tools/list"}));
    assert_eq!(c.receive(2)["result"]["tools"][0]["name"], "view.request");
    let discovery = c.call(3, json!({"op":"discover"}));
    let mut rust = view_tooling::fixture("mcp-test".into()).unwrap();
    let expected = rust.execute(view_automation::Request {
        version: 1,
        session: "mcp-test".into(),
        id: 3,
        operation: view_automation::Operation::Discover,
    });
    assert_eq!(discovery, serde_json::to_value(expected).unwrap());
    let resolved = c.call(4, json!({"op":"resolve","selector":{"test_id":"document"}}));
    let target = resolved["result"]["Ok"].clone();
    let mutation =
        json!({"op":"command","name":"document.set","arguments":{"value":42},"target":target});
    let first = c.call(5, mutation.clone());
    assert_eq!(first["result"]["Ok"]["value"], 42);
    assert_eq!(c.call(5, mutation), first);
    let stale = c.call(
        6,
        json!({"op":"command","name":"document.undo","arguments":{},"target":target}),
    );
    assert_eq!(stale["result"]["Err"]["code"], "stale");
    let resolved = c.call(7, json!({"op":"resolve","selector":{"test_id":"document"}}));
    let undo=c.call(8,json!({"op":"command","name":"document.undo","arguments":{},"target":resolved["result"]["Ok"]}));
    assert_eq!(undo["result"]["Ok"]["value"], 0);
    let missing = c.call(9, json!({"op":"capture","window":"unknown","revision":0}));
    assert_eq!(missing["result"]["Err"]["code"], "unsupported");
    let target = c.call(
        10,
        json!({"op":"resolve","selector":{"test_id":"document"}}),
    )["result"]["Ok"]
        .clone();
    let start = c.call(
        11,
        json!({"op":"start_task","name":"document.preview","arguments":{},"target":target}),
    );
    let operation = start["result"]["Ok"]["operation"].clone();
    for id in 12..100 {
        let poll = c.call(id, json!({"op":"poll","operation":operation}));
        if poll["result"]["Ok"]["status"] == "complete" {
            assert_eq!(poll["result"]["Ok"]["result"]["value"], 0);
            return;
        }
        std::thread::sleep(Duration::from_millis(2));
    }
    panic!("MCP worker did not complete within bounded polling deadline");
}
