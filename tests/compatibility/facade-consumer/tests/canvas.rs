use view::canvas::{Canvas2D, CanvasId, CanvasPoint, CanvasRecord, CanvasRect, CanvasView};
use view::{LogicalPoint, LogicalSize};

#[test]
fn canvas_is_available_without_backend_features() {
    let origin = CanvasPoint::new(1e12, 1e12).unwrap();
    let end = CanvasPoint::new(1e12 + 10.0, 1e12 + 10.0).unwrap();
    let camera = CanvasView::new(origin, 2.0, LogicalSize::new(100.0, 100.0)).unwrap();
    let mut canvas = Canvas2D::new(camera, 8);
    canvas
        .update(Some(vec![CanvasRecord {
            id: CanvasId(42),
            bounds: CanvasRect::new(origin, end).unwrap(),
            hit_enabled: true,
            semantics: None,
            draw: "app-owned mesh",
        }]))
        .unwrap();
    assert_eq!(
        canvas.hit_test(LogicalPoint::new(5.0, 5.0)).unwrap(),
        Some(CanvasId(42))
    );
    assert_eq!(
        canvas.view().project(end).unwrap(),
        LogicalPoint::new(20.0, 20.0)
    );
    let revision = canvas.revision();
    assert_eq!(canvas.update(None).unwrap(), revision);
}
