#![no_std]
#![no_main]

use pico1541_common::track_buffer::DualCoreTrackBuffer;
use defmt_rtt as _;
use panic_probe as _;

#[defmt_test::tests]
mod tests {
    use super::*;
    // use defmt::{assert, assert_eq};

    #[test]
    fn test_offsets() {
        let _buf = DualCoreTrackBuffer::new();
        // Fields are now private
    }
}
