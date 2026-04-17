//! DWG bit-stream reader.
//!
//! DWG object data is encoded as a bit stream, not a byte stream.
//! This module implements the low-level primitives documented in the
//! ODA DWG file specification and cross-checked against ACadSharp's
//! `DwgStreamReaderBase`:
//!
//! * `read_bit`   — one boolean bit (MSB first within each byte).
//! * `read_raw_u8 / u16 / u32 / u64` — unaligned little-endian integers.
//! * `read_raw_f64` — 64-bit unaligned IEEE 754 little-endian double.
//! * `read_bit_short` — 2-bit prefix + payload (B).
//! * `read_bit_long`  — 2-bit prefix + payload (BL).
//! * `read_bit_long_long` — 3-bit prefix + payload (BLL).
//! * `read_bit_double` — 2-bit prefix + payload (BD).
//! * `read_handle` — control byte + N raw bytes.
//! * `read_text_ascii` — BS length + ASCII bytes.
//!
//! Everything is MSB-first: bit 7 of byte 0 is the first bit returned.

use crate::DwgReadError;

/// Bit-level reader over a `&[u8]` slice.
///
/// The reader keeps a byte cursor plus an intra-byte bit cursor
/// (`bit_in_byte`, 0..=7 where 0 is the MSB). Unaligned reads are the
/// norm; callers are expected to align only when the on-disk format
/// says so.
#[derive(Debug, Clone)]
pub struct BitReader<'a> {
    bytes: &'a [u8],
    /// Current byte index.
    byte_offset: usize,
    /// Next bit index inside `bytes[byte_offset]`, MSB-first.
    /// Valid values are 0..=7; 8 means "advance to the next byte".
    bit_in_byte: u8,
}

impl<'a> BitReader<'a> {
    pub fn new(bytes: &'a [u8]) -> Self {
        Self {
            bytes,
            byte_offset: 0,
            bit_in_byte: 0,
        }
    }

    pub fn position_in_bits(&self) -> usize {
        self.byte_offset * 8 + self.bit_in_byte as usize
    }

    /// Number of bits that can still be consumed.
    pub fn bits_remaining(&self) -> usize {
        self.bytes.len() * 8 - self.position_in_bits().min(self.bytes.len() * 8)
    }

    /// True if there is not a single bit left to read.
    pub fn is_empty(&self) -> bool {
        self.bits_remaining() == 0
    }

    /// Align the cursor to the next byte boundary if it is not already
    /// aligned. This is a no-op when already at bit 0 of a byte.
    pub fn align_to_byte(&mut self) {
        if self.bit_in_byte != 0 {
            self.byte_offset += 1;
            self.bit_in_byte = 0;
        }
    }

    /// Read a single bit (MSB-first). Returns the bit value 0 or 1.
    pub fn read_bit(&mut self) -> Result<u8, DwgReadError> {
        if self.byte_offset >= self.bytes.len() {
            return Err(DwgReadError::UnexpectedEof { context: "bit" });
        }
        let byte = self.bytes[self.byte_offset];
        let shift = 7 - self.bit_in_byte;
        let bit = (byte >> shift) & 1;
        self.bit_in_byte += 1;
        if self.bit_in_byte == 8 {
            self.bit_in_byte = 0;
            self.byte_offset += 1;
        }
        Ok(bit)
    }

    /// Read `count` bits (MSB-first) packed into a u64. Intended for
    /// counts up to 64 inclusive.
    pub fn read_bits(&mut self, count: u8) -> Result<u64, DwgReadError> {
        if count > 64 {
            return Err(DwgReadError::UnexpectedEof {
                context: "bit count exceeds 64",
            });
        }
        let mut value: u64 = 0;
        for _ in 0..count {
            value = (value << 1) | (self.read_bit()? as u64);
        }
        Ok(value)
    }

    /// Read `count` bytes, filling from MSB first. Handles unaligned
    /// starts.
    pub fn read_bytes(&mut self, count: usize) -> Result<Vec<u8>, DwgReadError> {
        let mut out = Vec::with_capacity(count);
        for _ in 0..count {
            out.push(self.read_raw_u8()?);
        }
        Ok(out)
    }

