use view_core::{
    Alignment, Axis, Constraints, CoreError, CrossAxisAlignment, Dirty, FlexItem, FlexLayout,
    LayoutCache, LogicalInsets, LogicalRect, LogicalSize, MainAxisAlignment, PaddingLayout,
    StackLayout,
};

#[test]
fn test_constraints_validation_and_methods() {
    assert!(Constraints::new(0.0, 100.0, 0.0, 200.0).is_ok());
    assert_eq!(
        Constraints::new(100.0, 50.0, 0.0, 10.0),
        Err(CoreError::InvalidConstraints)
    );
    assert_eq!(
        Constraints::new(-5.0, 10.0, 0.0, 10.0),
        Err(CoreError::InvalidConstraints)
    );

    let tight = Constraints::tight(LogicalSize::new(50.0, 60.0));
    assert!(tight.is_tight());
    assert_eq!(
        tight.constrain(LogicalSize::new(10.0, 10.0)),
        LogicalSize::new(50.0, 60.0)
    );

    let loose = Constraints::loose(LogicalSize::new(100.0, 100.0));
    assert_eq!(
        loose.constrain(LogicalSize::new(50.0, 75.0)),
        LogicalSize::new(50.0, 75.0)
    );
    assert_eq!(
        loose.constrain(LogicalSize::new(150.0, 200.0)),
        LogicalSize::new(100.0, 100.0)
    );
}

#[test]
fn test_padding_layout() {
    let padding = PaddingLayout {
        insets: LogicalInsets::new(10.0, 20.0, 30.0, 40.0),
    };
    let constraints = Constraints::loose(LogicalSize::new(200.0, 200.0));
    let child_size = Some(LogicalSize::new(50.0, 50.0));
    let (container_size, child_rect) = padding.layout(constraints, child_size);

    // width: 50 + 40 + 20 = 110, height: 50 + 10 + 30 = 90
    assert_eq!(container_size, LogicalSize::new(110.0, 90.0));
    assert_eq!(
        child_rect,
        Some(LogicalRect::from_xywh(40.0, 10.0, 50.0, 50.0))
    );
}

#[test]
fn test_flex_row_layout_with_flex_factors_and_alignment() {
    let row = FlexLayout {
        axis: Axis::Horizontal,
        main_axis_alignment: MainAxisAlignment::Start,
        cross_axis_alignment: CrossAxisAlignment::Center,
        spacing: 10.0,
    };
    let constraints = Constraints::tight(LogicalSize::new(300.0, 100.0));
    let children = vec![
        (FlexItem::FIXED, LogicalSize::new(50.0, 40.0)),
        (FlexItem::EXPANDED, LogicalSize::new(0.0, 60.0)),
    ];
    let (size, rects) = row.layout(constraints, &children);
    assert_eq!(size, LogicalSize::new(300.0, 100.0));
    assert_eq!(rects.len(), 2);
    // Fixed item at start, y centered: (100 - 40) / 2 = 30
    assert_eq!(rects[0], LogicalRect::from_xywh(0.0, 30.0, 50.0, 40.0));
    // Remaining space: 300 - 50 - 10 (spacing) = 240
    // Expanded item starts at 50 + 10 = 60, y centered: (100 - 60) / 2 = 20
    assert_eq!(rects[1], LogicalRect::from_xywh(60.0, 20.0, 240.0, 60.0));
}

#[test]
fn test_stack_layout_alignment() {
    let stack = StackLayout {
        alignment: Alignment::CENTER,
    };
    let constraints = Constraints::tight(LogicalSize::new(200.0, 200.0));
    let children = vec![LogicalSize::new(100.0, 100.0), LogicalSize::new(50.0, 50.0)];
    let (size, rects) = stack.layout(constraints, &children);
    assert_eq!(size, LogicalSize::new(200.0, 200.0));
    assert_eq!(rects[0], LogicalRect::from_xywh(50.0, 50.0, 100.0, 100.0));
    assert_eq!(rects[1], LogicalRect::from_xywh(75.0, 75.0, 50.0, 50.0));
}

#[test]
fn test_layout_cache_pure_measurement() {
    let mut cache = LayoutCache::default();
    let constraints = Constraints::tight(LogicalSize::new(100.0, 100.0));
    assert_eq!(cache.get(constraints, Dirty::NONE), None);

    cache.store(constraints, LogicalSize::new(100.0, 100.0));
    assert_eq!(
        cache.get(constraints, Dirty::NONE),
        Some(LogicalSize::new(100.0, 100.0))
    );

    // Dirty::LAYOUT invalidates cache query.
    assert_eq!(cache.get(constraints, Dirty::LAYOUT), None);

    // Different constraints invalidates cache query.
    let different = Constraints::loose(LogicalSize::new(50.0, 50.0));
    assert_eq!(cache.get(different, Dirty::NONE), None);
}
