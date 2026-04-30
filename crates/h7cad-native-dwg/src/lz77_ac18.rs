//! AC1018 (AutoCAD R2004) DWG-LZ77 decompressor, R46-B scope.
//!
//! Algorithm reference: ACadSharp
//! `src/ACadSharp/IO/DWG/DwgStreamReaders/DwgLZ77AC18Decompressor.cs`.
//! This is **not** a standard LZ77 / LZ4 / LZMA codec; it is the
//! ODA-defined variant used by AC1018 system pages (page maps,
//! section descriptor maps).
//!
//! R46-B is a pure-algorithm brick: it carries no DWG file-header /
//! section-map dependencies and does **not** wire into
//! [`crate::read_dwg`]. Later bricks (R46-C page map, R46-D section
//! descriptor map) call into this module after they extract the
//! compressed page byte ranges from the AC1018 file layout.
//!
//! # Stream format (high level)
//!
//! ```text
//! opcode1 = first byte
//! if upper-nibble(opcode1) == 0:        # leading-literal preamble
//!     emit (literalCount(opcode1) + 3) raw bytes; opcode1 = next byte
//! loop:
//!     if opcode1 == 0x11: stop          # terminator
//!     decode back-reference (opcode1 selects one of 3 length/offset
//!       encodings); copy compressedBytes from
//!       dst[len-compOffset .. len-compOffset+compressedBytes],
//!       wrapping when compressedBytes > compOffset (RLE mode).
//!     trailing-literal block: 0..3 bytes encoded in low 2 bits of
//!       the post-back-reference opcode, with the 0-special-case
//!       extending into another literalCount chain.
//! ```

use std::fmt;

/// Errors that can surface while decoding an AC1018 DWG-LZ77 stream.
///
/// Kept independent of [`crate::DwgReadError`]: R46-B is a pure
/// algorithm and may be reused outside the `read_dwg` pipeline. Higher
/// bricks (R46-C / R46-D) wrap these into `DwgReadError` via a local
/// `From` impl when they call into the file-header path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Lz77DecodeError {
    /// The input stream ran out before a `0x11` terminator was seen,
    /// or before an opcode that demands further bytes had its operands
    /// fully read.
    TruncatedInput,
    /// A back-reference asked to copy from before the start of the
    /// already-decompressed buffer (i.e. `compOffset > dst.len()`),
    /// or from a non-positive offset (which is illegal in this codec).
    OffsetOutOfRange {
        /// Decoded back-reference offset, kept as `i64` so we can
        /// report the original (possibly negative) value verbatim.
        offset: i64,
        /// Length of the decompressed buffer at the moment the
        /// out-of-range back-reference was detected.
        dst_len: usize,
    },
    /// A back-reference declared a non-positive byte count, or an
    /// extended literal/length chain accumulated past `i32::MAX`. This
    /// usually indicates adversarial / corrupted input — legitimate
    /// AC1018 system pages keep these counts well within page-size
    /// limits.
    LengthOutOfRange {
        /// Decoded length, kept as `i64` so the message can carry the
        /// original (possibly negative or oversized) value.
        value: i64,
    },
    /// The reconstructed buffer would exceed the caller-provided
    /// decompressed-size cap. Defends against malformed input that
    /// tries to balloon the output (OOM resistance).
    OutputOverflow {
        /// Bytes already written to the destination at the moment the
        /// overflow was detected.
        current: usize,
        /// Additional bytes the next `push` would have written.
        attempted_push: usize,
        /// Caller-supplied hard cap on the decompressed size.
        cap: usize,
    },
}

impl fmt::Display for Lz77DecodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TruncatedInput => f.write_str("truncated AC1018 LZ77 input stream"),
            Self::OffsetOutOfRange { offset, dst_len } => write!(
                f,
                "AC1018 LZ77 back-reference offset {offset} is out of range (dst_len={dst_len})"
            ),
            Self::LengthOutOfRange { value } => {
                write!(f, "AC1018 LZ77 length {value} is out of range")
            }
            Self::OutputOverflow {
                current,
                attempted_push,
                cap,
            } => write!(
                f,
                "AC1018 LZ77 output overflow: current={current}, attempted_push={attempted_push}, cap={cap}"
            ),
        }
    }
}

