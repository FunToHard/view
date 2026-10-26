use view_wgpu::document::{
    AlphaMask, ImageBudget, ImageDocument, ImageError, ImageOperationBudget, ImageRect,
    MAX_BLUR_RADIUS, RgbaImage,
};

fn limits() -> ImageBudget {
    ImageBudget {
        max_pixels: 4096,
        max_tiles: 16,
        max_history: 64,
        max_history_bytes: 128 * 1024,
        max_export_bytes: 32 * 1024,
    }
}

fn rect(x: u32, y: u32, width: u32, height: u32) -> ImageRect {
    ImageRect {
        x,
        y,
        width,
        height,
    }
}

fn source(width: u32, height: u32, pixels: &[u8]) -> RgbaImage<'_> {
    RgbaImage {
        width,
        height,
        pixels,
    }
}

fn mask(width: u32, height: u32, coverage: &[u8]) -> AlphaMask<'_> {
    AlphaMask {
        width,
        height,
        coverage,
    }
}

#[test]
fn nonuniform_mask_is_clipped_in_source_coordinates_and_undo_is_one_transaction() {
    let mut image = ImageDocument::new(3, 2, limits()).unwrap();
    let original = image.encode_rgba8().unwrap();
    let colors = [
        255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 255, 255, 0, 255, 255, 0, 255, 255, 0, 255,
        255, 255,
    ];
    image
        .composite(
            source(3, 2, &colors),
            (-1, 0),
            Some(mask(3, 2, &[255, 128, 0, 0, 255, 64])),
            ImageOperationBudget::default(),
        )
        .unwrap();
    assert_eq!(image.pixel(0, 0), Some([0, 255, 0, 128]));
    assert_eq!(image.pixel(1, 0), Some([0, 0, 0, 0]));
    assert_eq!(image.pixel(0, 1), Some([255, 0, 255, 255]));
    assert_eq!(image.pixel(1, 1), Some([0, 255, 255, 64]));
    assert_eq!(image.undo_len(), 1);
    assert!(image.undo());
    assert_eq!(image.encode_rgba8().unwrap(), original);
    assert_eq!(image.allocated_tiles(), 0);
}

#[test]
fn source_over_uses_linear_light_and_multiplies_alpha_by_coverage() {
    let mut image = ImageDocument::new(2, 1, limits()).unwrap();
    image.set_pixel(0, 0, [0, 0, 0, 255]).unwrap();
    image.set_pixel(1, 0, [0, 0, 255, 128]).unwrap();
    let original = image.encode_rgba8().unwrap();
    image
        .composite(
            source(2, 1, &[255, 255, 255, 255, 255, 0, 0, 128]),
            (0, 0),
            Some(mask(2, 1, &[128, 255])),
            ImageOperationBudget::default(),
        )
        .unwrap();
    // Half white over opaque black is approximately sRGB 188, not 128.
    assert_eq!(image.pixel(0, 0), Some([188, 188, 188, 255]));
    assert_eq!(image.pixel(1, 0), Some([213, 0, 156, 192]));
    assert!(image.undo());
    assert_eq!(image.encode_rgba8().unwrap(), original);
}

#[test]
fn blur_ignores_hidden_transparent_color_and_reads_past_output_rectangle() {
    let mut image = ImageDocument::new(5, 1, limits()).unwrap();
    image.set_pixel(0, 0, [0, 0, 255, 0]).unwrap();
    image.set_pixel(1, 0, [255, 0, 0, 255]).unwrap();
    image.set_pixel(2, 0, [0, 0, 255, 0]).unwrap();
    let original = image.encode_rgba8().unwrap();
    let before_history = image.undo_len();
    image
        .box_blur(rect(2, 0, 1, 1), 1, None, ImageOperationBudget::default())
        .unwrap();
    assert_eq!(image.pixel(2, 0), Some([255, 0, 0, 85]));
    assert_eq!(image.pixel(1, 0), Some([255, 0, 0, 255]));
    assert_eq!(image.pixel(0, 0), Some([0, 0, 255, 0]));
    assert_eq!(image.undo_len(), before_history + 1);
    assert!(image.undo());
    assert_eq!(image.encode_rgba8().unwrap(), original);
}

