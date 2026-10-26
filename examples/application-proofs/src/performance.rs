//! Opt-in CPU workload evidence. This does not measure GPU or visible input latency.
use crate::{Action, Product, Proofs, Result};
use std::{
    hint::black_box,
    time::{Duration, Instant},
};
use view::{LogicalPoint, LogicalSize, controls::VirtualList, resources::assets::Cache};

fn report(name: &str, samples: &mut [f64]) {
    samples.sort_by(f64::total_cmp);
    let percentile = |p: usize| samples[(samples.len() * p).div_ceil(100) - 1];
    println!(
        "CPU_WORKLOAD name={name} count={} p50_ms={:.6} p95_ms={:.6} p99_ms={:.6}",
        samples.len(),
        percentile(50),
        percentile(95),
        percentile(99)
    );
}

/// Exercise bounded virtualization, three-window state, asynchronous import pressure,
/// stale completions and demand-driven idle behavior using public APIs.
pub fn run() -> Result<()> {
    let mut list = VirtualList::new(
        (0..100_000).map(|i| format!("row-{i}")).collect(),
        24.0,
        LogicalSize::new(800.0, 600.0),
        26,
    )?;
    let mut times = Vec::with_capacity(1000);
    for i in 0..1000 {
        let started = Instant::now();
        let row = (i * 7919) % 99_975;
        list.scroll
            .scroll_to(LogicalPoint::new(0.0, row as f32 * 24.0));
        let realized = list.realized()?;
        assert_eq!(realized, row..row + 25);
        black_box(realized);
        times.push(started.elapsed().as_secs_f64() * 1000.0);
    }
    report("virtual-100000-rows-25-realized", &mut times);

    let mut app = Proofs::new()?;
    // Mount the initial catalog status before measuring steady-state retention.
    app.refresh_catalog()?;
    app.ui.flush(&mut app.document)?;
    let count = app.ui.snapshot().nodes.len();
    times.clear();
    for i in 0..1000 {
        let started = Instant::now();
        let base = app.document.revision;
        app.dispatch(Action::Move {
            base,
            x: i as f32 / 1000.0,
        })?;
        app.dispatch(Action::Undo)?;
        assert_eq!(app.document.x, 0.0);
        assert_eq!(app.ui.pending_actions(), 0);
        assert_eq!(app.ui.snapshot().nodes.len(), count);
        times.push(started.elapsed().as_secs_f64() * 1000.0);
    }
    report("three-window-edit-undo", &mut times);

    let before = app.ui.counters();
    for _ in 0..1000 {
        assert!(!app.ui.has_work());
        assert!(app.ui.flush(&mut app.document)?.committed.is_none());
    }
    let after = app.ui.counters();
    assert_eq!(before.layouts, after.layouts);
    assert_eq!(before.builds, after.builds);
    assert_eq!(before.commits, after.commits);
    println!("IDLE explicit_polls=1000 builds=0 layouts=0 commits=0; OS wakeups not measured");

    // Producer backpressure is explicit. At most four decoded payloads wait for
    // the owner thread; the asset cache retains at most eight 4 KiB payloads.
    let (sender, receiver) = std::sync::mpsc::sync_channel(4);
    let worker = std::thread::spawn(move || {
        for i in 0..1000 {
            if sender.send((i, vec![(i % 256) as u8; 4096])).is_err() {
                break;
            }
        }
    });
    let mut cache = Cache::new(8, 8 * 4096);
    times.clear();
    for _ in 0..1000 {
        let (i, data) = receiver.recv_timeout(Duration::from_secs(10))?;
        let started = Instant::now();
        cache
            .insert(format!("asset-{i}"), data, 4096, Duration::MAX)
            .map_err(|e| format!("asset cache: {e:?}"))?;
        assert!(cache.len() <= 8 && cache.bytes() <= 32768);
        let stale = app.search("obsolete")?;
        let current = app.search("current")?;
        assert!(!app.reply(&stale, vec![])?);
        assert!(app.reply(
            &current,
            vec![Product {
                id: i,
                name: format!("Asset {i}"),
                image: format!("asset-{i}"),
            }]
        )?);
        assert_eq!(app.ui.pending_actions(), 0);
        times.push(started.elapsed().as_secs_f64() * 1000.0);
    }
    worker.join().map_err(|_| "asset producer panicked")?;
    report(
        "bounded-background-import-and-stale-completions",
        &mut times,
    );
    // Image size is independent of UI realization. Each transaction touches one
    // sparse tile, and undo must release it without accumulating history.
    let mut image = view::render::document::ImageDocument::new(
        4096,
        4096,
        view::render::document::ImageBudget {
            max_tiles: 1,
            max_history: 1,
            max_history_bytes: 1024,
            ..Default::default()
        },
    )?;
    times.clear();
    for i in 0..1000_u32 {
        let started = Instant::now();
        let x = (i * 7919) % 4096;
        let y = (i * 3571) % 4096;
        image.set_pixel(x, y, [80, 160, 240, 255])?;
        assert_eq!(image.allocated_tiles(), 1);
        assert_eq!(image.undo_len(), 1);
        assert!(image.undo());
        assert_eq!(image.pixel(x, y).unwrap_or([0; 4]), [0; 4]);
        assert_eq!(image.allocated_tiles(), 0);
        assert_eq!(image.undo_len(), 0);
        times.push(started.elapsed().as_secs_f64() * 1000.0);
    }
    report("sparse-4096-image-edit-undo", &mut times);
    println!(
        "PASS CPU workload invariants; fixed 1000 operations, hardware rendering remains a separate fixture"
    );
    Ok(())
}
