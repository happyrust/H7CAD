//! DWG bit-stream writer.
//!
//! Mirror image of [`crate::BitReader`]: the writer accepts the same
//! conceptual values (raw integers, BitShort, BitLong, BitDouble,
//! handle references, ASCII text) and emits the MSB-first bit stream
//! that a fresh `BitReader` can decode back to the original value.
//!
//! Everything is MSB-first: bit 7 of the first emitted byte is the
//! first bit produced. Unaligned writes are the norm — the writer
//! grows its backing `Vec<u8>` one bit at a time and pads to the
//! containing byte only when [`BitWriter::align_to_byte`] is invoked.
//!
//! This is the F5.M1 brick from
//! `docs/plans/2026-05-08-dwg-next-step-plan.md`. Higher-level
//! composers (section payloads, handle map, object stream) build on
//! this foundation in F5.M2 / M3 / M4.

use crate::DwgWriteError;

/// Bit-level writer building up a `Vec<u8>` in MSB-first order.
#[derive(Debug, Default)]
pub struct BitWriter {
    bytes: Vec<u8>,
    /// Next bit slot inside `bytes.last()`, MSB-first. `0` means the
    /// MSB of a *fresh* trailing byte (which the writer pushes lazily
    /// the first time a bit is emitted into it).
    bit_in_byte: u8,
}

impl BitWriter {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_capacity(byte_capacity: usize) -> Self {
        Self {
            bytes: Vec::with_capacity(byte_capacity),
            bit_in_byte: 0,
        }
    }

    /// Total number of bits emitted so far.
    pub fn position_in_bits(&self) -> usize {
        if self.bytes.is_empty() {
            0
        } else if self.bit_in_byte == 0 {
            self.bytes.len() * 8
        } else {
            (self.bytes.len() - 1) * 8 + self.bit_in_byte as usize
        }
    }

    /// Borrow the bytes assembled so far. Useful for chained writers
    /// that want to inspect intermediate state without consuming the
    /// builder.
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// Consume the writer and return the underlying byte buffer. The
    /// trailing byte (if any) is zero-padded on its low bits — exactly
    /// what an aligned reader expects.
    pub fn into_bytes(self) -> Vec<u8> {
        self.bytes
    }

    /// Pad with zero bits up to the next byte boundary. Mirror of
    /// [`crate::BitReader::align_to_byte`]. No-op when already aligned.
    pub fn align_to_byte(&mut self) {
        if self.bit_in_byte != 0 {
            self.bit_in_byte = 0;
        }
    }

    /// Emit a single bit (MSB-first within the current byte). Accepts
    /// any non-zero value as `1`; only the LSB of `bit` is consulted.
    pub fn write_bit(&mut self, bit: u8) -> Result<(), DwgWriteError> {
        if self.bit_in_byte == 0 {
            self.bytes.push(0);
        }
        if bit & 1 == 1 {
            let byte_idx = self.bytes.len() - 1;
            let shift = 7 - self.bit_in_byte;
            self.bytes[byte_idx] |= 1 << shift;
        }
        self.bit_in_byte = (self.bit_in_byte + 1) % 8;
        Ok(())
    }

    /// Emit `count` bits from `value` MSB-first. `count` must be in
    /// `0..=64`.
    pub fn write_bits(&mut self, value: u64, count: u8) -> Result<(), DwgWriteError> {
        if count > 64 {
            return Err(DwgWriteError::InvalidValue(format!(
                "write_bits count {count} exceeds 64"
            )));
        }
        for i in (0..count).rev() {
            let bit = ((value >> i) & 1) as u8;
            self.write_bit(bit)?;
        }
        Ok(())
    }

    /// Emit `bytes` verbatim, MSB-first within each byte. Handles
    /// unaligned starts.
    pub fn write_bytes(&mut self, bytes: &[u8]) -> Result<(), DwgWriteError> {
        for byte in bytes {
            self.write_raw_u8(*byte)?;
        }
        Ok(())
    }

    /// Raw unsigned 8-bit value (may straddle a byte boundary).
    pub fn write_raw_u8(&mut self, value: u8) -> Result<(), DwgWriteError> {
        self.write_bits(value as u64, 8)
    }

