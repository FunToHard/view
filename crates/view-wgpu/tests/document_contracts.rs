use view_wgpu::document::{
    ImageBudget, ImageDocument, ImageError, ImageRect, PenSample, SdrEffect,
};

fn budget() -> ImageBudget {
    ImageBudget {
        max_pixels: 4096,
        max_tiles: 16,
        max_history: 8,
        max_history_bytes: 16 * 1024,
        max_export_bytes: 64 * 1024,
    }
}

#[test]
fn effects_are_clipped_atomic_and_one_undo_restores_exact_pixels() {
    let mut image = ImageDocument::new(20, 18, budget()).unwrap();
    image.set_pixel(1, 1, [200, 20, 10, 128]).unwrap();
    image.set_pixel(19, 17, [8, 80, 160, 255]).unwrap();
    let before_a = image.pixel(1, 1);
    let before_b = image.pixel(19, 17);
    image
        .apply(
            ImageRect {
                x: 0,
                y: 0,
                width: 10,
                height: 10,
            },
            SdrEffect::Grayscale,
        )
        .unwrap();
    assert_eq!(image.pixel(1, 1), Some([58, 58, 58, 128]));
    assert_eq!(image.pixel(19, 17), before_b);
    assert!(image.undo());
    assert_eq!(image.pixel(1, 1), before_a);
    assert_eq!(image.pixel(19, 17), before_b);
}

#[test]
fn effect_history_exhaustion_leaves_all_tiles_and_history_unchanged() {
    let mut limits = budget();
    limits.max_history_bytes = 0;
    let mut image = ImageDocument::new(32, 16, limits).unwrap();
    image.set_pixel(1, 1, [1, 2, 3, 255]).unwrap();
    let before = image.pixel(1, 1);
    let tile_count = image.allocated_tiles();
    let history_count = image.undo_len();
    assert_eq!(
        image.apply(
            ImageRect {
                x: 0,
                y: 0,
                width: 32,
                height: 16,
            },
            SdrEffect::Invert,
        ),
        Err(ImageError::HistoryBudget)
    );
    assert_eq!(image.pixel(1, 1), before);
    assert_eq!(image.allocated_tiles(), tile_count);
    assert_eq!(image.undo_len(), history_count);
}

#[test]
fn deterministic_bmp_v4_and_raw_export_round_trip_pixels() {
    let mut image = ImageDocument::new(2, 2, budget()).unwrap();
    image.set_pixel(0, 0, [1, 2, 3, 4]).unwrap();
    image.set_pixel(1, 0, [10, 20, 30, 40]).unwrap();
    image.set_pixel(0, 1, [90, 80, 70, 60]).unwrap();
    let raw = image.encode_rgba8().unwrap();
    assert_eq!(&raw[..16], b"VWRGBA\0\x01\x02\0\0\0\x02\0\0\0");
    assert_eq!(
        &raw[16..],
        &[1, 2, 3, 4, 10, 20, 30, 40, 90, 80, 70, 60, 0, 0, 0, 0]
    );

    let bmp = image.encode_bmp().unwrap();
    assert_eq!(&bmp[..2], b"BM");
    assert_eq!(u32::from_le_bytes(bmp[10..14].try_into().unwrap()), 122);
    assert_eq!(u32::from_le_bytes(bmp[14..18].try_into().unwrap()), 108);
    assert_eq!(u32::from_le_bytes(bmp[30..34].try_into().unwrap()), 3);
    assert_eq!(i32::from_le_bytes(bmp[22..26].try_into().unwrap()), -2);
    assert_eq!(
        &bmp[122..],
        &[3, 2, 1, 4, 30, 20, 10, 40, 70, 80, 90, 60, 0, 0, 0, 0]
    );
    assert_eq!(&bmp, image.encode_bmp().unwrap().as_slice());
}

#[test]
fn budgets_dimensions_and_pen_sample_validation_are_explicit() {
    assert!(matches!(
        ImageDocument::new(0, 1, budget()),
        Err(ImageError::InvalidDimensions)
    ));
    let mut limits = budget();
    limits.max_pixels = 2;
    assert!(matches!(
        ImageDocument::new(2, 2, limits),
        Err(ImageError::PixelBudget)
    ));
    let sample = PenSample {
        x: 3.0,
        y: -4.0,
        pressure: 0.5,
        tilt_x_degrees: -90.0,
        tilt_y_degrees: 90.0,
    };
    assert_eq!(sample.validate(), Ok(sample));
    assert_eq!(
        PenSample {
            pressure: f32::NAN,
            ..sample
        }
        .validate(),
        Err(ImageError::InvalidSample)
    );
}