    /// Raw unsigned 8-bit value (may be bit-shifted if unaligned).
    pub fn read_raw_u8(&mut self) -> Result<u8, DwgReadError> {
        Ok(self.read_bits(8)? as u8)
    }

    /// Raw little-endian 16-bit unsigned value.
    pub fn read_raw_u16_le(&mut self) -> Result<u16, DwgReadError> {
        let lo = self.read_raw_u8()? as u16;
        let hi = self.read_raw_u8()? as u16;
        Ok(lo | (hi << 8))
    }

    /// Raw little-endian 32-bit unsigned value.
    pub fn read_raw_u32_le(&mut self) -> Result<u32, DwgReadError> {
        let mut value: u32 = 0;
        for i in 0..4 {
            value |= (self.read_raw_u8()? as u32) << (i * 8);
        }
        Ok(value)
    }

    /// Raw little-endian 64-bit unsigned value.
    pub fn read_raw_u64_le(&mut self) -> Result<u64, DwgReadError> {
        let mut value: u64 = 0;
        for i in 0..8 {
            value |= (self.read_raw_u8()? as u64) << (i * 8);
        }
        Ok(value)
    }

    /// Raw little-endian 64-bit IEEE 754 double.
    pub fn read_raw_f64_le(&mut self) -> Result<f64, DwgReadError> {
        let bits = self.read_raw_u64_le()?;
        Ok(f64::from_le_bytes(bits.to_le_bytes()))
    }

    /// DWG BitShort (BS). 2-bit prefix:
    /// * `00` — next 16 bits little-endian signed short
    /// * `01` — next 8 bits unsigned short
    /// * `10` — literal 0
    /// * `11` — literal 256
    pub fn read_bit_short(&mut self) -> Result<i16, DwgReadError> {
        let prefix = self.read_bits(2)? as u8;
        match prefix {
            0b00 => Ok(self.read_raw_u16_le()? as i16),
            0b01 => Ok(self.read_raw_u8()? as i16),
            0b10 => Ok(0),
            0b11 => Ok(256),
            _ => unreachable!(),
        }
    }

    /// DWG BitLong (BL). 2-bit prefix:
    /// * `00` — next 32 bits little-endian signed long
    /// * `01` — next 8 bits unsigned
    /// * `10` — literal 0
    /// * `11` — reserved (currently returns 0 to stay defensive)
    pub fn read_bit_long(&mut self) -> Result<i32, DwgReadError> {
        let prefix = self.read_bits(2)? as u8;
        match prefix {
            0b00 => Ok(self.read_raw_u32_le()? as i32),
            0b01 => Ok(self.read_raw_u8()? as i32),
            0b10 => Ok(0),
            0b11 => Ok(0),
            _ => unreachable!(),
        }
    }

    /// DWG BitLongLong (BLL, R24+). 3-bit unsigned prefix N, then
    /// N bytes of raw little-endian unsigned data.
    pub fn read_bit_long_long(&mut self) -> Result<u64, DwgReadError> {
        let len = self.read_bits(3)? as u8;
        if len == 0 {
            return Ok(0);
        }
        if len > 8 {
            return Err(DwgReadError::UnexpectedEof {
                context: "BLL length > 8",
            });
        }
        let mut value: u64 = 0;
        for i in 0..len {
            value |= (self.read_raw_u8()? as u64) << (i as u64 * 8);
        }
        Ok(value)
    }

    /// DWG BitDouble (BD). 2-bit prefix:
    /// * `00` — next 64 bits LE IEEE 754 double
    /// * `01` — literal 1.0
    /// * `10` — literal 0.0
    /// * `11` — reserved
    pub fn read_bit_double(&mut self) -> Result<f64, DwgReadError> {
        let prefix = self.read_bits(2)? as u8;
        match prefix {
            0b00 => self.read_raw_f64_le(),
            0b01 => Ok(1.0),
            0b10 => Ok(0.0),
            0b11 => Ok(0.0),
            _ => unreachable!(),
        }
    }