    /// Raw little-endian 16-bit unsigned value.
    pub fn write_raw_u16_le(&mut self, value: u16) -> Result<(), DwgWriteError> {
        for byte in value.to_le_bytes() {
            self.write_raw_u8(byte)?;
        }
        Ok(())
    }

    /// Raw little-endian 32-bit unsigned value.
    pub fn write_raw_u32_le(&mut self, value: u32) -> Result<(), DwgWriteError> {
        for byte in value.to_le_bytes() {
            self.write_raw_u8(byte)?;
        }
        Ok(())
    }

    /// Raw little-endian 64-bit unsigned value.
    pub fn write_raw_u64_le(&mut self, value: u64) -> Result<(), DwgWriteError> {
        for byte in value.to_le_bytes() {
            self.write_raw_u8(byte)?;
        }
        Ok(())
    }

    /// Raw little-endian 64-bit IEEE 754 double.
    pub fn write_raw_f64_le(&mut self, value: f64) -> Result<(), DwgWriteError> {
        self.write_raw_u64_le(u64::from_le_bytes(value.to_le_bytes()))
    }

    /// DWG BitShort (BS). Mirror of [`crate::BitReader::read_bit_short`].
    /// Encoding picks the most compact prefix that round-trips:
    /// `0` → `10`, `256` → `11`, fits in `u8` → `01 + raw_u8`,
    /// otherwise → `00 + raw_i16_le`.
    pub fn write_bit_short(&mut self, value: i16) -> Result<(), DwgWriteError> {
        match value {
            0 => self.write_bits(0b10, 2),
            256 => self.write_bits(0b11, 2),
            v if (0..=255).contains(&v) => {
                self.write_bits(0b01, 2)?;
                self.write_raw_u8(v as u8)
            }
            v => {
                self.write_bits(0b00, 2)?;
                self.write_raw_u16_le(v as u16)
            }
        }
    }

    /// DWG BitLong (BL). `0` → `10`, fits in unsigned `u8` →
    /// `01 + raw_u8`, otherwise → `00 + raw_i32_le`. The `11` prefix
    /// is reserved on the reader side; the writer never emits it.
    pub fn write_bit_long(&mut self, value: i32) -> Result<(), DwgWriteError> {
        match value {
            0 => self.write_bits(0b10, 2),
            v if (0..=255).contains(&v) => {
                self.write_bits(0b01, 2)?;
                self.write_raw_u8(v as u8)
            }
            v => {
                self.write_bits(0b00, 2)?;
                self.write_raw_u32_le(v as u32)
            }
        }
    }

    /// DWG BitLongLong (BLL, R24+). 3-bit length prefix `N` followed
    /// by `N` raw little-endian unsigned bytes. The minimum `N` that
    /// fits the value is selected; `0` collapses to `len = 0` with
    /// no payload, matching the reader's fast-path.
    ///
    /// Note that the 3-bit prefix can only represent `0..=7`, so the
    /// largest `BitLongLong` round-trippable through this codec is
    /// `0x00FF_FFFF_FFFF_FFFF` (7 bytes). Values that would need 8
    /// bytes are rejected with [`DwgWriteError::InvalidValue`]; the
    /// reader has the same limit (it cannot return `len = 8` either).
    pub fn write_bit_long_long(&mut self, value: u64) -> Result<(), DwgWriteError> {
        let len = if value == 0 {
            0u8
        } else {
            let used_bits = 64 - value.leading_zeros();
            ((used_bits + 7) / 8) as u8
        };
        if len > 7 {
            return Err(DwgWriteError::InvalidValue(format!(
                "BitLongLong value {value:#x} requires {len} bytes; \
                 the 3-bit length prefix tops out at 7"
            )));
        }
        self.write_bits(len as u64, 3)?;
        for i in 0..len {
            self.write_raw_u8(((value >> (i as u64 * 8)) & 0xFF) as u8)?;
        }
        Ok(())
    }

