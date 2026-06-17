use std::sync::atomic::Ordering;

fn main() {
    let length = 7108;
    let mut track_byte_pos_ix = 7106;
    let track_byte_size = 7108;

    for _ in 0..5 {
        if track_byte_size > 0 && track_byte_pos_ix >= track_byte_size {
            track_byte_pos_ix = 0;
        }
        let current = track_byte_pos_ix;
        track_byte_pos_ix += 1;
        println!("current: {}, ret: {}", current, current % length);
    }
}
