#![cfg(feature = "scene")]
use std::sync::Arc;
use view_core::Revision;
use view_wgpu::{scene::*, *};

fn camera() -> PerspectiveCamera {
    PerspectiveCamera::look_at(
        [0.0; 3],
        [0.0, 0.0, -1.0],
        [0.0, 1.0, 0.0],
        std::f32::consts::FRAC_PI_2,
        1.0,
        10.0,
    )
    .unwrap()
}
fn object(id: u64) -> SceneObject {
    SceneObject {
        id,
        mesh: Arc::new(
            Mesh3D::new(
                vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
                vec![0, 1, 2],
            )
            .unwrap(),
        ),
        transform: Transform3D::IDENTITY,
        material: BasicMaterial::new([1.0, 0.0, 0.0]).unwrap(),
    }
}
fn snapshot(revision: u64, objects: Vec<SceneObject>) -> SceneSnapshot {
    SceneSnapshot::new(Revision::from_raw(revision), camera(), objects).unwrap()
}
fn close(actual: f32, expected: f32) {
    assert!((actual - expected).abs() < 1e-5, "{actual} != {expected}");
}

#[test]
fn projection_maps_near_far_and_aspect_to_wgpu_clip_space() {
    let vp = camera().view_projection(2.0).unwrap();
    let near = vp.transform([0.0, 0.0, -1.0, 1.0]).unwrap();
    let far = vp.transform([0.0, 0.0, -10.0, 1.0]).unwrap();
    close(near[2] / near[3], 0.0);
    close(far[2] / far[3], 1.0);
    let edge = vp.transform([4.0, 2.0, -2.0, 1.0]).unwrap();
    close(edge[0] / edge[3], 1.0);
    close(edge[1] / edge[3], 1.0);
    assert!(vp.transform([0.0, 0.0, 2.0, 1.0]).unwrap()[3] < 0.0);
}

#[test]
fn look_at_handles_translated_and_rotated_camera_axes() {
    let camera = PerspectiveCamera::look_at(
        [2.0, 3.0, 4.0],
        [1.0, 3.0, 4.0],
        [0.0, 1.0, 0.0],
        1.0,
        0.1,
        100.0,
    )
    .unwrap();
    assert_eq!(
        camera.view().transform([2.0, 3.0, 4.0, 1.0]).unwrap(),
        [0.0, 0.0, 0.0, 1.0]
    );
    assert_eq!(
        camera.view().transform([1.0, 3.0, 4.0, 1.0]).unwrap(),
        [0.0, 0.0, -1.0, 1.0]
    );
    assert_eq!(
        camera.view().transform([2.0, 4.0, 4.0, 1.0]).unwrap(),
        [0.0, 1.0, 0.0, 1.0]
    );
}

#[test]
fn model_transform_order_and_handedness_are_explicit() {
    let transform = Transform3D::scale([2.0, 3.0, 4.0])
        .unwrap()
        .then(Transform3D::rotation_y(std::f32::consts::FRAC_PI_2).unwrap())
        .unwrap()
        .then(Transform3D::translation([10.0, 20.0, 30.0]).unwrap())
        .unwrap();
    let point = transform.matrix().transform([1.0, 1.0, 0.0, 1.0]).unwrap();
    for (a, b) in point.into_iter().zip([10.0, 23.0, 28.0, 1.0]) {
        close(a, b);
    }
    let mut perspective = Matrix4::IDENTITY.columns();
    perspective[0][3] = 1.0;
    assert!(Transform3D::from_columns(perspective).is_err());
    assert!(Transform3D::translation([f32::NAN, 0.0, 0.0]).is_err());
    let huge = Transform3D::scale([f32::MAX; 3]).unwrap();
    assert!(huge.then(huge).is_err());
    // Cancellation in f64 must not conceal intermediates that overflow in WGSL.
    let cancel = Transform3D::from_columns([
        [f32::MAX, 0.0, 0.0, 0.0],
        [-f32::MAX, 1.0, 0.0, 0.0],
        [0.0, 0.0, 1.0, 0.0],
        [0.0, 0.0, 0.0, 1.0],
    ])
    .unwrap();
    assert!(cancel.matrix().transform([2.0, 2.0, 0.0, 1.0]).is_err());
}

