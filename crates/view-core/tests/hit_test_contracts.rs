use view_core::{
    ClipChain, EllipseHit, HitNodeEntry, LogicalPoint, LogicalRect, Transform2D, TransformChain,
};

#[test]
fn test_clip_chain_rejection() {
    let clip = ClipChain::new(LogicalRect::from_xywh(0.0, 0.0, 100.0, 100.0));
    assert!(clip.contains(LogicalPoint::new(50.0, 50.0)));
    assert!(!clip.contains(LogicalPoint::new(150.0, 50.0)));

    let nested_clip = clip.intersect(
        Some(LogicalRect::from_xywh(20.0, 20.0, 50.0, 50.0)),
        &Transform2D::IDENTITY,
    );
    assert!(nested_clip.contains(LogicalPoint::new(30.0, 30.0)));
    assert!(!nested_clip.contains(LogicalPoint::new(10.0, 10.0)));
}

#[test]
fn test_paint_order_reverse_traversal() {
    let mut runtime = view_core::Runtime::<(), ()>::new(4).unwrap();
    let (window, root) = runtime
        .open_window(
            view_core::Description::new("root", (), |_, _, _| view_core::Dirty::NONE)
                .layout(LogicalRect::from_xywh(0.0, 0.0, 200.0, 200.0)),
        )
        .unwrap();

    // Bottom child
    let bottom = runtime
        .mount(
            root,
            view_core::Description::new("bottom", (), |_, _, _| view_core::Dirty::NONE)
                .layout(LogicalRect::from_xywh(10.0, 10.0, 100.0, 100.0)),
        )
        .unwrap();

    // Top overlapping child (declared after bottom)
    let top = runtime
        .mount(
            root,
            view_core::Description::new("top", (), |_, _, _| view_core::Dirty::NONE)
                .layout(LogicalRect::from_xywh(20.0, 20.0, 100.0, 100.0)),
        )
        .unwrap();

    let mut model = ();
    runtime.flush(&mut model).unwrap();

    // Hit test at (30, 30) where both overlap: top child must win due to reverse paint order!
    let hit = runtime
        .hit_test(window, LogicalPoint::new(30.0, 30.0))
        .unwrap()
        .expect("hit");
    assert_eq!(hit.target, top);

    // Hit test at (15, 15) where only bottom exists:
    let hit_bottom = runtime
        .hit_test(window, LogicalPoint::new(15.0, 15.0))
        .unwrap()
        .expect("hit");
    assert_eq!(hit_bottom.target, bottom);
}

#[test]
fn test_transformed_and_custom_hit_shapes() {
    let id = view_core::NodeId::from_handle(view_core::ArenaHandle::from_parts(
        view_core::ArenaId::new(std::num::NonZeroU64::new(1).unwrap()),
        0,
        view_core::Generation::INITIAL,
    ));

    let ellipse_contract = EllipseHit;
    let node = HitNodeEntry {
        id,
        local_bounds: LogicalRect::from_xywh(0.0, 0.0, 100.0, 100.0),
        local_transform: Transform2D::translation(50.0, 50.0),
        local_clip: None,
        children: &[],
        hit_contract: &ellipse_contract,
    };

    // Center of ellipse is at (50+50, 50+50) = (100, 100)
    let center_hit = node.hit_test(
        LogicalPoint::new(100.0, 100.0),
        ClipChain::NONE,
        TransformChain::IDENTITY,
    );
    assert!(center_hit.is_some());

    // Corner of bounding box at (51, 51) is outside ellipse:
    let corner_hit = node.hit_test(
        LogicalPoint::new(51.0, 51.0),
        ClipChain::NONE,
        TransformChain::IDENTITY,
    );
    assert!(corner_hit.is_none());
}
