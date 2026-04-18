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
    /// Absolute bit position (measured from `bytes[0]`) that marks the
    /// exclusive end of this reader's visible range.
    end_bit: usize,
}

impl<'a> BitReader<'a> {
    pub fn new(bytes: &'a [u8]) -> Self {
        Self {
            bytes,
            byte_offset: 0,
            bit_in_byte: 0,
            end_bit: bytes.len() * 8,
        }
    }

    /// Build a reader over an arbitrary bit range inside `bytes`.
    ///
    /// The visible window is `[start_bit, end_bit)`, both measured
    /// from the start of `bytes`. Reads that would cross `end_bit`
    /// surface as `UnexpectedEof`.
    pub fn from_bit_range(
        bytes: &'a [u8],
        start_bit: usize,
        end_bit: usize,
    ) -> Result<Self, DwgReadError> {
        let total_bits = bytes.len() * 8;
        if start_bit > end_bit || end_bit > total_bits {
            return Err(DwgReadError::UnexpectedEof {
                context: "bit range outside backing slice",
            });
        }
        Ok(Self {
            bytes,
            byte_offset: start_bit / 8,
            bit_in_byte: (start_bit % 8) as u8,
            end_bit,
        })
    }

    pub fn position_in_bits(&self) -> usize {
        self.byte_offset * 8 + self.bit_in_byte as usize
    }

