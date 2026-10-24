use view_wgpu::*;

#[test]
fn pass_graph_requires_declared_transitive_dependencies_and_single_writers() {
    let a = PassDeclaration {
        id: 1,
        after: vec![],
        reads: vec![ResourceId(0)],
        writes: vec![ResourceId(1)],
    };
    let b = PassDeclaration {
        id: 2,
        after: vec![1],
        reads: vec![ResourceId(1)],
        writes: vec![ResourceId(2)],
    };
    let mut c = PassDeclaration {
        id: 3,
        after: vec![2],
        reads: vec![ResourceId(1), ResourceId(2)],
        writes: vec![ResourceId(3)],
    };
    assert_eq!(
        validate_passes(&[c.clone(), b.clone(), a.clone()], &[ResourceId(0)]).unwrap(),
        [1, 2, 3]
    );
    c.after.clear();
    assert!(validate_passes(&[a.clone(), b.clone(), c], &[ResourceId(0)]).is_err());
    let mut cycle = a.clone();
    cycle.after = vec![2];
    assert!(validate_passes(&[cycle, b.clone()], &[ResourceId(0)]).is_err());
    let mut duplicate = b.clone();
    duplicate.writes = vec![ResourceId(1)];
    assert!(validate_passes(&[a.clone(), duplicate], &[ResourceId(0)]).is_err());
    assert!(validate_passes(&[a.clone(), b], &[]).is_err());
    assert!(validate_passes(&[a.clone(), a], &[ResourceId(0)]).is_err());
}
#[test]
fn unchanged_resize_does_not_reconfigure_but_resume_still_does() {
    let mut state = SurfaceState::new([100, 80]);
    state.resize([100, 80]);
    assert_eq!(state.apply(), Some([100, 80]));
    for _ in 0..1000 {
        state.resize([100, 80]);
        assert_eq!(state.apply(), None);
    }
    state.resize([120, 90]);
    state.resize([120, 90]);
    assert_eq!(state.apply(), Some([120, 90]));
    state.set_occluded(true);
    state.set_occluded(false);
    state.resize([120, 90]);
    assert_eq!(state.apply(), Some([120, 90]));
}
#[test]
fn resize_coalesces_zero_and_occlusion_suspend() {
    let mut state = SurfaceState::new([10, 10]);
    assert_eq!(state.apply(), Some([10, 10]));
    state.resize([20, 20]);
    state.resize([30, 40]);
    assert_eq!(state.apply(), Some([30, 40]));
    assert_eq!(state.apply(), None);
    state.resize([0, 40]);
    assert_eq!(state.apply(), None);
    assert!(state.suspended());
    state.resize([50, 50]);
    state.set_occluded(true);
    assert_eq!(state.apply(), None);
    assert!(state.suspended());
    state.set_occluded(false);
    assert!(!state.suspended());
    assert_eq!(state.size(), [50, 50]);
}
#[test]
fn direct_requires_resolved_sdr_nonisolated_attachment() {
    let contract = AttachmentContract {
        format: wgpu::TextureFormat::Rgba8UnormSrgb,
        size: [32, 32],
        sample_count: 1,
        isolated: false,
    };
    assert_eq!(
        contract.negotiate(ContributionMode::Direct).unwrap(),
        ContributionMode::Direct
    );
    assert_eq!(
        AttachmentContract {
            isolated: true,
            ..contract
        }
        .negotiate(ContributionMode::Direct)
        .unwrap(),
        ContributionMode::Offscreen
    );
    assert!(
        AttachmentContract {
            sample_count: 4,
            ..contract
        }
        .negotiate(ContributionMode::Direct)
        .is_err()
    );
    assert!(
        AttachmentContract {
            size: [0, 32],
            ..contract
        }
        .negotiate(ContributionMode::Direct)
        .is_err()
    );
}
#[test]
fn sdr_transfer_is_linear_before_premultiplication() {
    let c = Color([0.5, 1.0, 0.0, 0.5]).linear_premultiplied().unwrap();
    assert!((c[0] - 0.10702057).abs() < 1e-6);
    assert_eq!(&c[1..], &[0.5, 0.0, 0.5]);
    assert!(
        Color([f32::NAN, 0.0, 0.0, 1.0])
            .linear_premultiplied()
            .is_err()
    );
}

#[test]
fn editor_visuals_keep_selection_text_preedit_caret_order_and_font_identity() {
    use std::sync::Arc;
    use view_core::{LogicalPoint, Revision, ScaleFactor};
    use view_text::{
        Affinity, Caret, CosmicBackend, LayoutRequest, Selection, TextBackend, TextIndex,
        TextRevision, TextSnapshot, Wrap,
    };
    let font = include_bytes!("../../../tests/fixtures/fonts/NotoSans-Regular.ttf").to_vec();
    let mut backend = CosmicBackend::from_fonts([font.clone()], 2).unwrap();
    let request = LayoutRequest {
        text: TextSnapshot::new("abc", TextRevision::INITIAL),
        font_family: "Noto Sans".into(),
        font_size: 20.0,
        line_height: 26.0,
        max_width: None,
        wrap: Wrap::None,
        scale: ScaleFactor::ONE,
        font_revision: Revision::INITIAL,
    };
    let layout = Arc::new(backend.shape(&request).unwrap());
    let visuals = TextVisuals {
        layout: layout.clone(),
        origin: LogicalPoint::new(10.0, 20.0),
        selection: Some(Selection {
            anchor: TextIndex(0),
            focus: TextIndex(1),
        }),
        caret: Some(Caret {
            index: TextIndex(3),
            affinity: Affinity::Upstream,
        }),
        preedit: Some(Selection {
            anchor: TextIndex(1),
            focus: TextIndex(2),
        }),
        foreground: Color::WHITE,
        selection_color: Color([0.0, 0.0, 1.0, 0.5]),
        clip: None,
    };
    let paint = visuals.paint().unwrap();
    assert_eq!(paint.len(), 4);
    assert!(matches!(
        &paint[0],
        Paint::Draw(Draw {
            primitive: Primitive::Rect(_),
            ..
        })
    ));
    assert!(matches!(
        &paint[1],
        Paint::Draw(Draw {
            primitive: Primitive::Text { .. },
            ..
        })
    ));
    let Paint::Draw(Draw {
        primitive: Primitive::Rect(underline),
        ..
    }) = &paint[2]
    else {
        panic!("preedit underline missing")
    };
    assert_eq!(underline.size.height, 1.0);
    assert_eq!(underline.origin.y, 45.0);
    backend.reload_fonts([font]).unwrap();
    let new = backend
        .shape(&LayoutRequest {
            font_revision: backend.font_revision(),
            ..request
        })
        .unwrap();
    assert!(!layout.shares_font_collection(&new));
    assert!(layout.shares_font_collection(&layout));
    assert!(
        TextVisuals {
            caret: Some(Caret {
                index: TextIndex(100),
                affinity: Affinity::Upstream
            }),
            ..visuals
        }
        .paint()
        .is_err()
    );
}
