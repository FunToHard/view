use view_core::canvas::*;
use view_core::{CoreError, LogicalPoint, LogicalSize, Role, SemanticAction};

fn point(x: f64, y: f64) -> CanvasPoint {
    CanvasPoint::new(x, y).unwrap()
}

fn view() -> CanvasView {
    CanvasView::new(point(100.0, 200.0), 2.0, LogicalSize::new(100.0, 80.0)).unwrap()
}

fn record(id: u64, min: CanvasPoint, max: CanvasPoint) -> CanvasRecord<&'static str> {
    CanvasRecord {
        id: CanvasId(id),
        bounds: CanvasRect::new(min, max).unwrap(),
        hit_enabled: true,
        semantics: Some(CanvasSemantics {
            name: format!("Object {id}"),
            role: Role::Button,
            actions: vec![SemanticAction::Click],
        }),
        draw: "rectangle",
    }
}

#[test]
fn large_document_rebases_before_narrowing_and_reports_precision_loss() {
    let view = CanvasView::new(point(1e12, -1e12), 4.0, LogicalSize::new(800.0, 600.0)).unwrap();
    let document = point(1e12 + 0.25, -1e12 + 0.5);
    assert_eq!(view.project(document).unwrap(), LogicalPoint::new(1.0, 2.0));
    assert_eq!(
        view.document_at(LogicalPoint::new(1.0, 2.0)).unwrap(),
        document
    );
    assert_eq!(
        view.project(point(1e12 + 16_777_217.0 / 4.0, -1e12)),
        Err(CoreError::InvalidGeometry)
    );
    assert!(view.project(point(f64::MAX, 0.0)).is_err());
}

#[test]
fn anchored_zoom_and_pan_have_analytic_coordinates_and_atomic_failures() {
    let mut camera = view();
    let anchor = LogicalPoint::new(20.0, 40.0);
    camera.zoom_at(anchor, 4.0).unwrap();
    assert_eq!(camera.origin(), point(105.0, 210.0));
    assert_eq!(camera.document_at(anchor).unwrap(), point(110.0, 220.0));
    camera.pan(LogicalPoint::new(8.0, -12.0)).unwrap();
    assert_eq!(camera.origin(), point(103.0, 213.0));
    assert_eq!(
        camera.project(point(110.0, 220.0)).unwrap(),
        LogicalPoint::new(28.0, 28.0)
    );
    let before = camera;
    for zoom in [0.0, -1.0, f64::NAN, f64::INFINITY] {
        assert!(camera.zoom_at(anchor, zoom).is_err());
        assert_eq!(camera, before);
    }
    assert!(camera.pan(LogicalPoint::new(f32::INFINITY, 0.0)).is_err());
    assert!(camera.resize(LogicalSize::new(-1.0, 10.0)).is_err());
    assert_eq!(camera, before);
}

#[test]
fn hits_semantics_and_draw_order_share_camera_clip_and_revision() {
    let mut canvas = Canvas2D::new(view(), 4);
    canvas
        .update(Some(vec![
            record(1, point(90.0, 190.0), point(120.0, 220.0)),
            record(2, point(110.0, 210.0), point(130.0, 230.0)),
            record(3, point(300.0, 300.0), point(400.0, 400.0)),
        ]))
        .unwrap();
    assert_eq!(
        canvas.hit_test(LogicalPoint::new(25.0, 25.0)).unwrap(),
        Some(CanvasId(2))
    );
    assert_eq!(
        canvas.hit_test(LogicalPoint::new(0.0, 0.0)).unwrap(),
        Some(CanvasId(1))
    );
    assert_eq!(canvas.hit_test(LogicalPoint::new(-1.0, 0.0)).unwrap(), None);
    assert_eq!(
        canvas.hit_test(LogicalPoint::new(60.0, 60.0)).unwrap(),
        None
    );
    assert_eq!(
        canvas.hit_test(LogicalPoint::new(100.0, 80.0)).unwrap(),
        None
    );
    let regions = canvas.semantic_regions().unwrap();
    assert_eq!(regions.len(), 2);
    assert_eq!(regions[0].bounds.origin, LogicalPoint::ZERO);
    assert_eq!(regions[0].bounds.size, LogicalSize::new(40.0, 40.0));
    assert_eq!(regions[1].bounds.origin, LogicalPoint::new(20.0, 20.0));
    assert_eq!(regions[1].revision, canvas.revision());
    assert_eq!(canvas.records()[1].id, regions[1].id);
    let old_revision = canvas.revision();
    let mut camera = canvas.view();
    camera.pan(LogicalPoint::new(10.0, 0.0)).unwrap();
    canvas.set_view(camera).unwrap();
    assert!(canvas.revision() > old_revision);
    assert_eq!(
        canvas.semantic_regions().unwrap()[1].bounds.origin,
        LogicalPoint::new(30.0, 20.0)
    );
    assert_eq!(
        canvas.hit_test(LogicalPoint::new(25.0, 25.0)).unwrap(),
        Some(CanvasId(1))
    );
    camera.resize(LogicalSize::new(0.0, 0.0)).unwrap();
    canvas.set_view(camera).unwrap();
    assert!(canvas.semantic_regions().unwrap().is_empty());
    assert_eq!(canvas.hit_test(LogicalPoint::ZERO).unwrap(), None);
}

#[test]
fn skipped_updates_retain_records_and_rejected_updates_are_atomic() {
    let mut canvas = Canvas2D::new(view(), 2);
    let one = record(1, point(100.0, 200.0), point(110.0, 210.0));
    let revision = canvas.update(Some(vec![one.clone()])).unwrap();
    assert_eq!(canvas.update(None).unwrap(), revision);
    assert_eq!(canvas.set_view(view()).unwrap(), revision);
    assert_eq!(
        canvas.update(Some(vec![one.clone(), one.clone()])),
        Err(CoreError::DuplicateKey)
    );
    assert_eq!(
        canvas.update(Some(vec![one.clone(); 3])),
        Err(CoreError::CapacityExhausted)
    );
    let mut invalid = one.clone();
    invalid.bounds.max.x = f64::NAN;
    assert_eq!(
        canvas.update(Some(vec![invalid])),
        Err(CoreError::InvalidGeometry)
    );
    assert_eq!(canvas.records(), &[one]);
    assert_eq!(canvas.revision(), revision);
    canvas.update(Some(vec![])).unwrap();
    assert!(canvas.records().is_empty());
    assert!(canvas.revision() > revision);
}

#[test]
fn semantic_only_records_do_not_intercept_pointer_hits() {
    let mut canvas = Canvas2D::new(view(), 2);
    let bottom = record(1, point(100.0, 200.0), point(120.0, 220.0));
    let mut top = bottom.clone();
    top.id = CanvasId(2);
    top.hit_enabled = false;
    canvas.update(Some(vec![bottom, top])).unwrap();
    assert_eq!(
        canvas.hit_test(LogicalPoint::new(10.0, 10.0)).unwrap(),
        Some(CanvasId(1))
    );
    assert_eq!(canvas.semantic_regions().unwrap().len(), 2);
    assert!(canvas.hit_test(LogicalPoint::new(f32::NAN, 0.0)).is_err());
}
