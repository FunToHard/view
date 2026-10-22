//! An actual native guest plus a headless host proof. No document model lives in the library.
use std::{
    collections::BTreeMap,
    io::{self, Write},
    time::{Duration, Instant},
};
use view_addons::*;
use view_controls::view_text::MemoryClipboard;
use view_core::*;

fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
    let mode = std::env::args().nth(1).unwrap_or_default();
    if mode == "--proof" || mode == "--gpu" {
        proof(mode == "--gpu")?;
    } else {
        guest(&mode)?;
    }
    Ok(())
}
fn guest(mode: &str) -> view_addons::Result<()> {
    let mut input = io::stdin().lock();
    let mut output = io::stdout().lock();
    if mode == "--silent" {
        std::thread::sleep(Duration::from_secs(60));
        return Ok(());
    }
    if mode == "--oversized" {
        output.write_all(&((MAX_FRAME + 1) as u32).to_le_bytes())?;
        output.flush()?;
        return Ok(());
    }
    if mode == "--malformed" {
        output.write_all(&2u32.to_le_bytes())?;
        output.write_all(b"{}")?;
        output.flush()?;
        return Ok(());
    }
    write_frame(
        &mut output,
        &GuestEnvelope {
            generation: 0,
            message: GuestMessage::Hello {
                protocol: if mode == "--wrong-version" {
                    999
                } else {
                    VERSION
                },
                id: "review".into(),
                version: "1".into(),
            },
        },
    )?;
    let HostMessage::Welcome { generation, .. } = read_frame(&mut input)? else {
        return Err(Error::Incompatible("welcome".into()));
    };
    if mode == "--crash" {
        std::process::exit(31);
    }
    if mode == "--disconnect" {
        return Ok(());
    }
    let send = |out: &mut std::io::StdoutLock<'_>, message| {
        write_frame(
            out,
            &GuestEnvelope {
                generation,
                message,
            },
        )
    };
    if mode == "--late-generation" {
        write_frame(
            &mut output,
            &GuestEnvelope {
                generation: generation + 1,
                message: GuestMessage::Snapshot {
                    base: 0,
                    nodes: vec![],
                },
            },
        )?;
    }
    let nodes = vec![
        UiNode::label("suggestion", "Replace ‘teh’ with ‘the’"),
        UiNode::text("note", "Review note", ""),
        UiNode::button("accept", "Accept suggestion", "accept"),
        UiNode::button("toolbar", "Review", "accept").in_slot(Slot::Toolbar),
        UiNode::button("menu", "Review", "accept").in_slot(Slot::Menu),
    ];
    send(&mut output, GuestMessage::Snapshot { base: 0, nodes })?;
    send(&mut output, GuestMessage::ReadDocument { request: 1 })?;
    if mode == "--flood" {
        for _ in 0..1000 {
            send(&mut output, GuestMessage::Complete { event: 999 })?;
        }
    }
    let mut document_revision = 0;
    let mut request = 1;
    eprintln!("review add-on activated; trusted native executable");
    loop {
        match read_frame::<HostMessage>(&mut input)? {
            HostMessage::Reply {
                result: Ok(snapshot),
                ..
            } => document_revision = snapshot.revision,
            HostMessage::Event {
                event,
                key,
                command,
                value_revision,
                ..
            } => {
                if command.as_deref() == Some("accept") {
                    request += 1;
                    send(
                        &mut output,
                        GuestMessage::EditDocument {
                            request,
                            action: "replace".into(),
                            revision: document_revision,
                            arguments: serde_json::json!({"start":0,"end":3,"expected":"teh","replacement":"the"}),
                        },
                    )?;
                } else if key == "note" {
                    send(
                        &mut output,
                        GuestMessage::Validation {
                            event,
                            key,
                            value_revision,
                            error: None,
                        },
                    )?;
                }
                if mode != "--unack" {
                    send(&mut output, GuestMessage::Complete { event })?;
                }
            }
            HostMessage::Cancel { event } => eprintln!("cancelled event {event}"),
            _ => (),
        }
    }
}
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Replace {
    start: usize,
    end: usize,
    expected: String,
    replacement: String,
}
fn validate(value: &serde_json::Value) -> view_addons::Result<()> {
    let edit: Replace =
        serde_json::from_value(value.clone()).map_err(|e| Error::Invalid(e.to_string()))?;
    if edit.replacement.len() > 4096 {
        return Err(Error::Limit);
    }
    Ok(())
}
struct Document {
    text: String,
    revision: u64,
    undo: Vec<String>,
    reads: usize,
}
impl Application for Document {
    fn snapshot(&mut self) -> view_addons::Result<DocumentSnapshot> {
        self.reads += 1;
        Ok(DocumentSnapshot {
            revision: self.revision,
            value: serde_json::json!({"text":self.text}),
        })
    }
    fn edit(
        &mut self,
        action: &str,
        revision: u64,
        arguments: serde_json::Value,
    ) -> view_addons::Result<DocumentSnapshot> {
        if action != "replace" {
            return Err(Error::Denied);
        }
        if revision != self.revision {
            return Err(Error::Stale);
        }
        let e: Replace =
            serde_json::from_value(arguments).map_err(|e| Error::Invalid(e.to_string()))?;
        if self.text.get(e.start..e.end) != Some(e.expected.as_str()) {
            return Err(Error::Stale);
        }
        self.undo.push(self.text.clone());
        self.text.replace_range(e.start..e.end, &e.replacement);
        self.revision += 1;
        self.snapshot()
    }
}
fn proof(gpu: bool) -> view_addons::Result<()> {
    let mut runtime: Runtime<(), ()> = Runtime::new(64)?;
    let (window, root) =
        runtime.open_window(Description::new("word-document", (), |_, _, _| Dirty::NONE))?;
    let owner = runtime.own(root, ResourceKind::Task, || {})?.cancellation();
    let mut parents = BTreeMap::new();
    for (n, slot) in [Slot::SidePanel, Slot::Toolbar, Slot::Menu]
        .into_iter()
        .enumerate()
    {
        parents.insert(
            slot.clone(),
            runtime.mount(
                root,
                Description::new(format!("slot-{slot:?}"), (), |_, _, _| Dirty::NONE)
                    .layout(LogicalRect::from_xywh(n as f32 * 300., 0., 280., 300.)),
            )?,
        );
    }
    let scopes = [
        Scope::Ui,
        Scope::DocumentRead,
        Scope::DocumentEdit,
        Scope::Automation,
    ]
    .into();
    let manifest = Manifest {
        id: "review".into(),
        version: "1".into(),
        protocol: VERSION,
        executable: std::env::current_exe()?,
        arguments: vec!["--guest".into()],
        scopes,
        slots: parents.keys().cloned().collect(),
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
    };
    let grants = Grants {
        scopes: manifest.scopes.clone(),
        actions: [(
            "replace".into(),
            validate as fn(&serde_json::Value) -> view_addons::Result<()>,
        )]
        .into(),
        automation_commands: ["accept".into()].into(),
    };
    let mut addon = Addon::install(
        manifest,
        grants,
        Registry::standard(),
        parents,
        owner,
        Limits::default(),
    )?;
    let mut document = Document {
        text: "teh document".into(),
        revision: 1,
        undo: vec![],
        reads: 0,
    };
    addon.activate()?;
    let start = Instant::now();
    let mut clicked = false;
    while document.undo.is_empty() {
        if start.elapsed() > Duration::from_secs(10) {
            return Err(Error::Timeout);
        }
        addon.poll(&mut runtime, &mut document, start.elapsed())?;
        runtime.flush(&mut ())?;
        if addon.revision() == 1 && document.reads > 0 && !clicked {
            let note = addon.ui().node("note").ok_or(Error::Stale)?;
            runtime.set_focus(window, Some(note))?;
            runtime.flush(&mut ())?;
            let mut clipboard = MemoryClipboard::default();
            addon.input(
                &mut runtime,
                "note",
                Input::Text("Looks good".into(), 1),
                &mut clipboard,
            )?;
            if gpu {
                runtime.flush(&mut ())?;
                #[cfg(feature = "render")]
                capture(&mut addon, &runtime).map_err(|e| Error::Host(e.to_string()))?;
                #[cfg(not(feature = "render"))]
                return Err(Error::Incompatible(
                    "enable render feature for --gpu".into(),
                ));
            }
            addon.input(&mut runtime, "accept", Input::Activate, &mut clipboard)?;
            clicked = true;
        }
        std::thread::sleep(Duration::from_millis(2));
    }
    if document.text != "the document" || document.undo.len() != 1 {
        return Err(Error::Host("transaction proof".into()));
    }
    document.text = document.undo.pop().expect("one undo");
    document.revision += 1;
    if document.text != "teh document" {
        return Err(Error::Host("undo proof".into()));
    }
    addon.deactivate(&mut runtime)?;
    runtime.flush(&mut ())?;
    while !addon.receipt().is_some_and(|r| r.exited && r.workers == 0) {
        if start.elapsed() > Duration::from_secs(10) {
            return Err(Error::Timeout);
        }
        addon.poll(&mut runtime, &mut document, start.elapsed())?;
        std::thread::sleep(Duration::from_millis(2));
    }
    if runtime.focused(window)?.is_some() || !addon.commands().is_empty() || addon.pending() != 0 {
        return Err(Error::Host("cleanup proof".into()));
    }
    println!(
        "PASS actual child negotiation; host review controls; revision-checked edit; one undo; focus/command/request cleanup; {:?}",
        addon.receipt()
    );
    Ok(())
}

