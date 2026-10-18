#[path = "../examples/support/brush.rs"]
mod brush;
use brush::*;
use view_core::canvas::{CanvasPoint, CanvasView};
use view_core::{LogicalPoint, LogicalSize};
#[test]
fn tiled_stroke_crosses_negative_tile_edges_and_one_undo_restores_exact_document() {
    let mut doc = BrushDocument::new(8, 64, 4);
    let camera = CanvasView::new(
        CanvasPoint::new(-16., -16.).unwrap(),
        2.,
        LogicalSize::new(64., 64.),
    )
    .unwrap();
    doc.begin(camera.document_at(LogicalPoint::new(28., 32.)).unwrap())
        .unwrap();
    doc.append(camera.document_at(LogicalPoint::new(36., 32.)).unwrap())
        .unwrap();
    assert!(doc.tiles().is_empty());
    assert!(!doc.preview().is_empty());
    assert!(doc.commit().unwrap());
    assert!(!doc.commit().unwrap());
    assert_eq!(doc.undo_len(), 1);
    assert!(doc.tiles().contains_key(&(-1, 0)));
    assert!(doc.tiles().contains_key(&(0, -1)));
    assert!(doc.undo());
    assert!(doc.tiles().is_empty());
    assert_eq!(PRESSURE_TILT, (false, false));
}
#[test]
fn preview_and_document_budgets_fail_atomically_and_cancel_never_adds_history() {
    let mut doc = BrushDocument::new(1, 8, 2);
    doc.begin(CanvasPoint::new(8., 8.).unwrap()).unwrap();
    let before = doc.preview().to_vec();
    assert!(doc.append(CanvasPoint::new(100., 8.).unwrap()).is_err());
    assert_eq!(doc.preview(), before);
    doc.commit().unwrap();
    let tiles = doc.tiles().clone();
    doc.begin(CanvasPoint::new(16., 16.).unwrap()).unwrap();
    assert!(doc.commit().is_err());
    assert_eq!(doc.tiles(), &tiles);
    assert_eq!(doc.undo_len(), 1);
    doc.cancel();
    assert_eq!(doc.undo_len(), 1);
    assert!(doc.preview().is_empty());
    assert!(doc.undo());
    assert!(doc.tiles().is_empty());
}