#[test]
fn blur_has_separable_two_dimensional_support_and_replication_at_document_edges() {
    let mut image = ImageDocument::new(3, 3, limits()).unwrap();
    image.set_pixel(1, 1, [255, 255, 255, 255]).unwrap();
    image
        .box_blur(rect(0, 0, 3, 3), 1, None, ImageOperationBudget::default())
        .unwrap();
    for y in 0..3 {
        for x in 0..3 {
            // A centered impulse covers every 3x3 sample window once.
            assert_eq!(image.pixel(x, y), Some([255, 255, 255, 28]));
        }
    }
    let mut edge = ImageDocument::new(3, 1, limits()).unwrap();
    edge.set_pixel(0, 0, [255, 0, 0, 255]).unwrap();
    edge.box_blur(rect(0, 0, 3, 1), 1, None, ImageOperationBudget::default())
        .unwrap();
    assert_eq!(edge.pixel(0, 0), Some([255, 0, 0, 170]));
    assert_eq!(edge.pixel(1, 0), Some([255, 0, 0, 85]));
    assert_eq!(edge.pixel(2, 0), Some([0; 4]));
}

#[test]
fn blur_mask_interpolates_premultiplied_results_and_preserves_excluded_bytes() {
    let mut image = ImageDocument::new(3, 1, limits()).unwrap();
    image.set_pixel(0, 0, [0, 255, 0, 0]).unwrap();
    image.set_pixel(1, 0, [255, 0, 0, 255]).unwrap();
    image
        .box_blur(
            rect(0, 0, 5, 1),
            1,
            Some(mask(5, 1, &[0, 128, 255, 0, 0])),
            ImageOperationBudget::default(),
        )
        .unwrap();
    assert_eq!(image.pixel(0, 0), Some([0, 255, 0, 0]));
    assert_eq!(image.pixel(1, 0), Some([255, 0, 0, 170]));
    assert_eq!(image.pixel(2, 0), Some([255, 0, 0, 85]));
}

#[test]
fn opaque_blur_averages_linear_light_and_cross_tile_undo_restores_sparsity() {
    let mut image = ImageDocument::new(33, 1, limits()).unwrap();
    image.set_pixel(13, 0, [0, 0, 0, 255]).unwrap();
    image.set_pixel(14, 0, [0, 0, 0, 255]).unwrap();
    image.set_pixel(15, 0, [255, 255, 255, 255]).unwrap();
    let original = image.encode_rgba8().unwrap();
    let history = image.undo_len();
    image
        .box_blur(rect(14, 0, 3, 1), 1, None, Default::default())
        .unwrap();
    // Pixel 14 averages two opaque black samples and one opaque white sample.
    assert_eq!(image.pixel(14, 0), Some([156, 156, 156, 255]));
    // The formerly sparse neighboring tile receives premultiplied white.
    assert_eq!(image.pixel(16, 0), Some([255, 255, 255, 85]));
    assert_eq!(image.allocated_tiles(), 2);
    assert_eq!(image.undo_len(), history + 1);
    assert!(image.undo());
    assert_eq!(image.encode_rgba8().unwrap(), original);
    assert_eq!(image.allocated_tiles(), 1);
}