impl std::error::Error for Lz77DecodeError {}

/// Terminator opcode that ends a DWG-LZ77 stream.
const TERMINATOR: i32 = 0x11;

/// Decompress an AC1018 DWG-LZ77 byte stream.
///
/// `decompressed_size` is the caller-known decompressed size and acts
/// as a hard cap: if the algorithm tries to grow the output past
/// `decompressed_size` it errors with [`Lz77DecodeError::OutputOverflow`]
/// instead of allocating unboundedly. The returned `Vec<u8>` may be
/// shorter than `decompressed_size` if the stream terminates early —
/// callers that require an exact match should compare lengths after
/// the call.
///
/// # Errors
///
/// - [`Lz77DecodeError::TruncatedInput`] if `compressed` ends before a
///   `0x11` terminator is seen.
/// - [`Lz77DecodeError::OffsetOutOfRange`] if a back-reference points
///   before the start of the already-decompressed buffer.
/// - [`Lz77DecodeError::LengthOutOfRange`] if a length field decodes
///   to a non-positive value or overflows `i32`.
/// - [`Lz77DecodeError::OutputOverflow`] if the output would exceed
///   `decompressed_size`.
pub fn decompress_ac18_lz77(
    compressed: &[u8],
    decompressed_size: usize,
) -> Result<Vec<u8>, Lz77DecodeError> {
    let mut src = SrcCursor::new(compressed);
    let mut dst: Vec<u8> = Vec::with_capacity(decompressed_size);

    let mut opcode1 = i32::from(src.read_u8()?);

    // Leading-literal preamble: when the very first opcode has its
    // upper nibble cleared, treat it as a literal-length encoding
    // (not a back-reference) and copy the implied literal bytes
    // before entering the main state machine.
    // Cf. ACadSharp DwgLZ77AC18Decompressor.cs L37..L41.
    if (opcode1 & 0xF0) == 0 {
        let lit = literal_count(opcode1, &mut src)?
            .checked_add(3)
            .ok_or(Lz77DecodeError::LengthOutOfRange { value: i64::from(i32::MAX) + 3 })?;
        opcode1 = copy_literal(lit, &mut src, &mut dst, decompressed_size)?;
    }

    while opcode1 != TERMINATOR {
        let comp_offset: i32;
        let compressed_bytes: i32;

        if opcode1 < 0x10 || opcode1 >= 0x40 {
            // Short back-reference: 2-byte encoded opcode pair.
            // Cf. ACadSharp DwgLZ77AC18Decompressor.cs L52..L57.
            //
            // Note: when opcode1 falls in `[0x00, 0x0F]` we deliberately
            // produce `compressed_bytes == -1` so `apply_back_reference`
            // can fail-closed via `LengthOutOfRange`. ACadSharp itself
            // would throw `OverflowException` allocating
            // `new byte[-1]`; legitimate AC1018 streams never enter
            // the main loop with `opcode1 < 0x10`.
            compressed_bytes = (opcode1 >> 4) - 1;
            let opcode2 = i32::from(src.read_u8()?);
            comp_offset = (((opcode1 >> 2) & 3) | (opcode2 << 2)) + 1;
        } else if opcode1 < 0x20 {
            // Long back-reference variant 1.
            // Cf. ACadSharp DwgLZ77AC18Decompressor.cs L60..L65.
            compressed_bytes = read_compressed_bytes(opcode1, 0b0111, &mut src)?;
            let mut offset = (opcode1 & 8) << 11;
            opcode1 = two_byte_offset(&mut offset, 0x4000, &mut src)?;
            comp_offset = offset;
        } else {
            // opcode1 >= 0x20.
            // Cf. ACadSharp DwgLZ77AC18Decompressor.cs L67..L71.
            compressed_bytes = read_compressed_bytes(opcode1, 0b0001_1111, &mut src)?;
            let mut offset = 0;
            opcode1 = two_byte_offset(&mut offset, 1, &mut src)?;
            comp_offset = offset;
        }

        apply_back_reference(comp_offset, compressed_bytes, &mut dst, decompressed_size)?;

        // Trailing-literal block: low 2 bits of the post-back-reference
        // opcode encode 0..3 literal bytes that follow the back-reference.
        // Cf. ACadSharp DwgLZ77AC18Decompressor.cs L88..L99.
        let mut lit_count = opcode1 & 3;
        if lit_count == 0 {
            opcode1 = i32::from(src.read_u8()?);
            if (opcode1 & 0xF0) == 0 {
                lit_count = literal_count(opcode1, &mut src)?.checked_add(3).ok_or(
                    Lz77DecodeError::LengthOutOfRange {
                        value: i64::from(i32::MAX) + 3,
                    },
                )?;
            }
        }
        if lit_count > 0 {
            opcode1 = copy_literal(lit_count, &mut src, &mut dst, decompressed_size)?;
        }
    }

    Ok(dst)
}