    /// DWG BitDouble (BD). `0.0` → `10`, `1.0` → `01`, otherwise →
    /// `00 + raw_f64_le`. The `11` prefix is reserved on the reader
    /// side.
    pub fn write_bit_double(&mut self, value: f64) -> Result<(), DwgWriteError> {
        if value == 0.0 {
            // IEEE 754 -0.0 compares equal to 0.0; both round-trip
            // through the literal-zero prefix because the reader
            // unconditionally returns +0.0 for the `10` case.
            self.write_bits(0b10, 2)
        } else if value == 1.0 {
            self.write_bits(0b01, 2)
        } else {
            self.write_bits(0b00, 2)?;
            self.write_raw_f64_le(value)
        }
    }

    /// DWG BitDouble-with-default (DD). Mirror of
    /// [`crate::BitReader::read_bit_double_with_default`]. Encoding
    /// chooses the shortest prefix whose substituted bytes reproduce
    /// `value` from `default`:
    /// * `00` — emitted when `value == default`.
    /// * `01` — bytes 0..=3 of `value` differ; bytes 4..=7 match
    ///   `default`.
    /// * `10` — bytes 0..=5 of `value` differ; bytes 6..=7 match
    ///   `default`. The wire layout is bytes 4..=5 (high half) before
    ///   bytes 0..=3, mirroring the reader's `read_bit_double_with_default`.
    /// * `11` — fallback, full raw double, default ignored.
    pub fn write_bit_double_with_default(
        &mut self,
        value: f64,
        default: f64,
    ) -> Result<(), DwgWriteError> {
        if value.to_bits() == default.to_bits() {
            return self.write_bits(0b00, 2);
        }
        let value_bytes = value.to_le_bytes();
        let default_bytes = default.to_le_bytes();
        // Prefix `01` iff bytes 4..=7 already match the default.
        if value_bytes[4..8] == default_bytes[4..8] {
            self.write_bits(0b01, 2)?;
            for byte in &value_bytes[0..4] {
                self.write_raw_u8(*byte)?;
            }
            return Ok(());
        }
        // Prefix `10` iff bytes 6..=7 already match the default.
        if value_bytes[6..8] == default_bytes[6..8] {
            self.write_bits(0b10, 2)?;
            self.write_raw_u8(value_bytes[4])?;
            self.write_raw_u8(value_bytes[5])?;
            for byte in &value_bytes[0..4] {
                self.write_raw_u8(*byte)?;
            }
            return Ok(());
        }
        self.write_bits(0b11, 2)?;
        self.write_raw_f64_le(value)
    }

    /// DWG 3BD — three consecutive BitDoubles as an `(x, y, z)` triple.
    pub fn write_3bit_double(&mut self, value: [f64; 3]) -> Result<(), DwgWriteError> {
        for component in value {
            self.write_bit_double(component)?;
        }
        Ok(())
    }

    /// DWG 2RD — two raw doubles back-to-back.
    pub fn write_2raw_double(&mut self, value: [f64; 2]) -> Result<(), DwgWriteError> {
        for component in value {
            self.write_raw_f64_le(component)?;
        }
        Ok(())
    }

    /// DWG 2BD — two BitDoubles back-to-back.
    pub fn write_2bit_double(&mut self, value: [f64; 2]) -> Result<(), DwgWriteError> {
        for component in value {
            self.write_bit_double(component)?;
        }
        Ok(())
    }

    /// DWG BitExtrusion (BE), R2000+ encoding only. Mirror of
    /// [`crate::BitReader::read_bit_extrusion_r2000_plus`]. The unit
    /// Z normal `(0, 0, 1)` collapses to a single `1` bit; everything
    /// else writes `0` followed by a 3BD triple.
    pub fn write_bit_extrusion_r2000_plus(&mut self, value: [f64; 3]) -> Result<(), DwgWriteError> {
        if value[0] == 0.0 && value[1] == 0.0 && value[2] == 1.0 {
            self.write_bit(1)
        } else {
            self.write_bit(0)?;
            self.write_3bit_double(value)
        }
    }

    /// DWG BitThickness (BT), R2000+ encoding only. `0.0` collapses
    /// to a single `1` bit; everything else writes `0` followed by a
    /// BD.
    pub fn write_bit_thickness_r2000_plus(&mut self, value: f64) -> Result<(), DwgWriteError> {
        if value == 0.0 {
            self.write_bit(1)
        } else {
            self.write_bit(0)?;
            self.write_bit_double(value)
        }
    }

