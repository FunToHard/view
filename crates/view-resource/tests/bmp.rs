#![cfg(feature = "image-bmp")]
use view_resource::assets::*;
#[test]
fn bmp_rows_padding_channels_and_decode_limits_are_independently_checked() {
    let mut bytes = vec![0; 62];
    bytes[..2].copy_from_slice(b"BM");
    bytes[10..14].copy_from_slice(&54u32.to_le_bytes());
    bytes[14..18].copy_from_slice(&40u32.to_le_bytes());
    bytes[18..22].copy_from_slice(&1u32.to_le_bytes());
    bytes[22..26].copy_from_slice(&2i32.to_le_bytes());
    bytes[26] = 1;
    bytes[28] = 24;
    bytes[54..62].copy_from_slice(&[255, 0, 0, 0, 0, 0, 255, 0]);
    assert_eq!(
        decode_bmp(&bytes, 8).unwrap().pixels(),
        &[255, 0, 0, 255, 0, 0, 255, 255]
    );
    assert_eq!(decode_bmp(&bytes, 7), Err(AssetError::Limit));
    assert_eq!(decode_bmp(&bytes[..61], 8), Err(AssetError::Invalid));
    bytes[30] = 1;
    assert_eq!(decode_bmp(&bytes, 8), Err(AssetError::Unsupported));
}
