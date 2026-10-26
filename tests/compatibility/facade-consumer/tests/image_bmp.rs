#![cfg(feature = "image-bmp")]

use view::resources::assets::{AssetError, decode_bmp};

#[test]
fn standalone_codec_feature_decodes_bgr_pixels_and_enforces_output_budget() {
    // Two 24-bit BGR pixels followed by BMP's four-byte row padding.
    let mut bmp = vec![0_u8; 62];
    bmp[..2].copy_from_slice(b"BM");
    bmp[2..6].copy_from_slice(&62_u32.to_le_bytes());
    bmp[10..14].copy_from_slice(&54_u32.to_le_bytes());
    bmp[14..18].copy_from_slice(&40_u32.to_le_bytes());
    bmp[18..22].copy_from_slice(&2_i32.to_le_bytes());
    bmp[22..26].copy_from_slice(&1_i32.to_le_bytes());
    bmp[26..28].copy_from_slice(&1_u16.to_le_bytes());
    bmp[28..30].copy_from_slice(&24_u16.to_le_bytes());
    bmp[54..60].copy_from_slice(&[0, 0, 255, 0, 255, 0]);

    let image = decode_bmp(&bmp, 8).expect("optional facade codec");
    assert_eq!((image.width(), image.height()), (2, 1));
    assert_eq!(image.pixels(), &[255, 0, 0, 255, 0, 255, 0, 255]);
    assert_eq!(decode_bmp(&bmp, 7), Err(AssetError::Limit));
}