    /// DWG handle reference (H). Emits the control byte
    /// `(code << 4) | len` followed by `len` raw bytes carrying
    /// `value` in big-endian order — exactly what
    /// [`crate::BitReader::read_handle`] decodes back via
    /// `value = (value << 8) | byte`.
    ///
    /// `code` is the 4-bit handle reference code (`0..=0xF`); `value`
    /// is the absolute (or relative-encoded) handle payload. Length
    /// is the minimum byte count needed to represent `value`, capped
    /// at 8 bytes.
    pub fn write_handle(&mut self, code: u8, value: u64) -> Result<(), DwgWriteError> {
        if code > 0x0F {
            return Err(DwgWriteError::InvalidValue(format!(
                "handle code {code} exceeds 4 bits"
            )));
        }
        let len = if value == 0 {
            0u8
        } else {
            let used_bits = 64 - value.leading_zeros();
            ((used_bits + 7) / 8) as u8
        };
        let control = ((code & 0x0F) << 4) | (len & 0x0F);
        self.write_raw_u8(control)?;
        for i in (0..len).rev() {
            self.write_raw_u8(((value >> (i as u64 * 8)) & 0xFF) as u8)?;
        }
        Ok(())
    }

    /// DWG ASCII text string (T). Mirror of
    /// [`crate::BitReader::read_text_ascii`]. Length prefix is the
    /// BitShort `s.len() + 1`; the trailing `\0` matches the
    /// reader's terminator-stripping behaviour.
    pub fn write_text_ascii(&mut self, s: &str) -> Result<(), DwgWriteError> {
        if !s.is_ascii() {
            return Err(DwgWriteError::InvalidValue(format!(
                "non-ASCII bytes in `{s}` (write_text_ascii)"
            )));
        }
        let payload_len = s.len().checked_add(1).ok_or_else(|| {
            DwgWriteError::InvalidValue("text length overflow when adding null terminator".into())
        })?;
        if payload_len > i16::MAX as usize {
            return Err(DwgWriteError::InvalidValue(format!(
                "text payload length {payload_len} exceeds i16::MAX"
            )));
        }
        self.write_bit_short(payload_len as i16)?;
        for byte in s.as_bytes() {
            self.write_raw_u8(*byte)?;
        }
        self.write_raw_u8(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::BitReader;

    /// Helper: build a writer, run `f`, then turn it into bytes and
    /// re-read with a fresh `BitReader`. Used to express every
    /// roundtrip assertion as `value == read(write(value))`.
    fn roundtrip<W, R, T>(write: W, read: R) -> T
    where
        W: FnOnce(&mut BitWriter),
        R: FnOnce(&mut BitReader<'_>) -> T,
    {
        let mut writer = BitWriter::new();
        write(&mut writer);
        let bytes = writer.into_bytes();
        let mut reader = BitReader::new(&bytes);
        read(&mut reader)
    }

    #[test]
    fn position_in_bits_advances_one_bit_at_a_time() {
        let mut w = BitWriter::new();
        assert_eq!(w.position_in_bits(), 0);
        w.write_bit(1).unwrap();
        assert_eq!(w.position_in_bits(), 1);
        w.write_bits(0b101, 3).unwrap();
        assert_eq!(w.position_in_bits(), 4);
        // align jumps to the next byte boundary, mirroring
        // `BitReader::align_to_byte` (which also advances `byte_offset`).
        w.align_to_byte();
        assert_eq!(w.position_in_bits(), 8);
    }

    #[test]
    fn write_bit_msb_first_then_read_back() {
        let mut w = BitWriter::new();
        for bit in [1, 0, 1, 1, 0, 1, 0, 1] {
            w.write_bit(bit).unwrap();
        }
        let bytes = w.into_bytes();
        assert_eq!(bytes, vec![0xB5]);
    }

    #[test]
    fn write_bits_packs_msb_first() {
        let v = roundtrip(
            |w| {
                w.write_bits(0b10000, 5).unwrap();
                w.write_bits(0b0000, 4).unwrap();
                w.write_bits(0b10, 2).unwrap();
            },
            |r| {
                let a = r.read_bits(5).unwrap();
                let b = r.read_bits(4).unwrap();
                let c = r.read_bits(2).unwrap();
                (a, b, c)
            },
        );
        assert_eq!(v, (0b10000, 0b0000, 0b10));
    }

    #[test]
    fn raw_u8_round_trips() {
        for byte in [0x00u8, 0x01, 0x42, 0xFF] {
            let read_back = roundtrip(
                |w| w.write_raw_u8(byte).unwrap(),
                |r| r.read_raw_u8().unwrap(),
            );
            assert_eq!(read_back, byte, "raw_u8 round-trip failed for {byte:#x}");
        }
    }

    #[test]
    fn raw_u16_le_round_trips() {
        for v in [0u16, 1, 0x1234, 0xFFFF] {
            let r = roundtrip(
                |w| w.write_raw_u16_le(v).unwrap(),
                |r| r.read_raw_u16_le().unwrap(),
            );
            assert_eq!(r, v);
        }
    }

    #[test]
    fn raw_u32_le_round_trips() {
        for v in [0u32, 1, 0xCAFE_BABE, 0xFFFF_FFFF] {
            let r = roundtrip(
                |w| w.write_raw_u32_le(v).unwrap(),
                |r| r.read_raw_u32_le().unwrap(),
            );
            assert_eq!(r, v);
        }
    }

    #[test]
    fn raw_u64_le_round_trips() {
        for v in [0u64, 1, 0xDEAD_BEEF_CAFE_BABE, u64::MAX] {
            let r = roundtrip(
                |w| w.write_raw_u64_le(v).unwrap(),
                |r| r.read_raw_u64_le().unwrap(),
            );
            assert_eq!(r, v);
        }
    }

    #[test]
    fn raw_f64_le_round_trips() {
        for v in [
            0.0,
            -0.0,
            1.0,
            -1.0,
            std::f64::consts::PI,
            f64::MIN,
            f64::MAX,
        ] {
            let r = roundtrip(
                |w| w.write_raw_f64_le(v).unwrap(),
                |r| r.read_raw_f64_le().unwrap(),
            );
            assert_eq!(r.to_bits(), v.to_bits());
        }
    }

    #[test]
    fn bit_short_picks_compact_prefix_for_zero() {
        let mut w = BitWriter::new();
        w.write_bit_short(0).unwrap();
        // Only the 2-bit prefix `10` is emitted, padded with zero
        // bits to the next byte boundary => leading byte `1000_0000`.
        assert_eq!(w.into_bytes(), vec![0b1000_0000]);
    }

    #[test]
    fn bit_short_picks_compact_prefix_for_256() {
        let mut w = BitWriter::new();
        w.write_bit_short(256).unwrap();
        assert_eq!(w.into_bytes(), vec![0b1100_0000]);
    }

    #[test]
    fn bit_short_round_trips_full_range() {
        // Cover both u8 fast-path and full i16 range.
        for v in [
            0i16,
            1,
            42,
            200,
            256,
            257,
            1000,
            0x1234,
            -1,
            -1000,
            i16::MIN,
            i16::MAX,
        ] {
            let r = roundtrip(
                |w| w.write_bit_short(v).unwrap(),
                |r| r.read_bit_short().unwrap(),
            );
            assert_eq!(r, v, "bit_short round-trip failed for {v}");
        }
    }

    #[test]
    fn bit_long_round_trips_full_range() {
        for v in [
            0i32,
            1,
            42,
            255,
            256,
            1000,
            0x1234_5678,
            -1,
            i32::MIN,
            i32::MAX,
        ] {
            let r = roundtrip(
                |w| w.write_bit_long(v).unwrap(),
                |r| r.read_bit_long().unwrap(),
            );
            assert_eq!(r, v, "bit_long round-trip failed for {v}");
        }
    }

    #[test]
    fn bit_long_long_round_trips_supported_range() {
        // 7-byte cap: 0x00FF_FFFF_FFFF_FFFF is the largest value the
        // 3-bit length prefix can describe.
        for v in [
            0u64,
            1,
            0xFF,
            0x100,
            0xFFFF,
            0x1_0000,
            0x00CA_FEBA_BEDE_ADBEu64,
            0x00FF_FFFF_FFFF_FFFFu64,
        ] {
            let r = roundtrip(
                |w| w.write_bit_long_long(v).unwrap(),
                |r| r.read_bit_long_long().unwrap(),
            );
            assert_eq!(r, v, "bit_long_long round-trip failed for {v:#x}");
        }
    }

    #[test]
    fn bit_long_long_rejects_values_needing_eight_bytes() {
        let mut w = BitWriter::new();
        let err = w.write_bit_long_long(u64::MAX).unwrap_err();
        assert!(matches!(err, DwgWriteError::InvalidValue(_)));
    }

    #[test]
    fn bit_double_round_trips_three_paths() {
        // Literal-zero, literal-one and full-double prefixes.
        for v in [0.0, 1.0, std::f64::consts::PI, -42.5, f64::MIN, f64::MAX] {
            let r = roundtrip(
                |w| w.write_bit_double(v).unwrap(),
                |r| r.read_bit_double().unwrap(),
            );
            assert_eq!(
                r.to_bits(),
                v.to_bits(),
                "bit_double round-trip failed for {v}"
            );
        }
    }

    #[test]
    fn bit_double_with_default_round_trips_all_prefixes() {
        let default = std::f64::consts::PI;
        let cases = [
            // value == default → prefix 00
            default,
            // bytes 4..=7 match default; bytes 0..=3 differ → prefix 01
            f64::from_bits((default.to_bits() & 0xFFFF_FFFF_0000_0000) | 0x1234_5678),
            // bytes 6..=7 match default; bytes 0..=5 differ → prefix 10
            f64::from_bits((default.to_bits() & 0xFFFF_0000_0000_0000) | 0x0000_ABCD_1234_5678),
            // all bytes differ → prefix 11
            -default,
            // edge: value differs from default by exactly 1 ULP
            f64::from_bits(default.to_bits() ^ 0x1),
        ];
        for v in cases {
            let r = roundtrip(
                |w| w.write_bit_double_with_default(v, default).unwrap(),
                |r| r.read_bit_double_with_default(default).unwrap(),
            );
            assert_eq!(
                r.to_bits(),
                v.to_bits(),
                "DD round-trip failed for value={v} default={default}"
            );
        }
    }

    #[test]
    fn three_bit_double_round_trips() {
        let cases = [
            [0.0, 0.0, 0.0],
            [1.0, 2.0, 3.0],
            [-1.5, std::f64::consts::E, 1.0],
        ];
        for v in cases {
            let r = roundtrip(
                |w| w.write_3bit_double(v).unwrap(),
                |r| r.read_3bit_double().unwrap(),
            );
            for (got, expect) in r.iter().zip(v.iter()) {
                assert_eq!(got.to_bits(), expect.to_bits());
            }
        }
    }

    #[test]
    fn two_bit_double_round_trips() {
        let r = roundtrip(
            |w| w.write_2bit_double([3.5, -7.25]).unwrap(),
            |r| r.read_2bit_double().unwrap(),
        );
        assert_eq!(r[0], 3.5);
        assert_eq!(r[1], -7.25);
    }

    #[test]
    fn two_raw_double_round_trips() {
        let r = roundtrip(
            |w| w.write_2raw_double([1e10, 2.5e-3]).unwrap(),
            |r| r.read_2raw_double().unwrap(),
        );
        assert_eq!(r[0], 1e10);
        assert_eq!(r[1], 2.5e-3);
    }

    #[test]
    fn bit_extrusion_r2000_plus_unit_z_collapses_to_one_bit() {
        let mut w = BitWriter::new();
        w.write_bit_extrusion_r2000_plus([0.0, 0.0, 1.0]).unwrap();
        assert_eq!(w.position_in_bits(), 1);
        let bytes = w.into_bytes();
        let mut r = BitReader::new(&bytes);
        assert_eq!(r.read_bit_extrusion_r2000_plus().unwrap(), [0.0, 0.0, 1.0]);
    }

    #[test]
    fn bit_extrusion_r2000_plus_arbitrary_normal_round_trips() {
        let normal = [0.5_f64, -0.25, 0.75];
        let r = roundtrip(
            |w| w.write_bit_extrusion_r2000_plus(normal).unwrap(),
            |r| r.read_bit_extrusion_r2000_plus().unwrap(),
        );
        assert_eq!(r, normal);
    }

    #[test]
    fn bit_thickness_r2000_plus_zero_collapses_to_one_bit() {
        let mut w = BitWriter::new();
        w.write_bit_thickness_r2000_plus(0.0).unwrap();
        assert_eq!(w.position_in_bits(), 1);
    }

    #[test]
    fn bit_thickness_r2000_plus_nonzero_round_trips() {
        let r = roundtrip(
            |w| w.write_bit_thickness_r2000_plus(2.5).unwrap(),
            |r| r.read_bit_thickness_r2000_plus().unwrap(),
        );
        assert_eq!(r, 2.5);
    }

    #[test]
    fn handle_round_trips_for_typical_sizes() {
        for (code, value) in [
            (0x0_u8, 0u64),
            (0x2, 0x42),
            (0x5, 0x1234),
            (0x6, 0),
            (0xA, 0xCAFE),
            (0xC, 0x1234_5678),
        ] {
            let r = roundtrip(
                |w| w.write_handle(code, value).unwrap(),
                |r| r.read_handle().unwrap(),
            );
            assert_eq!(
                r,
                (code, value),
                "handle round-trip failed for code={code:#x} value={value:#x}"
            );
        }
    }

    #[test]
    fn handle_max_value_uses_eight_bytes() {
        let r = roundtrip(
            |w| w.write_handle(0x5, u64::MAX).unwrap(),
            |r| r.read_handle().unwrap(),
        );
        assert_eq!(r, (0x5, u64::MAX));
    }

    #[test]
    fn handle_rejects_oversized_code() {
        let mut w = BitWriter::new();
        let err = w.write_handle(0x10, 0).unwrap_err();
        assert!(matches!(err, DwgWriteError::InvalidValue(_)));
    }

    #[test]
    fn text_ascii_round_trips_including_empty_string() {
        for s in ["", "m", "Hello, world!", "AC1015"] {
            let r = roundtrip(
                |w| w.write_text_ascii(s).unwrap(),
                |r| r.read_text_ascii().unwrap(),
            );
            assert_eq!(r, s);
        }
    }

    #[test]
    fn text_ascii_rejects_non_ascii() {
        let mut w = BitWriter::new();
        let err = w.write_text_ascii("café").unwrap_err();
        assert!(matches!(err, DwgWriteError::InvalidValue(_)));
    }

    #[test]
    fn unaligned_writes_pack_correctly() {
        // Verify that an unaligned bit followed by a raw byte round-trips
        // exactly the same way the reader sees it: the byte straddles a
        // bit boundary and the trailing bits are zero-padded.
        let r = roundtrip(
            |w| {
                w.write_bits(0b110, 3).unwrap();
                w.write_raw_u8(0xA5).unwrap();
            },
            |r| {
                let prefix = r.read_bits(3).unwrap();
                let byte = r.read_raw_u8().unwrap();
                (prefix, byte)
            },
        );
        assert_eq!(r, (0b110, 0xA5));
    }

    #[test]
    fn align_to_byte_pads_with_zeros() {
        let mut w = BitWriter::new();
        w.write_bits(0b101, 3).unwrap();
        // Position at 3 bits, current byte holds `1010_0000` so far.
        // After align we advance to the next byte boundary (position 8);
        // the trailing 5 bits stay zero because no further writes
        // touched the partial byte.
        w.align_to_byte();
        assert_eq!(w.position_in_bits(), 8);
        // Write a sentinel byte after alignment and verify it ends up
        // verbatim in byte index 1.
        w.write_raw_u8(0xCC).unwrap();
        let bytes = w.into_bytes();
        assert_eq!(bytes.len(), 2);
        assert_eq!(bytes[0], 0b1010_0000);
        assert_eq!(bytes[1], 0xCC);
    }
}
