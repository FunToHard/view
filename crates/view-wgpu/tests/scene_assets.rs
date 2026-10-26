#![cfg(feature = "scene")]
use std::sync::Arc;
use view_wgpu::scene::*;

#[test]
fn obj_quad_fan_and_negative_indices_produce_valid_mesh() {
    let mesh = import_obj_positions(
        "# positions-only fixture\nv 0 0 0\nv 1 0 0\nv 1 1 0\nv 0 1 0\nf -4 -3 -2 -1\n",
        ObjLimits::default(),
    )
    .unwrap();
    assert_eq!(
        mesh.positions(),
        &[[0., 0., 0.], [1., 0., 0.], [1., 1., 0.], [0., 1., 0.]]
    );
    assert_eq!(mesh.indices(), &[0, 1, 2, 0, 2, 3]);
}

#[test]
fn obj_import_rejects_invalid_indices_nonfinite_data_and_unsupported_profiles() {
    assert!(matches!(
        import_obj_positions("v 0 0 0\nf 0 1 1\n", ObjLimits::default()),
        Err(ObjError::InvalidIndex { line: 2 })
    ));
    assert!(matches!(
        import_obj_positions("v NaN 0 0\nf 1 1 1\n", ObjLimits::default()),
        Err(ObjError::NonFinitePosition { line: 1 })
    ));
    assert!(matches!(
        import_obj_positions("vn 0 0 1\n", ObjLimits::default()),
        Err(ObjError::Unsupported {
            feature: "normals",
            ..
        })
    ));
    assert!(matches!(
        import_obj_positions("mtllib scene.mtl\n", ObjLimits::default()),
        Err(ObjError::Unsupported {
            feature: "materials",
            ..
        })
    ));
    assert!(matches!(
        import_obj_positions(
            "v 0 0 0\nv 1 0 0\nv 0 1 0\nf 1/1 2/2 3/3\n",
            ObjLimits::default()
        ),
        Err(ObjError::Unsupported {
            feature: "texture coordinates",
            ..
        })
    ));
}

#[test]
fn obj_resource_caps_are_applied_before_mesh_creation() {
    let limits = ObjLimits {
        max_bytes: 7,
        ..Default::default()
    };
    assert_eq!(
        import_obj_positions("v 0 0 0\n", limits).unwrap_err(),
        ObjError::LimitExceeded("bytes")
    );
    let limits = ObjLimits {
        max_positions: 2,
        ..Default::default()
    };
    assert_eq!(
        import_obj_positions("v 0 0 0\nv 1 0 0\nv 0 1 0\nf 1 2 3\n", limits).unwrap_err(),
        ObjError::LimitExceeded("positions")
    );
    let limits = ObjLimits {
        max_triangles: 1,
        ..Default::default()
    };
    assert_eq!(
        import_obj_positions("v 0 0 0\nv 1 0 0\nv 1 1 0\nv 0 1 0\nf 1 2 3 4\n", limits)
            .unwrap_err(),
        ObjError::LimitExceeded("triangles")
    );
}

#[test]
fn animation_validates_keyframes_clamps_and_interpolates() {
    assert!(TranslationAnimation::new(vec![]).is_err());
    assert!(
        TranslationAnimation::new(vec![
            TranslationKeyframe {
                time_seconds: 0.0,
                translation: [0.; 3]
            },
            TranslationKeyframe {
                time_seconds: 0.0,
                translation: [1.; 3]
            }
        ])
        .is_err()
    );
    assert!(
        TranslationAnimation::new(vec![TranslationKeyframe {
            time_seconds: f64::NAN,
            translation: [0.; 3]
        }])
        .is_err()
    );
    let track = TranslationAnimation::new(vec![
        TranslationKeyframe {
            time_seconds: 1.0,
            translation: [0., 2., 4.],
        },
        TranslationKeyframe {
            time_seconds: 3.0,
            translation: [2., 4., 8.],
        },
    ])
    .unwrap();
    assert_eq!(track.sample(0.0).unwrap(), [0., 2., 4.]);
    assert_eq!(track.sample(2.0).unwrap(), [1., 3., 6.]);
    assert_eq!(track.sample(9.0).unwrap(), [2., 4., 8.]);
    assert!(track.sample(f64::INFINITY).is_err());
}

#[test]
fn animation_updates_scene_object_translation_and_preserves_scale() {
    let track = TranslationAnimation::new(vec![
        TranslationKeyframe {
            time_seconds: 0.,
            translation: [-2., 3., -4.],
        },
        TranslationKeyframe {
            time_seconds: 2.,
            translation: [2., 7., -8.],
        },
    ])
    .unwrap();
    let mesh = Arc::new(
        Mesh3D::new(
            vec![[0., 0., 0.], [1., 0., 0.], [0., 1., 0.]],
            vec![0, 1, 2],
        )
        .unwrap(),
    );
    let mut object = SceneObject {
        id: 1,
        mesh,
        transform: Transform3D::scale([2., 3., 4.])
            .unwrap()
            .then(Transform3D::translation([9., 9., 9.]).unwrap())
            .unwrap(),
        material: BasicMaterial::new([1., 0., 0.]).unwrap(),
    };
    object.apply_translation_animation(&track, 1.).unwrap();
    assert_eq!(object.transform.translation_part(), [0., 5., -6.]);
    let transformed = object
        .transform
        .matrix()
        .transform([1., 1., 1., 1.])
        .unwrap();
    assert_eq!(transformed, [2., 8., -2., 1.]);
}

#[test]
fn mesh_derives_finite_area_weighted_normals_from_triangle_winding() {
    let forward = Mesh3D::new(
        vec![[0., 0., 0.], [2., 0., 0.], [0., 2., 0.]],
        vec![0, 1, 2],
    )
    .unwrap();
    assert_eq!(forward.normals(), &[[0., 0., 1.]; 3]);
    let reversed = Mesh3D::new(
        vec![[0., 0., 0.], [2., 0., 0.], [0., 2., 0.]],
        vec![0, 2, 1],
    )
    .unwrap();
    assert_eq!(reversed.normals(), &[[0., 0., -1.]; 3]);

    let degenerate = Mesh3D::new(vec![[0.; 3]; 3], vec![0, 1, 2]).unwrap();
    assert!(
        degenerate
            .normals()
            .iter()
            .flatten()
            .all(|value| value.is_finite())
    );
}

#[test]
fn directional_light_and_metallic_roughness_inputs_are_validated() {
    assert!(DirectionalLight::new([0.; 3], [1.; 3], 1.).is_err());
    assert!(DirectionalLight::new([0., 0., 1.], [f32::NAN, 1., 1.], 1.).is_err());
    assert!(DirectionalLight::new([0., 0., 1.], [1.; 3], f32::INFINITY).is_err());
    let light = DirectionalLight::new([0., 0., 4.], [1.; 3], 2.).unwrap();
    assert_eq!(light.direction(), [0., 0., 1.]);
    assert_eq!(light.intensity(), 2.);
    assert!(BasicMaterial::metallic_roughness([1.; 3], -0.1, 0.5, light).is_err());
    assert!(BasicMaterial::metallic_roughness([1.; 3], 0.5, 0.0, light).is_err());
    assert!(BasicMaterial::metallic_roughness([1.; 3], 0.5, 0.5, light).is_ok());
}
