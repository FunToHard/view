#![cfg(feature = "scene")]
use view::scene::*;

#[test]
fn optional_scene_api_builds_without_workspace_feature_unification() {
    let camera =
        PerspectiveCamera::look_at([0.0, 0.0, 2.0], [0.0; 3], [0.0, 1.0, 0.0], 1.0, 0.1, 100.0)
            .unwrap();
    let scene = SceneSnapshot::new(view::Revision::INITIAL, camera, vec![]).unwrap();
    let provider = SceneProvider::new(scene, SceneBudgets::default()).unwrap();
    assert!(provider.snapshot().objects().is_empty());
    let mut picks = PickTracker::new().unwrap();
    let stamp = PickStamp {
        scene: view::Revision::INITIAL,
        camera: view::Revision::INITIAL,
        viewport: view::Revision::INITIAL,
    };
    let request = picks
        .request(
            stamp,
            view::LogicalPoint::new(5.0, 5.0),
            view::LogicalSize::new(10.0, 10.0),
        )
        .unwrap();
    assert_eq!(
        picks.accept(provider.snapshot().pick(request).unwrap(), stamp),
        Some(None)
    );
    let frames = view::viewport::ViewportScheduler::new(2);
    assert!(frames.is_empty());
    assert_eq!(
        camera.view().transform([0.0, 0.0, 2.0, 1.0]).unwrap(),
        [0.0, 0.0, 0.0, 1.0]
    );
}