    pub fn bytes(&self) -> &'a [u8] {
        self.bytes
    }


    /// Reposition the cursor to an absolute bit offset measured from
    /// the start of the backing slice.
    pub fn set_position_in_bits(&mut self, position: usize) -> Result<(), DwgReadError> {
        if position > self.end_bit {
            return Err(DwgReadError::UnexpectedEof {
                context: "bit position outside visible range",
            });
        }
        self.byte_offset = position / 8;
        self.bit_in_byte = (position % 8) as u8;
        Ok(())
    }

    /// Number of bits that can still be consumed.
    pub fn bits_remaining(&self) -> usize {
        self.end_bit.saturating_sub(self.position_in_bits())
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
        if self.position_in_bits() >= self.end_bit || self.byte_offset >= self.bytes.len() {
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

    /// DWG BitDouble-with-default (DD). 2-bit prefix, payload relative
    /// to a caller-supplied default:
    /// * `00` — no data, return `default` unchanged
    /// * `01` — next 4 raw bytes replace bytes 0..=3 of the IEEE 754
    ///   little-endian representation of `default`
    /// * `10` — next 6 raw bytes replace bytes 4..=5 and then 0..=3 of
    ///   the default (the high-word half arrives first, mirroring the
    ///   wire format used by ACadSharp's DwgBitReader.BD_with_default)
    /// * `11` — a full raw LE IEEE 754 double follows, default ignored
    ///
    /// Used by entity geometry decoders where one coordinate is read
    /// as a raw double and subsequent coordinates are encoded as small
    /// deltas relative to it (e.g. LINE end.x ← start.x).
    pub fn read_bit_double_with_default(&mut self, default: f64) -> Result<f64, DwgReadError> {
        let mut bytes = default.to_le_bytes();
        match self.read_bits(2)? as u8 {
            0b00 => Ok(default),
            0b01 => {
                for byte in bytes.iter_mut().take(4) {
                    *byte = self.read_raw_u8()?;
                }
                Ok(f64::from_le_bytes(bytes))
            }
            0b10 => {
                bytes[4] = self.read_raw_u8()?;
                bytes[5] = self.read_raw_u8()?;
                for byte in bytes.iter_mut().take(4) {
                    *byte = self.read_raw_u8()?;
                }
                Ok(f64::from_le_bytes(bytes))
            }
            0b11 => self.read_raw_f64_le(),
            _ => unreachable!(),
        }
    }

    /// DWG 3BD — three consecutive BitDoubles as an (x, y, z) triple.
    pub fn read_3bit_double(&mut self) -> Result<[f64; 3], DwgReadError> {
        let x = self.read_bit_double()?;
        let y = self.read_bit_double()?;
        let z = self.read_bit_double()?;
        Ok([x, y, z])
    }

    /// DWG 2RD — two raw doubles encoded back-to-back.
    pub fn read_2raw_double(&mut self) -> Result<[f64; 2], DwgReadError> {
        let x = self.read_raw_f64_le()?;
        let y = self.read_raw_f64_le()?;
        Ok([x, y])
    }

    /// DWG 2BD — two BitDoubles encoded back-to-back.
    pub fn read_2bit_double(&mut self) -> Result<[f64; 2], DwgReadError> {
        let x = self.read_bit_double()?;
        let y = self.read_bit_double()?;
        Ok([x, y])
    }

    /// DWG BitExtrusion (BE), R2000+ encoding only.
    ///
    /// A single control bit:
    /// * `1` — the extrusion is the default unit Z normal `(0, 0, 1)`;
    ///   no further bits are consumed
    /// * `0` — followed by a full 3BD triple containing the actual
    ///   extrusion vector
    ///
    /// Pre-R2000 drawings always write a raw 3BD and are not handled
    /// here; this crate currently targets AC1015 only.
    pub fn read_bit_extrusion_r2000_plus(&mut self) -> Result<[f64; 3], DwgReadError> {
        if self.read_bit()? == 1 {
            Ok([0.0, 0.0, 1.0])
        } else {
            self.read_3bit_double()
        }
    }

    /// DWG BitThickness (BT), R2000+ encoding only.
    ///
    /// A single control bit:
    /// * `1` — thickness is 0.0; no further bits are consumed
    /// * `0` — thickness is the BD that follows
    ///
    /// Pre-R2000 drawings always write a BD and are not handled here.
    pub fn read_bit_thickness_r2000_plus(&mut self) -> Result<f64, DwgReadError> {
        if self.read_bit()? == 1 {
            Ok(0.0)
        } else {
            self.read_bit_double()
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

    /// DWG handle reference resolved relative to `reference_handle`.
    ///
    /// This matches the ACadSharp / ODA convention used by the handle
    /// stream in AC1015 object bodies:
    ///
    /// * `0x2..=0x5` => absolute handle bytes
    /// * `0x6` => `reference_handle + 1`
    /// * `0x8` => `reference_handle - 1`
    /// * `0xA` => `reference_handle + offset`
    /// * `0xC` => `reference_handle - offset`
    pub fn read_handle_relative(&mut self, reference_handle: u64) -> Result<u64, DwgReadError> {
        let (code, value) = self.read_handle()?;
        Ok(match code {
            0x0..=0x5 => value,
            0x6 => reference_handle.wrapping_add(1),
            0x8 => reference_handle.wrapping_sub(1),
            0xA => reference_handle.wrapping_add(value),
            0xC => reference_handle.wrapping_sub(value),
            _ => 0,
        })
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
    fn bit_double_with_default_zero_prefix_returns_default_unchanged() {
        // Prefix 00, no payload bits consumed beyond the 2 prefix bits.
        let mut r = BitReader::new(&[0b0000_0000]);
        let value = r.read_bit_double_with_default(7.5).unwrap();
        assert_eq!(value, 7.5);
        assert_eq!(r.position_in_bits(), 2);
    }

    #[test]
    fn bit_double_with_default_full_prefix_reads_raw_double() {
        // Prefix 11 then a full LE raw double 1.25.
        // Layout: 2 prefix bits + 64 payload bits = 66 bits = 9 bytes with 6 trailing pad bits.
        let raw = 1.25_f64.to_le_bytes();
        let mut buf = Vec::with_capacity(9);
        // First byte: prefix 11 in high bits + high 6 bits of raw[0].
        buf.push(0b1100_0000 | (raw[0] >> 2));
        // Next 7 bytes: each is (low 2 bits of prev) << 6 | (high 6 bits of next).
        for i in 0..7 {
            buf.push(((raw[i] & 0x03) << 6) | (raw[i + 1] >> 2));
        }
        // Tail byte: low 2 bits of raw[7] in top bits, pad with zeros.
        buf.push((raw[7] & 0x03) << 6);

        let mut r = BitReader::new(&buf);
        let value = r.read_bit_double_with_default(0.0).unwrap();
        assert_eq!(value, 1.25);
        assert_eq!(r.position_in_bits(), 66);
    }

    #[test]
    fn bit_double_with_default_01_replaces_low_four_bytes() {
        // Prefix 01 then 4 raw bytes. Default = 1.0 (bytes
        // 0x00,0x00,0x00,0x00,0x00,0x00,0xF0,0x3F). Replace bytes 0..=3
        // with 0x01,0x02,0x03,0x04. Result = f64::from_le_bytes([01,02,03,04,00,00,F0,3F]).
        let default = 1.0_f64;
        let expected = f64::from_le_bytes([0x01, 0x02, 0x03, 0x04, 0x00, 0x00, 0xF0, 0x3F]);
        // 2 prefix bits 01 + 4 * 8 = 34 bits → 5 bytes with 6 pad bits at end.
        //   byte0: 01 000000 → 0x40 ∧ high 6 bits of 0x01 (=0b000000) = 0x40
        //   byte1: (0x01 low 2 bits << 6) | (0x02 high 6 bits) = (0x01<<6) | (0x02>>2) = 0x40 | 0x00 = 0x40
        //   byte2: (0x02 low 2 bits << 6) | (0x03 high 6 bits) = (0x02<<6) | 0x00     = 0x80
        //   byte3: (0x03 low 2 bits << 6) | (0x04 high 6 bits) = (0x03<<6) | 0x01     = 0xC1
        //   byte4: (0x04 low 2 bits << 6) | 0 = 0x00
        let buf = [0x40, 0x40, 0x80, 0xC1, 0x00];
        let mut r = BitReader::new(&buf);
        let value = r.read_bit_double_with_default(default).unwrap();
        assert_eq!(value, expected);
        assert_eq!(r.position_in_bits(), 34);
    }

    #[test]
    fn bit_extrusion_flag_bit_returns_unit_z_normal() {
        // Single `1` bit ⇒ default extrusion (0,0,1); no further bits.
        let mut r = BitReader::new(&[0b1000_0000]);
        let normal = r.read_bit_extrusion_r2000_plus().unwrap();
        assert_eq!(normal, [0.0, 0.0, 1.0]);
        assert_eq!(r.position_in_bits(), 1);
    }

    #[test]
    fn bit_extrusion_zero_flag_reads_three_bit_doubles() {
        // Flag 0 then three BDs each using prefix 01 (=1.0). Total =
        // 1 + 3*2 = 7 bits = 1 byte with one pad bit.
        // Layout: 0 01 01 01 0 -> bits 0010101_0 -> 0x2A.
        let mut r = BitReader::new(&[0b0010_1010]);
        let normal = r.read_bit_extrusion_r2000_plus().unwrap();
        assert_eq!(normal, [1.0, 1.0, 1.0]);
        assert_eq!(r.position_in_bits(), 7);
    }

    #[test]
    fn bit_thickness_flag_bit_returns_zero() {
        let mut r = BitReader::new(&[0b1000_0000]);
        let thickness = r.read_bit_thickness_r2000_plus().unwrap();
        assert_eq!(thickness, 0.0);
        assert_eq!(r.position_in_bits(), 1);
    }

    #[test]
    fn bit_thickness_zero_flag_reads_bit_double() {
        // Flag 0 then BD prefix 01 = literal 1.0 (3 bits total).
        // Bits: 0 01 00000 → 0b0010_0000 = 0x20.
        let mut r = BitReader::new(&[0b0010_0000]);
        let thickness = r.read_bit_thickness_r2000_plus().unwrap();
        assert_eq!(thickness, 1.0);
        assert_eq!(r.position_in_bits(), 3);
    }

    #[test]
    fn read_3bit_double_reads_three_bds_in_order() {
        // Three BDs with prefixes 01 / 10 / 01 → 1.0 / 0.0 / 1.0.
        // Bits: 01 10 01 00 → 0b0110_0100 = 0x64.
        let mut r = BitReader::new(&[0b0110_0100]);
        let triple = r.read_3bit_double().unwrap();
        assert_eq!(triple, [1.0, 0.0, 1.0]);
        assert_eq!(r.position_in_bits(), 6);
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
    fn handle_relative_resolves_plus_and_minus_offsets() {
        let mut plus_one = BitReader::new(&[0x60]);
        assert_eq!(plus_one.read_handle_relative(0x2A).unwrap(), 0x2B);

        let mut minus_one = BitReader::new(&[0x80]);
        assert_eq!(minus_one.read_handle_relative(0x2A).unwrap(), 0x29);

        let mut plus_delta = BitReader::new(&[0xA1, 0x05]);
        assert_eq!(plus_delta.read_handle_relative(0x20).unwrap(), 0x25);

        let mut minus_delta = BitReader::new(&[0xC1, 0x05]);
        assert_eq!(minus_delta.read_handle_relative(0x20).unwrap(), 0x1B);
    }

    #[test]
    fn bit_range_reader_reports_eof_at_range_end() {
        let bytes = [0xAA, 0x55];
        let mut r = BitReader::from_bit_range(&bytes, 4, 12).unwrap();
        assert_eq!(r.bits_remaining(), 8);
        assert_eq!(r.read_raw_u8().unwrap(), 0xA5);
        assert!(matches!(
            r.read_bit().unwrap_err(),
            DwgReadError::UnexpectedEof { .. }
        ));
    }

    #[test]
    fn set_position_in_bits_repositions_inside_visible_range() {
        let bytes = [0xF0, 0x0F];
        let mut r = BitReader::from_bit_range(&bytes, 0, 12).unwrap();
        assert_eq!(r.read_bits(4).unwrap(), 0xF);
        r.set_position_in_bits(8).unwrap();
        assert_eq!(r.read_bits(4).unwrap(), 0x0);
        assert!(matches!(
            r.set_position_in_bits(13).unwrap_err(),
            DwgReadError::UnexpectedEof { .. }
        ));
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
        let emit = |b: bool, buf: &mut Vec<u8>, cur: &mut usize| {
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
