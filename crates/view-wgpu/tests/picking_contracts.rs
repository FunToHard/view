#![cfg(feature = "scene")]
use std::sync::Arc;
use view_core::{LogicalPoint, LogicalRect, LogicalSize, Revision};
use view_wgpu::scene::*;
fn scene() -> SceneSnapshot {
    let mesh = Arc::new(
        Mesh3D::new(
            vec![[-1., -1., 0.], [1., -1., 0.], [0., 1., 0.]],
            vec![0, 1, 2],
        )
        .unwrap(),
    );
    let camera = PerspectiveCamera::look_at(
        [0.; 3],
        [0., 0., -1.],
        [0., 1., 0.],
        std::f32::consts::FRAC_PI_2,
        1.,
        10.,
    )
    .unwrap();
    SceneSnapshot::new(
        Revision::from_raw(1),
        camera,
        vec![
            SceneObject {
                id: 1,
                mesh: mesh.clone(),
                transform: Transform3D::translation([0., 0., -4.]).unwrap(),
                material: BasicMaterial::new([1., 0., 0.]).unwrap(),
            },
            SceneObject {
                id: 2,
                mesh,
                transform: Transform3D::translation([0., 0., -2.]).unwrap(),
                material: BasicMaterial::new([0., 1., 0.]).unwrap(),
            },
        ],
    )
    .unwrap()
}
fn stamp() -> PickStamp {
    PickStamp {
        scene: Revision::from_raw(1),
        camera: Revision::from_raw(2),
        viewport: Revision::from_raw(3),
    }
}
#[test]
fn nearest_triangle_overlay_and_outside_hit_have_analytic_expectations() {
    let scene = scene();
    let mut gate = PickTracker::new().unwrap();
    let request = gate
        .request(
            stamp(),
            LogicalPoint::new(50., 50.),
            LogicalSize::new(100., 100.),
        )
        .unwrap();
    let hit = gate
        .accept(scene.pick(request).unwrap(), stamp())
        .unwrap()
        .unwrap();
    assert_eq!(hit.object, 2);
    assert!((hit.distance - 2.).abs() < 1e-6);
    assert_eq!(hit.position, [0., 0., -2.]);
    assert_eq!(
        scene
            .project_overlay([0., 0., -2.], LogicalRect::from_xywh(20., 10., 100., 100.))
            .unwrap(),
        Some(LogicalPoint::new(70., 60.))
    );
    assert_eq!(
        scene
            .project_overlay([0., 0., 2.], LogicalRect::from_xywh(20., 10., 100., 100.))
            .unwrap(),
        None
    );
    let outside = gate
        .request(
            stamp(),
            LogicalPoint::new(100., 50.),
            LogicalSize::new(100., 100.),
        )
        .unwrap();
    assert_eq!(
        gate.accept(scene.pick(outside).unwrap(), stamp()),
        Some(None)
    );
}
#[test]
fn delayed_out_of_order_cancelled_foreign_and_changed_revision_picks_never_select() {
    let scene = scene();
    let mut gate = PickTracker::new().unwrap();
    let old = gate
        .request(
            stamp(),
            LogicalPoint::new(50., 50.),
            LogicalSize::new(100., 100.),
        )
        .unwrap();
    let new = gate
        .request(
            stamp(),
            LogicalPoint::new(50., 50.),
            LogicalSize::new(100., 100.),
        )
        .unwrap();
    assert!(gate.accept(scene.pick(old).unwrap(), stamp()).is_none());
    for changed in [
        PickStamp {
            scene: Revision::from_raw(4),
            ..stamp()
        },
        PickStamp {
            camera: Revision::from_raw(4),
            ..stamp()
        },
        PickStamp {
            viewport: Revision::from_raw(4),
            ..stamp()
        },
    ] {
        assert!(gate.accept(scene.pick(new).unwrap(), changed).is_none());
    }
    let result = scene.pick(new).unwrap();
    assert!(gate.accept(result.clone(), stamp()).is_some());
    assert!(gate.accept(result, stamp()).is_none());
    let request = gate
        .request(
            stamp(),
            LogicalPoint::new(50., 50.),
            LogicalSize::new(100., 100.),
        )
        .unwrap();
    let result = scene.pick(request).unwrap();
    gate.cancel();
    assert!(gate.accept(result.clone(), stamp()).is_none());
    let mut replacement = PickTracker::new().unwrap();
    replacement
        .request(
            stamp(),
            LogicalPoint::new(50., 50.),
            LogicalSize::new(100., 100.),
        )
        .unwrap();
    assert!(replacement.accept(result, stamp()).is_none());
}
#[test]
fn named_commands_require_current_revision_and_declared_action_not_pointer_hit() {
    let scene = scene();
    let catalog = SceneCatalog::new(
        &scene,
        vec![NamedObject {
            id: 1,
            name: "rear mesh".into(),
            actions: vec![SceneAction::Select],
        }],
    )
    .unwrap();
    assert_eq!(
        catalog
            .command("rear mesh", SceneAction::Select, scene.revision())
            .unwrap()
            .object,
        1
    );
    assert!(
        catalog
            .command("rear mesh", SceneAction::Frame, scene.revision())
            .is_err()
    );
    assert!(
        catalog
            .command("rear mesh", SceneAction::Select, Revision::INITIAL)
            .is_err()
    );
    assert!(
        SceneCatalog::new(
            &scene,
            vec![NamedObject {
                id: 99,
                name: "gone".into(),
                actions: vec![]
            }]
        )
        .is_err()
    );
}

