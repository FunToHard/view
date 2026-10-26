use std::{
    fs, io,
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::{SystemTime, UNIX_EPOCH},
};
#[cfg(feature = "failure-fixture")]
use std::{
    process::{Child, Command, ExitStatus, Stdio},
    time::{Duration, Instant},
};
use view_core::{Description, Revision, Runtime, Trace};
use view_diagnostics::*;

fn fixture_directory(name: &str) -> PathBuf {
    let directory = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/diagnostics-contracts")
        .join(format!(
            "{}-{}-{name}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
    fs::create_dir_all(&directory).unwrap();
    directory.canonicalize().unwrap()
}
fn diagnostics(limits: Limits) -> Diagnostics {
    Diagnostics::new(
        Identity {
            application: "fixture".into(),
            build: "build-123".into(),
            symbols: "pdb-guid-age".into(),
            framework: "0.1.0".into(),
            platform: "windows".into(),
            backend: "headless".into(),
        },
        limits,
    )
}
fn prepare(diagnostics: &Diagnostics) -> PreparedReport {
    diagnostics.prepare(|_, text| Some(text.into())).unwrap()
}

#[test]
fn bounds_redaction_revisions_and_runtime_trace_are_observable() {
    let mut diagnostic = diagnostics(Limits {
        events: 2,
        text_bytes: 32,
        report_bytes: 2048,
    });
    diagnostic.capabilities(["native", "private-capability"]);
    diagnostic.revisions(Revisions {
        document: 12,
        committed: 7,
        presented: 5,
        device_generation: 3,
    });
    diagnostic.record(1, "discard-oldest");
    diagnostic.record(2, "private-operation");
    diagnostic.trace(3, Trace::Committed(Revision::from_raw(9)));
    let report = diagnostic
        .prepare(|field, value| match field {
            Field::Symbols => None,
            Field::Build => Some("host-redacted-build".into()),
            Field::Capability | Field::Operation if value.starts_with("private") => None,
            _ => Some(value.into()),
        })
        .unwrap();
    let envelope: Envelope = serde_json::from_slice(report.bytes()).unwrap();
    assert_eq!(envelope.schema, 1);
    assert_eq!(envelope.identity.symbols, "");
    assert_eq!(envelope.identity.build, "host-redacted-build");
    assert_eq!(envelope.capabilities, ["native"]);
    assert_eq!(envelope.dropped, 1);
    assert_eq!(
        envelope.breadcrumbs,
        [Breadcrumb {
            tick: 3,
            operation: "runtime.commit".into()
        }]
    );
    assert_eq!(
        envelope.revisions,
        Revisions {
            document: 12,
            committed: 9,
            presented: 5,
            device_generation: 3
        }
    );
    assert!(!String::from_utf8_lossy(report.bytes()).contains("private"));

    let mut runtime = Runtime::<(), ()>::new(4).unwrap();
    runtime.observe(4);
    runtime
        .open_window(Description::new("root", (), |_, _, _| {
            view_core::Dirty::NONE
        }))
        .unwrap();
    for trace in runtime.drain_trace() {
        diagnostic.trace(4, trace);
    }
    assert!(
        serde_json::from_slice::<Envelope>(prepare(&diagnostic).bytes())
            .unwrap()
            .breadcrumbs
            .iter()
            .any(|b| b.operation == "runtime.mount")
    );
}

#[test]
fn utf8_disabled_breadcrumbs_hard_limits_and_serialization_budget() {
    let mut disabled = diagnostics(Limits {
        events: 0,
        text_bytes: 4,
        report_bytes: 4096,
    });
    disabled.record(0, "🐈🐈");
    let envelope: Envelope = serde_json::from_slice(prepare(&disabled).bytes()).unwrap();
    assert!(envelope.breadcrumbs.is_empty());
    assert_eq!(envelope.dropped, 1);
    assert_eq!(envelope.identity.application, "fixt");
    let mut bounded = diagnostics(Limits {
        events: usize::MAX,
        text_bytes: usize::MAX,
        report_bytes: usize::MAX,
    });
    bounded.capabilities(std::iter::repeat_n("c", 1000));
    for tick in 0..1000 {
        bounded.record(tick, &"🐈".repeat(1000));
    }
    let report = prepare(&bounded);
    let envelope: Envelope = serde_json::from_slice(report.bytes()).unwrap();
    assert_eq!(envelope.capabilities.len(), 64);
    assert_eq!(envelope.breadcrumbs.len(), 256);
    assert_eq!(envelope.breadcrumbs[0].tick, 744);
    assert_eq!(envelope.breadcrumbs[0].operation.len(), 512);
    assert!(report.bytes().len() <= 1_048_576);
    assert!(
        diagnostics(Limits {
            report_bytes: 16,
            ..Limits::default()
        })
        .prepare(|_, text| Some(text.into()))
        .is_err()
    );
}

#[test]
fn retention_is_explicit_preserves_foreign_files_and_rejects_overwrites() {
    let directory = fixture_directory("retention");
    fs::write(directory.join("application-document.json"), "preserve").unwrap();
    fs::write(
        directory.join("view-report-not-a-sequence.json"),
        "preserve",
    )
    .unwrap();
    let store = ReportStore::new(&directory, 2, 4096).unwrap();
    let report = prepare(&diagnostics(Limits::default()));
    let first = store.save(1, &report).unwrap();
    let second = store.save(2, &report).unwrap();
    assert!(store.save(2, &report).is_err());
    assert!(store.save(0, &report).is_err());
    let third = store.save(3, &report).unwrap();
    assert!(!first.exists());
    assert!(second.exists() && third.exists());
    assert_eq!(
        fs::read_to_string(directory.join("application-document.json")).unwrap(),
        "preserve"
    );
    assert!(directory.join("view-report-not-a-sequence.json").exists());
    assert!(
        ReportStore::new(directory.join("tiny"), 1, 1)
            .unwrap()
            .save(0, &report)
            .is_err()
    );
    assert!(ReportStore::new(&directory, 0, 1024).is_err());
}

struct CountingReporter(AtomicUsize);
impl Reporter for CountingReporter {
    fn report(&self, _: &[u8]) -> io::Result<()> {
        self.0.fetch_add(1, Ordering::SeqCst);
        Err(io::Error::other("host policy rejected write"))
    }
}
#[test]
fn panic_reporting_failure_does_not_panic_or_implicitly_install_a_hook() {
    let sink = Arc::new(CountingReporter(AtomicUsize::new(0)));
    let reporter = PanicReporter::new(prepare(&diagnostics(Limits::default())), sink.clone());
    reporter
        .update(prepare(&diagnostics(Limits::default())))
        .unwrap();
    assert!(!reporter.emit());
    assert_eq!(sink.0.load(Ordering::SeqCst), 1);
    // Global ownership/chaining is tested only in the disposable child below.
}

#[test]
fn wer_configuration_is_per_executable_reviewable_and_rejects_injection() {
    let directory = fixture_directory("wer");
    let config = WerLocalDumps::new(
        "diagnostic-fixture.exe",
        directory.clone(),
        3,
        DumpType::Mini,
    )
    .unwrap()
    .registry_file();
    assert!(config.contains("LocalDumps\\diagnostic-fixture.exe]"));
    assert!(config.contains("\"DumpCount\"=dword:00000003"));
    assert!(config.contains("\"DumpType\"=dword:00000001"));
    assert!(config.contains("\"DumpFolder\"=hex(2):"));
    assert!(fs::read_dir(&directory).unwrap().next().is_none());
    assert!(WerLocalDumps::new("bad.exe]", directory.clone(), 1, DumpType::Full).is_err());
    assert!(WerLocalDumps::new("../bad.exe", directory, 1, DumpType::Full).is_err());
}

#[cfg(feature = "failure-fixture")]
struct SupervisedChild(Child);

#[test]
fn symbol_identity_parses_pe_records_without_exposing_private_paths() {
    let directory = fixture_directory("symbols");
    let path = directory.join("symbol-fixture.exe");
    let mut pe = vec![0_u8; 1024];
    pe[..2].copy_from_slice(b"MZ");
    pe[60..64].copy_from_slice(&64_u32.to_le_bytes());
    pe[64..68].copy_from_slice(b"PE\0\0");
    pe[70..72].copy_from_slice(&1_u16.to_le_bytes());
    pe[84..86].copy_from_slice(&240_u16.to_le_bytes());
    pe[88..90].copy_from_slice(&0x20b_u16.to_le_bytes());
    pe[196..200].copy_from_slice(&7_u32.to_le_bytes());
    pe[248..252].copy_from_slice(&0x1000_u32.to_le_bytes());
    pe[252..256].copy_from_slice(&28_u32.to_le_bytes());
    pe[340..344].copy_from_slice(&0x1000_u32.to_le_bytes());
    pe[344..348].copy_from_slice(&512_u32.to_le_bytes());
    pe[348..352].copy_from_slice(&512_u32.to_le_bytes());
    pe[524..528].copy_from_slice(&2_u32.to_le_bytes());
    pe[528..532].copy_from_slice(&64_u32.to_le_bytes());
    pe[536..540].copy_from_slice(&600_u32.to_le_bytes());
    pe[600..604].copy_from_slice(b"RSDS");
    pe[604..620].copy_from_slice(&(0_u8..16).collect::<Vec<_>>());
    pe[620..624].copy_from_slice(&2_u32.to_le_bytes());
    pe[624..642].copy_from_slice(b"private-source.pdb");
    fs::write(&path, &pe).unwrap();
    let identity = read_pdb_identity(&path).unwrap().unwrap();
    assert_eq!(identity.symbol_key(), "030201000504070608090A0B0C0D0E0F2");
    // A malicious section count fails before allocating its requested table.
    pe[70..72].copy_from_slice(&u16::MAX.to_le_bytes());
    fs::write(&path, &pe).unwrap();
    assert!(read_pdb_identity(&path).is_err());
    fs::write(&path, &pe[..64]).unwrap();
    assert!(read_pdb_identity(&path).is_err());
    #[cfg(windows)]
    assert!(
        read_pdb_identity(&std::env::current_exe().unwrap())
            .unwrap()
            .is_some()
    );
}

#[cfg(feature = "failure-fixture")]
impl Drop for SupervisedChild {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
#[cfg(feature = "failure-fixture")]
fn launch(mode: &str, directory: &std::path::Path, label: &str) -> SupervisedChild {
    SupervisedChild(
        Command::new(env!("CARGO_BIN_EXE_diagnostic-fixture"))
            .args(["--disposable-fixture", mode])
            .arg(directory)
            .stdin(Stdio::null())
            .stdout(fs::File::create(directory.join(format!("{label}-stdout.json"))).unwrap())
            .stderr(fs::File::create(directory.join(format!("{label}-stderr.log"))).unwrap())
            .spawn()
            .unwrap(),
    )
}
#[cfg(feature = "failure-fixture")]
fn wait(child: &mut SupervisedChild, timeout: Duration) -> (ExitStatus, bool) {
    let deadline = Instant::now() + timeout;
    loop {
        if let Some(status) = child.0.try_wait().unwrap() {
            return (status, false);
        }
        if Instant::now() >= deadline {
            child.0.kill().unwrap();
            return (child.0.wait().unwrap(), true);
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[test]
#[cfg(feature = "failure-fixture")]
fn independent_supervisor_retains_failure_policy_and_recovers_on_fresh_launch() {
    let modes = if cfg!(windows) {
        vec!["panic", "abort", "native-fault", "hang"]
    } else {
        vec!["panic", "abort", "hang"]
    };
    let mut manifest = vec![];
    for mode in modes {
        let directory = fixture_directory(mode);
        let mut child = launch(mode, &directory, "failure");
        let readiness_deadline = Instant::now() + Duration::from_secs(15);
        while !directory.join("ready").exists() && Instant::now() < readiness_deadline {
            assert!(
                child.0.try_wait().unwrap().is_none(),
                "{mode}: child failed before ready; logs in {}",
                directory.display()
            );
            std::thread::sleep(Duration::from_millis(10));
        }
        assert!(
            directory.join("ready").exists(),
            "child initialization timed out"
        );
        let (status, timed_out) = wait(
            &mut child,
            if mode == "hang" {
                Duration::from_millis(300)
            } else {
                Duration::from_secs(20)
            },
        );
        assert!(!status.success(), "{mode} unexpectedly survived");
        assert_eq!(
            timed_out,
            mode == "hang",
            "{mode}: native collector/termination exceeded supervisor deadline"
        );
        if mode == "native-fault" {
            assert_eq!(status.code().map(|code| code as u32), Some(0xE042_5649));
        }
        let panic_report = fs::read(directory.join("panic-report.json")).unwrap();
        if mode == "panic" {
            let envelope: Envelope = serde_json::from_slice(&panic_report).unwrap();
            assert_eq!(envelope.revisions.document, 2);
            assert_eq!(envelope.identity.build, "fixture-build-2026-10-25");
            if cfg!(windows) {
                assert!(
                    envelope.identity.symbols.len() >= 33
                        && envelope
                            .identity
                            .symbols
                            .bytes()
                            .all(|b| b.is_ascii_hexdigit())
                );
            }
            assert!(!String::from_utf8_lossy(&panic_report).contains("private panic payload"));
            assert_eq!(
                fs::read_to_string(directory.join("host-hook.txt")).unwrap(),
                "host hook preserved"
            );
        } else {
            assert!(
                panic_report.is_empty(),
                "fatal OS paths must not pretend to run Rust panic reporting"
            );
            assert!(!directory.join("host-hook.txt").exists());
        }
        assert!(directory.join("healthy-context.json").is_file());
        assert!(
            directory
                .join("checkpoint-00000000000000000003.partial")
                .is_file()
        );
        // Corrupt and unsupported later revisions are independently rejected.
        fs::write(
            directory.join("checkpoint-00000000000000000004.json"),
            b"{torn}",
        )
        .unwrap();
        fs::write(
            directory.join("checkpoint-00000000000000000005.json"),
            br#"{"schema":2,"revision":5,"text":"future","checksum":0}"#,
        )
        .unwrap();
        let mut restore = launch("restore", &directory, "candidate");
        let (restore_status, timeout) = wait(&mut restore, Duration::from_secs(15));
        assert!(restore_status.success() && !timeout);
        let candidate: serde_json::Value =
            serde_json::from_slice(&fs::read(directory.join("candidate-stdout.json")).unwrap())
                .unwrap();
        assert_eq!(candidate["candidate"]["revision"], 2);
        assert_eq!(candidate["rejected"], 2);
        assert!(
            !directory.join("restored.txt").exists(),
            "recovery requires an application restore choice"
        );
        let mut accept = launch("restore-accept", &directory, "accepted");
        let (accepted_status, timeout) = wait(&mut accept, Duration::from_secs(15));
        assert!(accepted_status.success() && !timeout);
        assert_eq!(
            fs::read_to_string(directory.join("restored.txt")).unwrap(),
            "Unsaved second edit survives process failure"
        );
        let observation = serde_json::json!({"mode":mode,"exit_code":status.code(),"supervisor_timeout":timed_out,"directory":directory,"panic_report_bytes":panic_report.len(),"recovered_revision":2,"wer_dump_collection":"not configured or qualified"});
        fs::write(
            directory.join("supervisor.json"),
            serde_json::to_vec_pretty(&observation).unwrap(),
        )
        .unwrap();
        println!("{observation}");
        manifest.push(observation);
    }
    let directory = fixture_directory("manifest");
    fs::write(
        directory.join("manifest.json"),
        serde_json::to_vec_pretty(&manifest).unwrap(),
    )
    .unwrap();
    println!(
        "supervisor manifest: {}",
        directory.join("manifest.json").display()
    );
}
