//! Local transport adapter. Stdio is private to an explicitly launched child.
#![forbid(unsafe_code)]
use serde_json::{Value, json};
use std::io::{BufRead, Write};
use view_automation::*;
use view_core::{Description, Dirty, LogicalRect, LogicalSize, Role, Runtime};
use view_testing::Harness;
#[cfg(feature = "mcp")]
pub mod mcp;

#[derive(Default)]
pub struct Document {
    pub value: i64,
    pub undo: Vec<i64>,
}
pub enum Action {
    Set(i64),
    Undo,
}
pub fn fixture(name: String) -> Result<Session<Document, Action>, Error> {
    let mut runtime = Runtime::new(64)?;
    let (window, root) = runtime.open_window(
        Description::new("document", (), |_, m: &mut Document, a| {
            match a {
                Action::Set(v) => {
                    m.undo.push(m.value);
                    m.value = v;
                }
                Action::Undo => {
                    if let Some(v) = m.undo.pop() {
                        m.value = v;
                    }
                }
            }
            Dirty::SEMANTICS
        })
        .role(Role::Window)
        .name("Automation fixture")
        .layout(LogicalRect::from_xywh(0., 0., 640., 480.)),
    )?;
    runtime.set_window_size(window, LogicalSize::new(640., 480.))?;
    let mut model = Document::default();
    runtime.flush(&mut model)?;
    let mut session = Session::new(
        name,
        Harness::new(runtime, model, 64),
        vec![Scope::Inspect, Scope::Mutate, Scope::Test],
        1024,
    )?;
    session.test_id(root, "document".into())?;
    let result = Schema {
        fields: [(
            "value".into(),
            Field::Integer {
                min: i64::MIN,
                max: i64::MAX,
            },
        )]
        .into(),
    };
    session.register::<Value>(
        CommandDescriptor {
            name: "document.set".into(),
            description: "One ordinary undoable document action".into(),
            scope: Scope::Mutate,
            arguments: result.clone(),
            result: result.clone(),
        },
        |_, v| {
            let n = v["value"].as_i64().expect("validated");
            Ok((Some(Action::Set(n)), json!({"value":n})))
        },
    )?;
    session.register::<Value>(
        CommandDescriptor {
            name: "document.undo".into(),
            description: "Undo through the same document reducer".into(),
            scope: Scope::Mutate,
            arguments: Schema::default(),
            result: result.clone(),
        },
        |m, _| {
            Ok((
                Some(Action::Undo),
                json!({"value":m.undo.last().copied().unwrap_or(m.value)}),
            ))
        },
    )?;
    session.register::<Value>(
        CommandDescriptor {
            name: "document.read".into(),
            description: "Read document state".into(),
            scope: Scope::Inspect,
            arguments: Schema::default(),
            result,
        },
        |m, _| Ok((None, json!({"value":m.value}))),
    )?;
    session.register_task::<Value>(
        CommandDescriptor {
            name: "document.preview".into(),
            description: "Bounded worker preview; result is queried by operation ID".into(),
            scope: Scope::Inspect,
            arguments: Schema::default(),
            result: Schema {
                fields: [(
                    "value".into(),
                    Field::Integer {
                        min: i64::MIN,
                        max: i64::MAX,
                    },
                )]
                .into(),
            },
        },
        |m, _| {
            let value = m.value;
            Ok(Box::new(move |context| {
                context.progress(50);
                if context.is_cancelled() {
                    return Err(Error::new(Code::Cancelled, "preview cancelled"));
                }
                Ok(json!({"value":value}))
            }))
        },
    )?;
    Ok(session)
}
/// Bounded newline-delimited JSON; protocol only on stdout, diagnostics on stderr.
/// A caller can attach to an app by piping to this function on the owner thread.
pub fn serve<M, A>(
    session: &mut Session<M, A>,
    mut input: impl BufRead,
    mut output: impl Write,
) -> std::io::Result<()> {
    let result = (|| loop {
        let mut bytes = Vec::new();
        let count = std::io::Read::take(&mut input, (MAX_MESSAGE + 1) as u64)
            .read_until(b'\n', &mut bytes)?;
        if count == 0 {
            return Ok(());
        }
        if bytes.len() > MAX_MESSAGE {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "request exceeds byte budget",
            ));
        }
        let response = match serde_json::from_slice::<Request>(&bytes) {
            Ok(request) => session.execute(request),
            Err(e) => Response {
                id: 0,
                revision: session.revision(),
                result: Err(Error::new(Code::Invalid, e.to_string())),
            },
        };
        serde_json::to_writer(&mut output, &response)?;
        output.write_all(b"\n")?;
        output.flush()?;
    })();
    session.disconnect();
    result
}
