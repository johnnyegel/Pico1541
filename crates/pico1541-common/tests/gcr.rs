use pico1541_common::gcr::{encode_gcr_block, decode_gcr_block};

#[test]
fn test_gcr_encode_decode() {
    let original = [0x12, 0x34, 0x56, 0x78];
    let mut encoded = [0u8; 5];
    let mut decoded = [0u8; 4];

    encode_gcr_block(&original, &mut encoded);
    decode_gcr_block(&encoded, &mut decoded);

    assert_eq!(original, decoded);
}

#[test]
fn test_gcr_all_nibbles() {
    for i in 0..16 {
        let val = i | (i << 4);
        let original = [val, val, val, val];
        let mut encoded = [0u8; 5];
        let mut decoded = [0u8; 4];

        encode_gcr_block(&original, &mut encoded);
        decode_gcr_block(&encoded, &mut decoded);

        assert_eq!(original, decoded);
    }
}