#[test]
fn masked_two_dimensional_blur_matches_direct_neighborhood_reference() {
    let mut image = ImageDocument::new(5, 4, limits()).unwrap();
    let colors = [[255, 0, 0], [0, 255, 0], [0, 0, 255], [255, 255, 255]];
    let mut original = [[0; 4]; 20];
    for y in 0..4 {
        for x in 0..5 {
            let index = y * 5 + x;
            let color = colors[(x + y * 3) % colors.len()];
            let alpha = [0, 64, 128, 255][(x * 3 + y) % 4];
            original[index] = [color[0], color[1], color[2], alpha];
            image
                .set_pixel(x as u32, y as u32, original[index])
                .unwrap();
        }
    }
    let coverage: Vec<u8> = (0..48)
        .map(|index| [0, 64, 128, 255][(index * 3) % 4])
        .collect();
    image
        .box_blur(
            rect(1, 1, 8, 6),
            1,
            Some(mask(8, 6, &coverage)),
            Default::default(),
        )
        .unwrap();
    for y in 0..4 {
        for x in 0..5 {
            let before = original[y * 5 + x];
            let weight = if x >= 1 && y >= 1 {
                f64::from(coverage[(y - 1) * 8 + x - 1]) / 255.0
            } else {
                0.0
            };
            if weight == 0.0 {
                assert_eq!(image.pixel(x as u32, y as u32), Some(before));
                continue;
            }
            // Independent scalar 3x3 reference: fixture channels are only 0 or
            // 255, so their linear values are exactly 0 or 1 without decoding.
            let mut alpha_sum = 0.0;
            let mut color_sum = [0.0; 3];
            for dy in -1..=1 {
                for dx in -1..=1 {
                    let sx = (x as i32 + dx).clamp(0, 4) as usize;
                    let sy = (y as i32 + dy).clamp(0, 3) as usize;
                    let sample = original[sy * 5 + sx];
                    alpha_sum += f64::from(sample[3]);
                    for channel in 0..3 {
                        if sample[channel] == 255 {
                            color_sum[channel] += f64::from(sample[3]);
                        }
                    }
                }
            }
            let alpha = f64::from(before[3]) * (1.0 - weight) + alpha_sum / 9.0 * weight;
            let mut expected = [0; 4];
            expected[3] = alpha.round() as u8;
            for channel in 0..3 {
                let retained = if before[channel] == 255 {
                    f64::from(before[3]) * (1.0 - weight)
                } else {
                    0.0
                };
                let linear = (retained + color_sum[channel] / 9.0 * weight) / alpha;
                let encoded = if linear == 0.0 {
                    0.0
                } else {
                    1.055 * linear.powf(1.0 / 2.4) - 0.055
                };
                expected[channel] = (encoded * 255.0).round() as u8;
            }
            assert_eq!(
                image.pixel(x as u32, y as u32),
                Some(expected),
                "at {x},{y}"
            );
        }
    }
}

#[test]
fn source_scratch_work_history_and_tile_rejections_leave_the_document_unchanged() {
    let mut image = ImageDocument::new(32, 1, limits()).unwrap();
    image.set_pixel(15, 0, [255, 0, 0, 255]).unwrap();
    let original = image.encode_rgba8().unwrap();
    let history = image.undo_len();
    for (operation_budget, expected) in [
        (
            ImageOperationBudget {
                max_source_bytes: 1,
                ..Default::default()
            },
            ImageError::SourceBudget,
        ),
        (
            ImageOperationBudget {
                max_scratch_bytes: 1,
                ..Default::default()
            },
            ImageError::ScratchBudget,
        ),
        (
            ImageOperationBudget {
                max_work_pixels: 1,
                ..Default::default()
            },
            ImageError::WorkBudget,
        ),
    ] {
        assert_eq!(
            image.box_blur(rect(0, 0, 32, 1), 1, None, operation_budget),
            Err(expected.clone())
        );
        assert_eq!(
            image.composite(source(2, 1, &[255; 8]), (15, 0), None, operation_budget),
            Err(expected)
        );
        assert_eq!(image.encode_rgba8().unwrap(), original);
        assert_eq!(image.undo_len(), history);
    }
    let mut no_history = limits();
    no_history.max_history_bytes = 0;
    let mut image = ImageDocument::new(32, 1, no_history).unwrap();
    image.set_pixel(15, 0, [255, 0, 0, 255]).unwrap();
    let original = image.encode_rgba8().unwrap();
    assert_eq!(
        image.box_blur(rect(0, 0, 32, 1), 1, None, Default::default()),
        Err(ImageError::HistoryBudget)
    );
    assert_eq!(
        image.composite(source(1, 1, &[255; 4]), (15, 0), None, Default::default()),
        Err(ImageError::HistoryBudget)
    );
    assert_eq!(image.encode_rgba8().unwrap(), original);
    assert_eq!(image.undo_len(), 1);

    let mut one_tile = limits();
    one_tile.max_tiles = 1;
    let mut image = ImageDocument::new(32, 1, one_tile).unwrap();
    image.set_pixel(15, 0, [255, 0, 0, 255]).unwrap();
    let original = image.encode_rgba8().unwrap();
    assert_eq!(
        image.box_blur(rect(0, 0, 32, 1), 1, None, Default::default()),
        Err(ImageError::TileBudget)
    );
    assert_eq!(
        image.composite(source(2, 1, &[255; 8]), (15, 0), None, Default::default()),
        Err(ImageError::TileBudget)
    );
    assert_eq!(image.encode_rgba8().unwrap(), original);
    assert_eq!(image.undo_len(), 1);
    assert_eq!(image.allocated_tiles(), 1);
}

