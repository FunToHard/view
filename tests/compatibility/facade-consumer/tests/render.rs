#![cfg(feature = "wgpu")]
#[test]
fn renderer_profile_is_available_without_root_feature_unification() {
    use view::render::*;
    let contract = AttachmentContract {
        format: wgpu::TextureFormat::Rgba8UnormSrgb,
        size: [100, 100],
        sample_count: 1,
        isolated: false,
    };
    assert_eq!(
        contract.negotiate(ContributionMode::Direct).unwrap(),
        ContributionMode::Direct
    );
    let paint = PaintSnapshot {
        revision: view::Revision::INITIAL,
        size: [100, 100],
        scale: view::ScaleFactor::ONE,
        items: vec![Paint::Draw(Draw::new(
            Primitive::Rect(view::LogicalRect::new(
                view::LogicalPoint::ZERO,
                view::LogicalSize::new(10.0, 10.0),
            )),
            Color::WHITE,
        ))],
    };
    assert_eq!(paint.items.len(), 1);
}