#[test]
fn worker_completion_is_rejected_after_a_newer_pointer_request() {
    let scene = scene();
    let mut gate = PickTracker::new().unwrap();
    let old = gate
        .request(
            stamp(),
            LogicalPoint::new(50., 50.),
            LogicalSize::new(100., 100.),
        )
        .unwrap();
    let worker = std::thread::spawn(move || scene.pick(old).unwrap());
    gate.request(
        stamp(),
        LogicalPoint::new(0., 0.),
        LogicalSize::new(100., 100.),
    )
    .unwrap();
    assert!(gate.accept(worker.join().unwrap(), stamp()).is_none());
}

#[test]
fn projected_gizmo_uses_normal_runtime_hit_order_and_capture_outside_viewport() {
    use view_core::{
        Description, Dirty, Modifiers, PointerButton, PointerEvent, PointerPhase,
        RoutedPointerAction, Runtime,
    };
    let anchor = scene()
        .project_overlay([0., 0., -2.], LogicalRect::from_xywh(0., 0., 100., 100.))
        .unwrap()
        .unwrap();
    let mut ui = Runtime::<(), ()>::new(8).unwrap();
    let (window, root) = ui
        .open_window(
            Description::new("viewport", (), |_, _, _| Dirty::NONE)
                .layout(LogicalRect::from_xywh(0., 0., 100., 100.)),
        )
        .unwrap();
    let gizmo = ui
        .mount(
            root,
            Description::new("gizmo", (), |_, _, _| Dirty::NONE).layout(LogicalRect::from_xywh(
                anchor.x - 4.,
                anchor.y - 4.,
                8.,
                8.,
            )),
        )
        .unwrap();
    let down = PointerEvent {
        phase: PointerPhase::Down,
        position: anchor,
        button: Some(PointerButton::Primary),
        modifiers: Modifiers::default(),
        timestamp: 0,
    };
    let routed = ui.route_pointer(window, down, &mut ()).unwrap();
    assert!(
        routed
            .iter()
            .any(|a| matches!(a, RoutedPointerAction::Down { target, .. } if *target==gizmo))
    );
    ui.capture_pointer(window, gizmo).unwrap();
    let moved = ui
        .route_pointer(
            window,
            PointerEvent {
                phase: PointerPhase::Move,
                position: LogicalPoint::new(200., 200.),
                ..down
            },
            &mut (),
        )
        .unwrap();
    assert!(
        moved
            .iter()
            .any(|a| matches!(a,RoutedPointerAction::Move{target,..} if *target==gizmo))
    );
    assert!(
        ui.cancel_pointer(window)
            .unwrap()
            .contains(&RoutedPointerAction::Cancel(gizmo))
    );
}