#[test]
fn malformed_inputs_zero_extents_and_extreme_origins_are_safe() {
    let mut image = ImageDocument::new(1, 1, limits()).unwrap();
    let original = image.encode_rgba8().unwrap();
    for bad_source in [
        source(0, 1, &[]),
        source(1, 1, &[0; 3]),
        source(u32::MAX, u32::MAX, &[]),
    ] {
        assert_eq!(
            image.composite(bad_source, (0, 0), None, Default::default()),
            Err(ImageError::InvalidSource)
        );
    }
    assert_eq!(
        image.composite(
            source(1, 1, &[255; 4]),
            (0, 0),
            Some(mask(1, 1, &[])),
            Default::default()
        ),
        Err(ImageError::InvalidMask)
    );
    assert_eq!(
        image.box_blur(
            rect(0, 0, 1, 1),
            MAX_BLUR_RADIUS + 1,
            None,
            Default::default()
        ),
        Err(ImageError::InvalidEffect)
    );
    for origin in [(i64::MIN, 0), (i64::MAX, 0), (0, i64::MIN), (0, i64::MAX)] {
        image
            .composite(source(1, 1, &[255; 4]), origin, None, Default::default())
            .unwrap();
    }
    for bounds in [
        rect(0, 0, 0, 1),
        rect(0, 0, 1, 0),
        rect(u32::MAX, u32::MAX, u32::MAX, u32::MAX),
    ] {
        image
            .box_blur(bounds, MAX_BLUR_RADIUS, None, Default::default())
            .unwrap();
    }
    image
        .box_blur(rect(0, 0, 1, 1), 0, None, Default::default())
        .unwrap();
    image
        .composite(
            source(1, 1, &[255, 0, 255, 0]),
            (0, 0),
            None,
            Default::default(),
        )
        .unwrap();
    assert_eq!(image.encode_rgba8().unwrap(), original);
    assert_eq!(image.undo_len(), 0);
    assert_eq!(image.allocated_tiles(), 0);

    // A sparse document may describe an enormous extent, but processing cannot
    // allocate a correspondingly enormous intermediate before rejecting it.
    let mut huge = ImageDocument::new(
        u32::MAX,
        u32::MAX,
        ImageBudget {
            max_pixels: usize::MAX,
            ..limits()
        },
    )
    .unwrap();
    assert_eq!(
        huge.box_blur(rect(0, 0, u32::MAX, u32::MAX), 1, None, Default::default()),
        Err(ImageError::SourceBudget)
    );
    assert_eq!(huge.undo_len(), 0);
    assert_eq!(huge.allocated_tiles(), 0);
}

#[test]
fn single_pixel_document_and_maximum_radius_have_defined_behavior() {
    let mut image = ImageDocument::new(1, 1, limits()).unwrap();
    image.set_pixel(0, 0, [32, 64, 96, 128]).unwrap();
    let original = image.encode_rgba8().unwrap();
    let history = image.undo_len();
    image
        .box_blur(
            rect(0, 0, u32::MAX, u32::MAX),
            MAX_BLUR_RADIUS,
            None,
            Default::default(),
        )
        .unwrap();
    assert_eq!(image.encode_rgba8().unwrap(), original);
    assert_eq!(image.undo_len(), history);
}
