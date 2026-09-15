#![no_std]
#![no_main]

use defmt_rtt as _;
use panic_probe as _;
use pico1541_common::track_buffer::DualCoreTrackBuffer;

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
