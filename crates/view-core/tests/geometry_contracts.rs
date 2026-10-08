use view_core::{
    CoreError, DocumentPoint, LogicalInsets, LogicalPoint, LogicalRect, LogicalSize, PhysicalPoint,
    PhysicalSize, ScaleFactor, Transform2D,
};

#[test]
fn test_scale_factor_validation_and_conversion() {
    assert!(ScaleFactor::new(1.0).is_ok());
    assert!(ScaleFactor::new(1.5).is_ok());
    assert!(ScaleFactor::new(2.0).is_ok());
    assert_eq!(ScaleFactor::new(0.0), Err(CoreError::InvalidGeometry));
    assert_eq!(ScaleFactor::new(-1.0), Err(CoreError::InvalidGeometry));
    assert_eq!(ScaleFactor::new(f32::NAN), Err(CoreError::InvalidGeometry));
    assert_eq!(
        ScaleFactor::new(f32::INFINITY),
        Err(CoreError::InvalidGeometry)
    );

    let scale = ScaleFactor::new(2.0).unwrap();
    let logical = LogicalPoint::new(10.0, 20.0);
    let physical = logical.to_physical(scale);
    assert_eq!(physical, PhysicalPoint::new(20.0, 40.0));
    assert_eq!(physical.to_logical(scale), logical);

    let logical_size = LogicalSize::new(100.0, 50.0);
    let physical_size = logical_size.to_physical(scale);
    assert_eq!(physical_size, PhysicalSize::new(200.0, 100.0));
    assert_eq!(physical_size.to_logical(scale), logical_size);
}

#[test]
fn test_finite_validation_and_rect_operations() {
    assert!(LogicalPoint::new_checked(0.0, 0.0).is_ok());
    assert_eq!(
        LogicalPoint::new_checked(f32::NAN, 1.0),
        Err(CoreError::InvalidGeometry)
    );
    assert_eq!(
        LogicalPoint::new_checked(1.0, f32::INFINITY),
        Err(CoreError::InvalidGeometry)
    );

    assert!(LogicalSize::new_checked(10.0, 20.0).is_ok());
    assert_eq!(
        LogicalSize::new_checked(-1.0, 10.0),
        Err(CoreError::InvalidGeometry)
    );
    assert_eq!(
        LogicalSize::new_checked(10.0, f32::NAN),
        Err(CoreError::InvalidGeometry)
    );

    let rect1 = LogicalRect::from_xywh(10.0, 10.0, 40.0, 40.0);
    let rect2 = LogicalRect::from_xywh(30.0, 30.0, 40.0, 40.0);
    assert!(rect1.contains(LogicalPoint::new(20.0, 20.0)));
    assert!(!rect1.contains(LogicalPoint::new(5.0, 20.0)));
    assert!(rect1.intersects(rect2));

    let intersection = rect1.intersection(rect2).unwrap();
    assert_eq!(intersection, LogicalRect::from_xywh(30.0, 30.0, 20.0, 20.0));

    let union = rect1.union(rect2);
    assert_eq!(union, LogicalRect::from_xywh(10.0, 10.0, 60.0, 60.0));

    let insets = LogicalInsets::all(5.0);
    let deflated = rect1.deflate(insets);
    assert_eq!(deflated, LogicalRect::from_xywh(15.0, 15.0, 30.0, 30.0));
    let inflated = deflated.inflate(insets);
    assert_eq!(inflated, rect1);
}

#[test]
fn test_transform2d_composition_and_inversion() {
    let t_translate = Transform2D::translation(20.0, 30.0);
    let t_scale = Transform2D::scale(2.0, 3.0);
    let composed = t_translate.then(&t_scale);

    let p = LogicalPoint::new(10.0, 10.0);
    // (10 + 20) * 2 = 60, (10 + 30) * 3 = 120
    let transformed = composed.transform_point(p);
    assert_eq!(transformed, LogicalPoint::new(60.0, 120.0));

    let inv = composed.inverse().expect("invertible");
    let back = inv.transform_point(transformed);
    assert!((back.x - p.x).abs() < 1e-4);
    assert!((back.y - p.y).abs() < 1e-4);

    // Singular matrix has no inverse.
    let singular = Transform2D::new([0.0, 0.0, 0.0, 0.0, 10.0, 10.0]).unwrap();
    assert!(singular.inverse().is_none());
}

#[test]
fn test_document_coordinate_space_mapping() {
    let transform = Transform2D::scale(2.0, 2.0).then(&Transform2D::translation(100.0, 50.0));
    let doc_point = DocumentPoint::new(10.0, 20.0);
    let logical_point = doc_point.to_logical(&transform);
    assert_eq!(logical_point, LogicalPoint::new(120.0, 90.0));

    let mapped_back = logical_point.to_document(&transform).unwrap();
    assert_eq!(mapped_back, doc_point);
}
