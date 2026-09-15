/// Utility functions for bit manipulation and alignment.
#[inline(always)]
pub fn bit_align_u32(value: u32, from_bit: usize, to_bit: usize) -> u32 {
    if from_bit > to_bit {
        value >> (from_bit - to_bit)
    } else if from_bit < to_bit {
        value << (to_bit - from_bit)
    } else {
        value
    }
}

/// Aligns a u8 value from one bit position to another.
#[inline(always)]
pub fn bit_align_u8(value: u8, from_bit: usize, to_bit: usize) -> u8 {
    if from_bit > to_bit {
        value >> (from_bit - to_bit)
    } else if from_bit < to_bit {
        value << (to_bit - from_bit)
    } else {
        value
    }
}