/// Forward-only cursor over the compressed input. Wraps `&[u8]` and
/// returns [`Lz77DecodeError::TruncatedInput`] on EOF, mirroring the
/// way ACadSharp's `Stream.ReadByte` panics turn into bounded errors
/// in this Rust port.
struct SrcCursor<'a> {
    bytes: &'a [u8],
    pos: usize,
}

impl<'a> SrcCursor<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, pos: 0 }
    }

    fn read_u8(&mut self) -> Result<u8, Lz77DecodeError> {
        let byte = *self
            .bytes
            .get(self.pos)
            .ok_or(Lz77DecodeError::TruncatedInput)?;
        self.pos += 1;
        Ok(byte)
    }
}

/// `literalCount` from ACadSharp DwgLZ77AC18Decompressor.cs L115..L128.
///
/// Returns the literal byte count *excluding* the +3 adjustment the
/// caller applies; callers compute `lit + 3`.
fn literal_count(code: i32, src: &mut SrcCursor<'_>) -> Result<i32, Lz77DecodeError> {
    let mut lowbits = code & 0x0F;
    if lowbits == 0 {
        loop {
            let b = i32::from(src.read_u8()?);
            if b == 0 {
                lowbits = lowbits
                    .checked_add(0xFF)
                    .ok_or(Lz77DecodeError::LengthOutOfRange {
                        value: i64::from(i32::MAX) + 0xFF,
                    })?;
            } else {
                lowbits = lowbits
                    .checked_add(0x0F)
                    .and_then(|v| v.checked_add(b))
                    .ok_or(Lz77DecodeError::LengthOutOfRange {
                        value: i64::from(i32::MAX) + 0xFF,
                    })?;
                break;
            }
        }
    }
    Ok(lowbits)
}

/// `readCompressedBytes` from ACadSharp DwgLZ77AC18Decompressor.cs
/// L130..L145.
fn read_compressed_bytes(
    opcode1: i32,
    valid_bits: i32,
    src: &mut SrcCursor<'_>,
) -> Result<i32, Lz77DecodeError> {
    let mut compressed_bytes = opcode1 & valid_bits;
    if compressed_bytes == 0 {
        loop {
            let b = i32::from(src.read_u8()?);
            if b == 0 {
                compressed_bytes = compressed_bytes.checked_add(0xFF).ok_or(
                    Lz77DecodeError::LengthOutOfRange {
                        value: i64::from(i32::MAX) + 0xFF,
                    },
                )?;
            } else {
                compressed_bytes = compressed_bytes
                    .checked_add(b)
                    .and_then(|v| v.checked_add(valid_bits))
                    .ok_or(Lz77DecodeError::LengthOutOfRange {
                        value: i64::from(i32::MAX) + 0xFF,
                    })?;
                break;
            }
        }
    }
    compressed_bytes
        .checked_add(2)
        .ok_or(Lz77DecodeError::LengthOutOfRange {
            value: i64::from(i32::MAX) + 2,
        })
}

