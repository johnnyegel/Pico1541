//! Marcus Geelnard LZ77 decompression implementation for Commodore .nbz files.

use std::error::Error;
use std::fmt;

/// Maximum allowable decompressed size (16 MB) to protect against decompression bombs.
pub const MAX_DECOMPRESSED_SIZE: usize = 16 * 1024 * 1024;

/// Errors that can occur during LZ77 decompression.
#[derive(Debug, PartialEq, Eq)]
pub enum DecompressError {
    EmptyInput,
    UnexpectedEof,
    InvalidReference { offset: usize, current_pos: usize },
    VarIntOverflow,
    OutputLimitExceeded,
}

impl fmt::Display for DecompressError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyInput => write!(f, "empty input"),
            Self::UnexpectedEof => write!(f, "unexpected end of file"),
            Self::InvalidReference {
                offset,
                current_pos,
            } => {
                write!(
                    f,
                    "invalid LZ77 reference: offset {} at pos {}",
                    offset, current_pos
                )
            }
            Self::VarIntOverflow => write!(f, "variable-length integer overflow"),
            Self::OutputLimitExceeded => write!(f, "decompressed output size limit exceeded"),
        }
    }
}

impl Error for DecompressError {}

fn read_var_size(buf: &[u8], pos: &mut usize) -> Result<usize, DecompressError> {
    let mut val: usize = 0;
    loop {
        if *pos >= buf.len() {
            return Err(DecompressError::UnexpectedEof);
        }
        let b = buf[*pos];
        *pos += 1;
        let shifted = val
            .checked_shl(7)
            .filter(|&v| (v >> 7) == val)
            .ok_or(DecompressError::VarIntOverflow)?;
        val = shifted | ((b & 0x7F) as usize);
        if (b & 0x80) == 0 {
            break;
        }
    }
    Ok(val)
}

fn decompress_internal(input: &[u8]) -> Result<Vec<u8>, DecompressError> {
    if input.is_empty() {
        return Err(DecompressError::EmptyInput);
    }
    let marker = input[0];
    let mut inpos = 1;
    let mut output = Vec::with_capacity(input.len().saturating_mul(2));

    while inpos < input.len() {
        let symbol = input[inpos];
        inpos += 1;

        if symbol == marker {
            if inpos >= input.len() {
                return Err(DecompressError::UnexpectedEof);
            }
            let next_byte = input[inpos];
            if next_byte == 0 {
                inpos += 1;
                if output.len() >= MAX_DECOMPRESSED_SIZE {
                    return Err(DecompressError::OutputLimitExceeded);
                }
                output.push(marker);
            } else {
                let length = read_var_size(input, &mut inpos)?;
                let offset = read_var_size(input, &mut inpos)?;

                if offset == 0 || offset > output.len() {
                    return Err(DecompressError::InvalidReference {
                        offset,
                        current_pos: output.len(),
                    });
                }

                if output.len().saturating_add(length) > MAX_DECOMPRESSED_SIZE {
                    return Err(DecompressError::OutputLimitExceeded);
                }

                for _ in 0..length {
                    let b = output[output.len() - offset];
                    output.push(b);
                }
            }
        } else {
            if output.len() >= MAX_DECOMPRESSED_SIZE {
                return Err(DecompressError::OutputLimitExceeded);
            }
            output.push(symbol);
        }
    }

    Ok(output)
}

/// Decompresses an LZ77-compressed byte slice according to the Marcus Geelnard LZ77 specification.
pub fn decompress(data: &[u8]) -> Result<Vec<u8>, String> {
    decompress_internal(data).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_literal_and_marker_escapes() {
        // Marker is 0xAA.
        // Stream: marker (0xAA), literal 'H', literal 'i', marker followed by 0x00 (literal 0xAA).
        let input = vec![0xAA, b'H', b'i', 0xAA, 0x00];
        let decompressed = decompress(&input).expect("decompress failed");
        assert_eq!(decompressed, vec![b'H', b'i', 0xAA]);
    }

    #[test]
    fn test_history_reference_copy() {
        // Marker 0xFF. "ABCD" followed by repeat of "ABCD" (length=4, offset=4).
        // Length 4: byte 0x04. Offset 4: byte 0x04.
        let input = vec![0xFF, b'A', b'B', b'C', b'D', 0xFF, 0x04, 0x04];
        let decompressed = decompress(&input).expect("decompress failed");
        assert_eq!(decompressed, b"ABCDABCD");
    }

    #[test]
    fn test_empty_input() {
        assert_eq!(decompress(&[]), Err("empty input".to_string()));
    }

    #[test]
    fn test_unexpected_eof() {
        // Marker only, no subsequent bytes
        assert_eq!(decompress(&[0x55]), Ok(vec![]));

        // Marker followed immediately by end of stream when escape expected
        assert_eq!(
            decompress(&[0x55, b'A', 0x55]),
            Err("unexpected end of file".to_string())
        );

        // Incomplete varint
        assert_eq!(
            decompress(&[0x55, b'A', 0x55, 0x81]),
            Err("unexpected end of file".to_string())
        );
    }

    #[test]
    fn test_invalid_reference() {
        // Reference offset 0
        let input = vec![0xFF, b'A', 0xFF, 0x01, 0x00];
        assert_eq!(
            decompress(&input),
            Err("invalid LZ77 reference: offset 0 at pos 1".to_string())
        );

        // Reference offset beyond history length
        let input = vec![0xFF, b'A', 0xFF, 0x01, 0x05];
        assert_eq!(
            decompress(&input),
            Err("invalid LZ77 reference: offset 5 at pos 1".to_string())
        );
    }

    #[test]
    fn test_overlapping_rle_reference() {
        // Marker 0x7E, literal 'X', repeat with offset 1 and length 5 -> "XXXXXX"
        let input = vec![0x7E, b'X', 0x7E, 0x05, 0x01];
        let decompressed = decompress(&input).expect("decompress failed");
        assert_eq!(decompressed, b"XXXXXX");
    }

    #[test]
    fn test_multi_byte_var_size() {
        // 300 bytes of 'Z' via RLE
        // Length 300 = [0x82, 0x2C], offset 1 = [0x01]
        let input = vec![0x00, b'Z', 0x00, 0x82, 0x2C, 0x01];
        let decompressed = decompress(&input).expect("decompress failed");
        assert_eq!(decompressed.len(), 301);
        assert!(decompressed.iter().all(|&b| b == b'Z'));
    }

    #[test]
    fn test_varint_overflow() {
        // 12 bytes with continuation bit set (overflows 64-bit int)
        let mut input = vec![0xFF, b'A', 0xFF];
        input.extend([0xFF; 12]);
        input.push(0x01); // offset
        assert_eq!(
            decompress(&input),
            Err("variable-length integer overflow".to_string())
        );
    }

    #[test]
    fn test_output_limit_exceeded() {
        // Length 17 MB = 17 * 1024 * 1024 = 17,825,792
        // In 7-bit varint:
        // bytes: 0x88, 0xC0, 0x80, 0x00
        let input = vec![0x00, b'A', 0x00, 0x88, 0xC0, 0x80, 0x00, 0x01];
        assert_eq!(
            decompress(&input),
            Err("decompressed output size limit exceeded".to_string())
        );
    }
}
