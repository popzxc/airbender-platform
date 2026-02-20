//! Canonical host/guest input wire format.
//!
//! The input stream is encoded as `u32` words where:
//! - the first word stores payload byte length,
//! - each following word stores up to 4 payload bytes in big-endian order,
//! - the final word is zero-padded when payload length is not a multiple of 4.

use alloc::vec::Vec;

const WORD_BYTES: usize = 4;

/// Read one framed payload from a word source.
///
/// The provided callback must yield the frame length word first, then payload words.
pub fn read_framed_bytes_with(mut read_word: impl FnMut() -> u32) -> Vec<u8> {
    let len = read_word() as usize;
    let words_needed = len.div_ceil(WORD_BYTES);

    let mut bytes = Vec::with_capacity(len);
    let mut remaining = len;
    for _ in 0..words_needed {
        let word_bytes = read_word().to_be_bytes();
        let bytes_to_take = remaining.min(WORD_BYTES);
        bytes.extend_from_slice(&word_bytes[..bytes_to_take]);
        remaining -= bytes_to_take;
    }

    bytes
}

/// Frame payload bytes into input words consumed by the runtime.
pub fn frame_words_from_bytes(bytes: &[u8]) -> Vec<u32> {
    let word_count = bytes.len().div_ceil(WORD_BYTES);
    let mut words = Vec::with_capacity(1 + word_count);
    words.push(bytes.len() as u32);
    for chunk in bytes.chunks(WORD_BYTES) {
        let mut padded = [0u8; WORD_BYTES];
        padded[..chunk.len()].copy_from_slice(chunk);
        words.push(u32::from_be_bytes(padded));
    }
    words
}

#[cfg(test)]
mod tests {
    use super::{frame_words_from_bytes, read_framed_bytes_with};

    #[test]
    fn framing_roundtrip() {
        let bytes = b"airbender";
        let words = frame_words_from_bytes(bytes);
        assert_eq!(words[0], bytes.len() as u32);
        let mut cursor = 0;
        let reconstructed = read_framed_bytes_with(|| {
            let word = words[cursor];
            cursor += 1;
            word
        });
        assert_eq!(reconstructed, bytes);
    }

    #[test]
    fn closure_reader_handles_partial_word() {
        let bytes = [0x12u8, 0x34, 0x56];
        let words = frame_words_from_bytes(&bytes);
        let mut cursor = 0;
        let reconstructed = read_framed_bytes_with(|| {
            let word = words[cursor];
            cursor += 1;
            word
        });
        assert_eq!(reconstructed, bytes);
    }
}