#[cfg(feature = "render")]
fn capture(
    addon: &mut Addon,
    runtime: &Runtime<(), ()>,
) -> std::result::Result<(), Box<dyn std::error::Error>> {
    use std::{
        future::Future,
        sync::Arc,
        task::{Context, Poll, Wake, Waker},
    };
    use view_wgpu::*;
    struct ThreadWake(std::thread::Thread);
    impl Wake for ThreadWake {
        fn wake(self: Arc<Self>) {
            self.0.unpark();
        }
    }
    fn wait<T>(future: impl Future<Output = T>) -> T {
        let deadline = Instant::now() + Duration::from_secs(30);
        let waker = Waker::from(Arc::new(ThreadWake(std::thread::current())));
        let mut context = Context::from_waker(&waker);
        let mut future = std::pin::pin!(future);
        loop {
            if let Poll::Ready(value) = future.as_mut().poll(&mut context) {
                return value;
            }
            std::thread::park_timeout(
                deadline
                    .checked_duration_since(Instant::now())
                    .expect("GPU deadline"),
            );
        }
    }
    let backend = match std::env::args().nth(2).as_deref() {
        Some("dx12") => wgpu::Backends::DX12,
        Some("vulkan") => wgpu::Backends::VULKAN,
        Some("metal") => wgpu::Backends::METAL,
        _ => return Err("specify dx12, vulkan or metal".into()),
    };
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
        backends: backend,
        ..wgpu::InstanceDescriptor::new_without_display_handle()
    });
    let adapter = wait(instance.request_adapter(&Default::default()))?;
    println!("M7 GPU adapter={:?}", adapter.get_info());
    let context = wait(GpuContext::request(
        &adapter,
        wgpu::Features::empty(),
        wgpu::Limits::default(),
    ))?;
    let mut renderer = Renderer::new(
        context.clone(),
        wgpu::TextureFormat::Rgba8UnormSrgb,
        Budgets::default(),
    )?;
    let mut fonts = view_controls::view_text::CosmicBackend::from_fonts(
        [include_bytes!("../../../../tests/fixtures/fonts/NotoSans-Regular.ttf").to_vec()],
        32,
    )?;
    let scale = ScaleFactor::new(1.)?;
    let items = addon.ui_mut().paint(
        runtime,
        &mut fonts,
        &view_controls::Style::default(),
        scale,
        1,
    )?;
    let snapshot = PaintSnapshot {
        revision: runtime.snapshot().revision,
        size: [900, 300],
        scale,
        items,
    };
    let target = context.target(snapshot.size)?;
    let pixels = renderer
        .prepare(&snapshot)?
        .submit_capture(&context, &target, Color([0.06, 0.075, 0.1, 1.]))?
        .readback(&context)?;
    assert_eq!(pixels.rgba.len(), 900 * 300 * 4);
    assert!(pixels.rgba.as_chunks::<4>().0.iter().any(|p| p[0] > 100));
    assert!(context.take_errors().is_empty());
    let mut bmp = vec![];
    let bytes = 900u32 * 300 * 4;
    bmp.extend(b"BM");
    bmp.extend((54 + bytes).to_le_bytes());
    bmp.extend([0; 4]);
    bmp.extend(54u32.to_le_bytes());
    bmp.extend(40u32.to_le_bytes());
    bmp.extend(900i32.to_le_bytes());
    bmp.extend((-300i32).to_le_bytes());
    bmp.extend(1u16.to_le_bytes());
    bmp.extend(32u16.to_le_bytes());
    bmp.extend([0; 4]);
    bmp.extend(bytes.to_le_bytes());
    bmp.extend([0; 16]);
    for p in pixels.rgba.as_chunks::<4>().0 {
        bmp.extend([p[2], p[1], p[0], p[3]]);
    }
    std::fs::create_dir_all("target/verification")?;
    std::fs::write("target/verification/m7-review.bmp", bmp)?;
    println!("PASS host-rendered add-on label/button/text controls: 900x300 RGBA readback");
    Ok(())
}
