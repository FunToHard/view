//! CPU-only document/view mapping example; no native window or input injection.
use view_core::canvas::*;
use view_core::{LogicalPoint, LogicalSize};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let origin = CanvasPoint::new(1e12, 1e12)?;
    let end = CanvasPoint::new(1e12 + 10.0, 1e12 + 20.0)?;
    let camera = CanvasView::new(origin, 2.0, LogicalSize::new(640.0, 480.0))?;
    let mut canvas = Canvas2D::new(camera, 100);
    canvas.update(Some(vec![CanvasRecord {
        id: CanvasId(1),
        bounds: CanvasRect::new(origin, end)?,
        hit_enabled: true,
        semantics: None,
        draw: "rectangle",
    }]))?;
    let mut camera = canvas.view();
    camera.zoom_at(LogicalPoint::new(10.0, 20.0), 4.0)?;
    canvas.set_view(camera)?;
    for record in canvas.records() {
        println!(
            "{}: {:?} to {:?}",
            record.draw,
            canvas.view().project(record.bounds.min)?,
            canvas.view().project(record.bounds.max)?
        );
    }
    assert_eq!(
        canvas.hit_test(LogicalPoint::new(10.0, 20.0))?,
        Some(CanvasId(1))
    );
    Ok(())
}