#[test]
fn invalid_camera_and_geometry_reject_without_gpu_access() {
    for (eye, target, up, fov, near, far) in [
        ([0.0; 3], [0.0; 3], [0.0, 1.0, 0.0], 1.0, 1.0, 10.0),
        ([0.0; 3], [0.0, 0.0, -1.0], [0.0, 0.0, 1.0], 1.0, 1.0, 10.0),
        ([0.0; 3], [0.0, 0.0, -1.0], [0.0, 1.0, 0.0], 0.0, 1.0, 10.0),
        ([0.0; 3], [0.0, 0.0, -1.0], [0.0, 1.0, 0.0], 1.0, 0.0, 10.0),
        ([0.0; 3], [0.0, 0.0, -1.0], [0.0, 1.0, 0.0], 1.0, 10.0, 1.0),
        (
            [f32::INFINITY; 3],
            [0.0, 0.0, -1.0],
            [0.0, 1.0, 0.0],
            1.0,
            1.0,
            10.0,
        ),
    ] {
        assert!(PerspectiveCamera::look_at(eye, target, up, fov, near, far).is_err());
    }
    for aspect in [0.0, -1.0, f32::NAN, f32::INFINITY] {
        assert!(camera().view_projection(aspect).is_err());
    }
    assert!(Mesh3D::new(vec![], vec![]).is_err());
    assert!(Mesh3D::new(vec![[0.0; 3]], vec![0, 0]).is_err());
    assert!(Mesh3D::new(vec![[0.0; 3]], vec![0, 0, 1]).is_err());
    assert!(Mesh3D::new(vec![[f32::NAN; 3]], vec![0, 0, 0]).is_err());
    for color in [[-0.1, 0.0, 0.0], [1.1, 0.0, 0.0], [f32::NAN, 0.0, 0.0]] {
        assert!(BasicMaterial::new(color).is_err());
    }
}

#[test]
fn snapshots_reject_duplicate_ids_and_updates_are_revision_and_budget_atomic() {
    assert!(SceneSnapshot::new(Revision::INITIAL, camera(), vec![object(1), object(1)]).is_err());
    let budgets = SceneBudgets {
        max_objects: 1,
        upload_bytes: 128,
        ..Default::default()
    };
    // Triangle = 36 vertex bytes + 12 index bytes + 80 uniform bytes.
    let mut provider = SceneProvider::new(snapshot(5, vec![object(1)]), budgets).unwrap();
    assert!(provider.set_snapshot(snapshot(5, vec![])).is_err());
    assert!(provider.set_snapshot(snapshot(4, vec![])).is_err());
    assert_eq!(
        provider.set_snapshot(snapshot(6, vec![object(2), object(3)])),
        Err(RenderError::BudgetExceeded)
    );
    assert_eq!(provider.snapshot().revision(), Revision::from_raw(5));
    assert_eq!(provider.snapshot().objects()[0].id, 1);
    provider.set_snapshot(snapshot(6, vec![])).unwrap();
    assert!(provider.snapshot().objects().is_empty());
    assert!(matches!(
        SceneProvider::new(
            snapshot(0, vec![object(1)]),
            SceneBudgets {
                upload_bytes: 127,
                ..budgets
            }
        ),
        Err(RenderError::BudgetExceeded)
    ));
}

#[test]
fn provider_negotiates_depth_offscreen_and_validates_target_budget() {
    let mut provider = SceneProvider::new(
        snapshot(0, vec![]),
        SceneBudgets {
            target_pixels: 4096,
            ..Default::default()
        },
    )
    .unwrap();
    let contract = AttachmentContract {
        format: wgpu::TextureFormat::Rgba8UnormSrgb,
        size: [64, 64],
        sample_count: 1,
        isolated: false,
    };
    assert!(provider.requires_depth());
    assert_eq!(
        provider.negotiate(contract).unwrap(),
        ContributionMode::Offscreen
    );
    assert!(provider.resize([0, 64]).is_err());
    assert_eq!(provider.resize([65, 64]), Err(RenderError::BudgetExceeded));
    assert!(
        provider
            .negotiate(AttachmentContract {
                sample_count: 4,
                ..contract
            })
            .is_err()
    );
    assert!(
        provider
            .negotiate(AttachmentContract {
                format: wgpu::TextureFormat::Rgba16Float,
                ..contract
            })
            .is_err()
    );
    provider.resize([32, 64]).unwrap();
}