/// `twoByteOffset` from ACadSharp DwgLZ77AC18Decompressor.cs
/// L147..L156. Returns the *first* of the two read bytes (it doubles
/// as the next opcode); the offset is mutated in place.
fn two_byte_offset(
    offset: &mut i32,
    added_value: i32,
    src: &mut SrcCursor<'_>,
) -> Result<i32, Lz77DecodeError> {
    let first_byte = i32::from(src.read_u8()?);
    *offset |= first_byte >> 2;
    *offset |= i32::from(src.read_u8()?) << 6;
    *offset = offset
        .checked_add(added_value)
        .ok_or(Lz77DecodeError::LengthOutOfRange {
            value: i64::from(i32::MAX) + i64::from(added_value),
        })?;
    Ok(first_byte)
}

/// Append `count` raw literal bytes from `src` to `dst`, then read one
/// more byte and return it as the next opcode. Mirrors `copy` in
/// ACadSharp DwgLZ77AC18Decompressor.cs L103..L113.
fn copy_literal(
    count: i32,
    src: &mut SrcCursor<'_>,
    dst: &mut Vec<u8>,
    cap: usize,
) -> Result<i32, Lz77DecodeError> {
    let count = usize::try_from(count).map_err(|_| Lz77DecodeError::LengthOutOfRange {
        value: i64::from(count),
    })?;
    let attempted = dst
        .len()
        .checked_add(count)
        .ok_or(Lz77DecodeError::OutputOverflow {
            current: dst.len(),
            attempted_push: count,
            cap,
        })?;
    if attempted > cap {
        return Err(Lz77DecodeError::OutputOverflow {
            current: dst.len(),
            attempted_push: count,
            cap,
        });
    }
    for _ in 0..count {
        let byte = src.read_u8()?;
        dst.push(byte);
    }
    Ok(i32::from(src.read_u8()?))
}