    /// DWG handle reference (H). Returns `(code, value)` pair where
    /// `code` is the high nibble of the control byte (0..15) and
    /// `value` is the u64 constructed from the trailing raw bytes.
    pub fn read_handle(&mut self) -> Result<(u8, u64), DwgReadError> {
        let control = self.read_raw_u8()?;
        let code = (control >> 4) & 0x0F;
        let len = control & 0x0F;
        if len > 8 {
            return Err(DwgReadError::UnexpectedEof {
                context: "handle length > 8",
            });
        }
        let mut value: u64 = 0;
        for i in 0..len {
            let byte = self.read_raw_u8()? as u64;
            value = (value << 8) | byte;
            let _ = i;
        }
        Ok((code, value))
    }

    /// DWG ASCII text string (T). 16-bit BitShort length followed by
    /// `len` raw ASCII bytes.
    ///
    /// DWG writes the C-style null terminator as part of the string
    /// payload; this helper strips a single trailing `\0` so the
    /// returned string matches the documented value (e.g. `"m"`
    /// instead of `"m\0"`).
    pub fn read_text_ascii(&mut self) -> Result<String, DwgReadError> {
        let len = self.read_bit_short()?;
        if len <= 0 {
            return Ok(String::new());
        }
        let mut bytes = self.read_bytes(len as usize)?;
        if bytes.last() == Some(&0) {
            bytes.pop();
        }
        Ok(String::from_utf8_lossy(&bytes).into_owned())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn read_individual_bits_msb_first() {
        // Byte 0xB5 = 1011 0101
        let mut r = BitReader::new(&[0xB5]);
        assert_eq!(r.read_bit().unwrap(), 1);
        assert_eq!(r.read_bit().unwrap(), 0);
        assert_eq!(r.read_bit().unwrap(), 1);
        assert_eq!(r.read_bit().unwrap(), 1);
        assert_eq!(r.read_bit().unwrap(), 0);
        assert_eq!(r.read_bit().unwrap(), 1);
        assert_eq!(r.read_bit().unwrap(), 0);
        assert_eq!(r.read_bit().unwrap(), 1);
        assert!(r.read_bit().is_err(), "should EOF after 8 bits");
    }

    #[test]
    fn read_bits_packs_into_u64_msb_first() {
        // 0x80 0x40 = 10000000 01000000
        // Position 0..5 (5 bits MSB first): 10000 = 16
        // Position 5..9 (4 bits): 0000 = 0
        // Position 9..11 (2 bits): 10 = 2
        //   (the set bit of 0x40 is at bit 6 = position 9)
        let mut r = BitReader::new(&[0x80, 0x40]);
        assert_eq!(r.read_bits(5).unwrap(), 0b10000);
        assert_eq!(r.read_bits(4).unwrap(), 0b0000);
        assert_eq!(r.read_bits(2).unwrap(), 0b10);
    }

    #[test]
    fn unaligned_u8_round_trips_shift() {
        // 0x80 0xFF = 10000000 11111111
        // Skip 4 bits, read 8 bits:
        // Bits 4..=11 = 0000 1111 (which reads as 0x0F)
        let mut r = BitReader::new(&[0x80, 0xFF]);
        let _ = r.read_bits(4).unwrap();
        assert_eq!(r.read_raw_u8().unwrap(), 0x0F);
    }

    #[test]
    fn bit_short_dispatches_four_prefixes() {
        // prefix 10 -> 0
        let mut r = BitReader::new(&[0b1000_0000]);
        assert_eq!(r.read_bit_short().unwrap(), 0);

        // prefix 11 -> 256
        let mut r = BitReader::new(&[0b1100_0000]);
        assert_eq!(r.read_bit_short().unwrap(), 256);

        // prefix 01, next u8 = 42
        let mut r = BitReader::new(&[0b0100_0000, 42u8 << 2]);
        let _ = r.read_bits(0);
        // prefix 01 is bits 00,01; then 8 bit u8 42
        // bit layout: 01 00101010 00
        // which in two bytes is: 0100 1010 | 1000 0000
        let mut r = BitReader::new(&[0b0100_1010, 0b1000_0000]);
        assert_eq!(r.read_bit_short().unwrap(), 42);

        // prefix 00, next u16 LE = 0x1234
        // layout: 00 00110100 00010010 XX -> pack
        //   bits: 00 0011 0100 0001 0010 XX
        let _ = r;
        // Easier: pack manually.
        let mut bits: Vec<bool> = Vec::new();
        // 00 prefix
        bits.extend_from_slice(&[false, false]);
        // u16 LE 0x1234 -> low byte 0x34, high byte 0x12
        for byte in [0x34u8, 0x12u8] {
            for i in (0..8).rev() {
                bits.push((byte >> i) & 1 == 1);
            }
        }
        let mut bytes = vec![0u8];
        let mut cursor = 0usize;
        for b in &bits {
            let byte_idx = cursor / 8;
            let bit_idx = 7 - (cursor % 8);
            while bytes.len() <= byte_idx {
                bytes.push(0);
            }
            if *b {
                bytes[byte_idx] |= 1 << bit_idx;
            }
            cursor += 1;
        }
        let mut r = BitReader::new(&bytes);
        assert_eq!(r.read_bit_short().unwrap(), 0x1234);
    }

    #[test]
    fn bit_double_dispatches_four_prefixes() {
        // 01 -> 1.0
        let mut r = BitReader::new(&[0b0100_0000]);
        assert_eq!(r.read_bit_double().unwrap(), 1.0);

        // 10 -> 0.0
        let mut r = BitReader::new(&[0b1000_0000]);
        assert_eq!(r.read_bit_double().unwrap(), 0.0);
    }

    #[test]
    fn bit_long_literal_zero_and_small_u8() {
        // 10 -> 0
        let mut r = BitReader::new(&[0b1000_0000]);
        assert_eq!(r.read_bit_long().unwrap(), 0);

        // 01 -> next u8
        // Pack 01 followed by 0x2A
        // 01 00101010 XXXXXX -> byte0 = 0100 1010 = 0x4A, byte1 = 1000 0000 = 0x80
        let mut r = BitReader::new(&[0x4A, 0x80]);
        assert_eq!(r.read_bit_long().unwrap(), 42);
    }

    #[test]
    fn handle_reads_control_and_n_bytes() {
        // control byte 0x42 => code=4, len=2; then bytes 0x12 0x34
        let bytes = [0x42u8, 0x12, 0x34];
        let mut r = BitReader::new(&bytes);
        let (code, value) = r.read_handle().unwrap();
        assert_eq!(code, 4);
        assert_eq!(value, 0x1234);
    }

    #[test]
    fn align_to_byte_is_noop_on_boundary() {
        let mut r = BitReader::new(&[0x55, 0xAA]);
        r.align_to_byte();
        assert_eq!(r.read_raw_u8().unwrap(), 0x55);
    }

    #[test]
    fn align_to_byte_advances_after_partial_read() {
        let mut r = BitReader::new(&[0x80, 0xAA]);
        let _ = r.read_bit().unwrap();
        r.align_to_byte();
        assert_eq!(r.read_raw_u8().unwrap(), 0xAA);
    }

    #[test]
    fn bit_long_long_reads_length_prefix_then_bytes() {
        // prefix 010 (len=2), then bytes 0x34 0x12 (LE => 0x1234)
        // bit layout: 010 00110100 00010010 XXX
        let mut bytes = vec![0u8];
        let mut cursor = 0usize;
        let mut emit = |b: bool, buf: &mut Vec<u8>, cur: &mut usize| {
            let byte_idx = *cur / 8;
            let bit_idx = 7 - (*cur % 8);
            while buf.len() <= byte_idx {
                buf.push(0);
            }
            if b {
                buf[byte_idx] |= 1 << bit_idx;
            }
            *cur += 1;
        };
        for &b in &[false, true, false] {
            emit(b, &mut bytes, &mut cursor);
        }
        for byte in [0x34u8, 0x12u8] {
            for i in (0..8).rev() {
                emit((byte >> i) & 1 == 1, &mut bytes, &mut cursor);
            }
        }
        let mut r = BitReader::new(&bytes);
        assert_eq!(r.read_bit_long_long().unwrap(), 0x1234);
    }
}
