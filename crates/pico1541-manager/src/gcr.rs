pub const GCR_TABLE: [u8; 16] = [
    0x0A, 0x0B, 0x09, 0x0D, 0x0E, 0x0F, 0x12, 0x13,
    0x19, 0x1B, 0x1A, 0x1E, 0x1D, 0x1F, 0x16, 0x17,
];

pub fn encode_gcr_block(src: &[u8; 4], dst: &mut [u8; 5]) {
    // 8 nibbles (4 bits each)
    let mut n = [0u8; 8];
    for i in 0..4 {
        n[i * 2] = GCR_TABLE[(src[i] >> 4) as usize];
        n[i * 2 + 1] = GCR_TABLE[(src[i] & 0x0F) as usize];
    }

    // Pack 8 * 5 bits = 40 bits into 5 bytes
    dst[0] = (n[0] << 3) | (n[1] >> 2);
    dst[1] = (n[1] << 6) | (n[2] << 1) | (n[3] >> 4);
    dst[2] = (n[3] << 4) | (n[4] >> 1);
    dst[3] = (n[4] << 7) | (n[5] << 2) | (n[6] >> 3);
    dst[4] = (n[6] << 5) | n[7];
}

pub fn decode_gcr_block(src: &[u8; 5], dst: &mut [u8; 4]) {
    // This is the reverse of encode_gcr_block, useful for reading back modified tracks.
    // For now we prioritize encoding for D64 loading.
    let mut n = [0u8; 8];
    n[0] = src[0] >> 3;
    n[1] = ((src[0] & 0x07) << 2) | (src[1] >> 6);
    n[2] = (src[1] >> 1) & 0x1F;
    n[3] = ((src[1] & 0x01) << 4) | (src[2] >> 4);
    n[4] = ((src[2] & 0x0F) << 1) | (src[3] >> 7);
    n[5] = (src[3] >> 2) & 0x1F;
    n[6] = ((src[3] & 0x03) << 3) | (src[4] >> 5);
    n[7] = src[4] & 0x1F;

    for i in 0..4 {
        let high = reverse_gcr(n[i * 2]);
        let low = reverse_gcr(n[i * 2 + 1]);
        dst[i] = (high << 4) | low;
    }
}

fn reverse_gcr(g: u8) -> u8 {
    for (i, &val) in GCR_TABLE.iter().enumerate() {
        if val == g {
            return i as u8;
        }
    }
    0 // Should not happen with valid GCR
}