/// Apply a back-reference: copy `compressed_bytes` bytes from
/// `dst[len-comp_offset ..]` to the end of `dst`, wrapping (RLE-style)
/// when `compressed_bytes > comp_offset`. Mirrors the body of
/// ACadSharp DwgLZ77AC18Decompressor.cs L73..L85, but uses byte-by-byte
/// re-read of the growing `dst` instead of the C# `tempBuf`
/// indirection (the two are equivalent: both treat the
/// `comp_offset`-byte source window as a circular template).
fn apply_back_reference(
    comp_offset: i32,
    compressed_bytes: i32,
    dst: &mut Vec<u8>,
    cap: usize,
) -> Result<(), Lz77DecodeError> {
    if comp_offset <= 0 {
        return Err(Lz77DecodeError::OffsetOutOfRange {
            offset: i64::from(comp_offset),
            dst_len: dst.len(),
        });
    }
    let comp_offset = comp_offset as usize;
    let compressed_bytes =
        usize::try_from(compressed_bytes).map_err(|_| Lz77DecodeError::LengthOutOfRange {
            value: i64::from(compressed_bytes),
        })?;
    if comp_offset > dst.len() {
        return Err(Lz77DecodeError::OffsetOutOfRange {
            offset: comp_offset as i64,
            dst_len: dst.len(),
        });
    }
    let attempted = dst
        .len()
        .checked_add(compressed_bytes)
        .ok_or(Lz77DecodeError::OutputOverflow {
            current: dst.len(),
            attempted_push: compressed_bytes,
            cap,
        })?;
    if attempted > cap {
        return Err(Lz77DecodeError::OutputOverflow {
            current: dst.len(),
            attempted_push: compressed_bytes,
            cap,
        });
    }
    for _ in 0..compressed_bytes {
        let src_idx = dst.len() - comp_offset;
        let byte = dst[src_idx];
        dst.push(byte);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Smallest legal stream: a single `0x11` terminator decompresses
    /// to an empty buffer.
    #[test]
    fn terminator_only_returns_empty() {
        let bytes = [0x11];
        let out = decompress_ac18_lz77(&bytes, 0).expect("terminator-only stream decodes");
        assert!(out.is_empty(), "expected empty output, got {out:?}");
    }

    /// Leading-literal preamble: `opcode1 = 0x05` activates the
    /// `(opcode1 & 0xF0) == 0` branch with `lowbits = 5`, producing
    /// `5 + 3 = 8` literal bytes, then the next byte (`0x11`)
    /// terminates the stream.
    #[test]
    fn leading_literal_preamble_round_trips_eight_bytes() {
        let bytes = [
            0x05, b'A', b'B', b'C', b'D', b'E', b'F', b'G', b'H', // 8 literal bytes
            0x11, // terminator
        ];
        let out = decompress_ac18_lz77(&bytes, 8).expect("leading-literal stream decodes");
        assert_eq!(out, b"ABCDEFGH");
    }

    /// Short back-reference happy path. Layout:
    ///
    /// 1. Leading literal `"ABCDEFGH"` (opcode1 = 0x01, +3 = 4 bytes
    ///    → wait we need 8, see encoding below).
    /// 2. Short back-reference with compressed_bytes=4, comp_offset=4
    ///    → copies `dst[4..8] == "EFGH"` to end of dst.
    /// 3. Terminator.
    ///
    /// Encoding: leading `0x05` -> 5+3 = 8 literal bytes "ABCDEFGH";
    /// then `0x5C 0x00`:
    ///   compressed_bytes = (0x5C >> 4) - 1 = 4
    ///   comp_offset      = (((0x5C >> 2) & 3) | (0x00 << 2)) + 1
    ///                    = (0x17 & 3) + 1 = 3 + 1 = 4
    /// then `0x11` terminator.
    #[test]
    fn short_back_reference_copies_existing_window() {
        let bytes = [
            0x05, b'A', b'B', b'C', b'D', b'E', b'F', b'G', b'H', // 8 literal bytes
            0x5C, 0x00, // back-ref compressed_bytes=4, comp_offset=4
            0x11, // terminator
        ];
        let out = decompress_ac18_lz77(&bytes, 12).expect("short back-ref stream decodes");
        assert_eq!(out, b"ABCDEFGHEFGH");
    }

    /// RLE / self-overlapping back-reference: `comp_offset = 1` and
    /// `compressed_bytes = 3` repeats the last byte three times.
    /// Encoding choices verified by hand:
    ///   opcode1 = 0x40 → compressed_bytes = (0x40 >> 4) - 1 = 3
    ///   opcode2 = 0x00 → comp_offset = (((0x40 >> 2) & 3) | 0) + 1
    ///                                = (0x10 & 3) + 1 = 0 + 1 = 1
    #[test]
    fn rle_self_overlapping_back_reference_repeats_last_byte() {
        let bytes = [
            0x01, b'A', b'A', b'B', b'B', // leading literal "AABB" (1+3 = 4 bytes)
            0x40, 0x00, // back-ref compressed_bytes=3, comp_offset=1 → "BBB"
            0x11, // terminator
        ];
        let out = decompress_ac18_lz77(&bytes, 7).expect("rle stream decodes");
        assert_eq!(out, b"AABBBBB");
    }

    /// Trailing-literal block via the low-2-bits encoding: opcode1
    /// after a back-reference has `opcode1 & 3 == 1`, so 1 literal
    /// byte follows. Layout:
    ///   leading "AB" via 0x01 wait - leading needs 4 bytes minimum
    ///   (lowbits=1 + 3 = 4). Use 4-byte leading "ABCD".
    ///   short back-ref comp_offset=4 compressed_bytes=4 with
    ///   `opcode1 & 3 == 1` → 1 trailing literal "Z".
    ///   terminator.
    ///
    /// Encoding:
    ///   leading `0x01 'A' 'B' 'C' 'D'` (4 literal bytes).
    ///   back-ref `0x5D 0x00`:
    ///     compressed_bytes = (0x5D >> 4) - 1 = 4
    ///     comp_offset      = (((0x5D >> 2) & 3) | 0) + 1
    ///                      = (0x17 & 3) + 1 = 4
    ///     opcode1 & 3 = 0x5D & 3 = 1 → 1 trailing literal byte
    ///   trailing literal `'Z'`, then copy_literal reads next opcode
    ///   `0x11` as terminator.
    #[test]
    fn trailing_literal_block_via_low_two_bits() {
        let bytes = [
            0x01, b'A', b'B', b'C', b'D', // leading literal "ABCD"
            0x5D, 0x00, // short back-ref + 1 trailing literal flag
            b'Z', // 1 trailing literal byte
            0x11, // terminator
        ];
        let out = decompress_ac18_lz77(&bytes, 9).expect("trailing-literal stream decodes");
        assert_eq!(out, b"ABCDABCDZ");
    }

    /// Trailing-literal block via the extended `litCount == 0 && next
    /// opcode high-nibble 0` chain: after a back-reference, opcode1
    /// has `& 3 == 0`, the next byte (`0x02`) is read; its upper
    /// nibble is 0 so it is decoded as `literalCount(0x02) + 3 = 5`
    /// literal bytes.
    ///
    /// We use the same back-ref as `short_back_reference_copies…`
    /// but with `opcode1 & 3 == 0` → encoding `0x5C 0x00`. Then the
    /// next byte must be `0x02` (lowbits=2, no extension chain),
    /// followed by 5 literal bytes, then `0x11` terminator.
    #[test]
    fn trailing_literal_block_via_extended_literal_count() {
        let bytes = [
            0x05, b'A', b'B', b'C', b'D', b'E', b'F', b'G', b'H', // leading 8 literal bytes
            0x5C, 0x00, // short back-ref compressed_bytes=4 comp_offset=4, lit==0 path
            0x02, // next opcode -> literalCount(0x02)+3 = 5 trailing literals
            b'1', b'2', b'3', b'4', b'5', // 5 trailing literal bytes
            0x11, // terminator
        ];
        let out =
            decompress_ac18_lz77(&bytes, 17).expect("extended-literal stream decodes");
        assert_eq!(out, b"ABCDEFGHEFGH12345");
    }

    /// Extended literal-count chain via opcode1 `0x00`: lowbits = 0
    /// activates the loop; one zero byte adds 0xFF, one terminating
    /// non-zero byte (0x05) adds 0x0F + 0x05 = 0x14. Total:
    /// 0xFF + 0x14 = 0x113 = 275. Plus the leading-literal `+3`
    /// adjustment: 275 + 3 = 278 literal bytes before the terminator.
    ///
    /// We synthesise 278 literal bytes (`0x41` repeated) followed by
    /// the `0x11` terminator and verify the round trip. This
    /// exercises the only multi-byte literal_count path in the
    /// algorithm.
    #[test]
    fn literal_count_extended_chain_decodes_278_literals() {
        let mut bytes = Vec::new();
        bytes.push(0x00); // opcode1 = 0x00 → enter literal_count extension
        bytes.push(0x00); // first chain byte = 0x00 adds 0xFF (255)
        bytes.push(0x05); // terminating byte: lowbits += 0x0F + 0x05 = 0x14 (20)
        bytes.extend(std::iter::repeat(0x41).take(278)); // 278 literal bytes
        bytes.push(0x11); // terminator (read by copy_literal as next opcode)

        let out = decompress_ac18_lz77(&bytes, 278)
            .expect("extended literal_count chain decodes");
        assert_eq!(out.len(), 278);
        assert!(out.iter().all(|&b| b == 0x41));
    }

    /// Truncated stream: leading-literal preamble announces 8 bytes
    /// but the input only carries 2.
    #[test]
    fn truncated_input_returns_truncated_input_error() {
        let bytes = [0x05, b'A', b'B'];
        let err = decompress_ac18_lz77(&bytes, 16).unwrap_err();
        assert_eq!(err, Lz77DecodeError::TruncatedInput);
    }

    /// Out-of-range back-reference: `comp_offset = 10` but only 4
    /// bytes have been emitted.
    ///
    /// Encoding:
    ///   leading `0x01 'A' 'B' 'C' 'D'` (4 literal bytes)
    ///   short back-ref `0x44 0x02`:
    ///     compressed_bytes = (0x44 >> 4) - 1 = 3
    ///     comp_offset      = (((0x44 >> 2) & 3) | (0x02 << 2)) + 1
    ///                      = ((0x11 & 3) | 8) + 1 = (1 | 8) + 1 = 10
    #[test]
    fn out_of_range_back_reference_returns_offset_out_of_range_error() {
        let bytes = [0x01, b'A', b'B', b'C', b'D', 0x44, 0x02];
        let err = decompress_ac18_lz77(&bytes, 64).unwrap_err();
        assert_eq!(
            err,
            Lz77DecodeError::OffsetOutOfRange {
                offset: 10,
                dst_len: 4,
            }
        );
    }

    /// `decompressed_size` cap is honoured: the leading-literal
    /// preamble announces 8 bytes but the cap is only 4.
    #[test]
    fn output_overflow_returns_output_overflow_error() {
        let bytes = [
            0x05, b'A', b'B', b'C', b'D', b'E', b'F', b'G', b'H', 0x11,
        ];
        let err = decompress_ac18_lz77(&bytes, 4).unwrap_err();
        assert_eq!(
            err,
            Lz77DecodeError::OutputOverflow {
                current: 0,
                attempted_push: 8,
                cap: 4,
            }
        );
    }

    /// Negative compressed-bytes path: after a leading literal,
    /// `opcode1 = 0x05` re-enters the main loop with `opcode1 < 0x10`,
    /// which produces `compressed_bytes = (0x05 >> 4) - 1 = -1`.
    /// `apply_back_reference` must fail-closed instead of panicking.
    ///
    /// Encoding:
    ///   leading `0x01 'A' 'B' 'C' 'D'` (4 literal bytes)
    ///   then byte `0x05` (re-enters main loop with bad opcode1)
    ///   then byte `0x00` (read as opcode2; comp_offset = 1)
    #[test]
    fn negative_compressed_bytes_returns_length_out_of_range_error() {
        let bytes = [0x01, b'A', b'B', b'C', b'D', 0x05, 0x00];
        let err = decompress_ac18_lz77(&bytes, 64).unwrap_err();
        assert_eq!(err, Lz77DecodeError::LengthOutOfRange { value: -1 });
    }

    /// `Lz77DecodeError` carries useful context in its `Display`
    /// output: surface a few key error strings so future readers /
    /// log scrapes can match on stable text.
    #[test]
    fn error_display_strings_include_diagnostics() {
        let truncated = format!("{}", Lz77DecodeError::TruncatedInput);
        assert!(truncated.contains("AC1018 LZ77"));

        let offset = format!(
            "{}",
            Lz77DecodeError::OffsetOutOfRange {
                offset: -1,
                dst_len: 0,
            }
        );
        assert!(offset.contains("offset -1"));
        assert!(offset.contains("dst_len=0"));

        let length = format!(
            "{}",
            Lz77DecodeError::LengthOutOfRange { value: -2 }
        );
        assert!(length.contains("length -2"));

        let overflow = format!(
            "{}",
            Lz77DecodeError::OutputOverflow {
                current: 1,
                attempted_push: 2,
                cap: 3,
            }
        );
        assert!(overflow.contains("current=1"));
        assert!(overflow.contains("attempted_push=2"));
        assert!(overflow.contains("cap=3"));
    }
}
