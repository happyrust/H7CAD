//! AC1015 common entity header writer (F5.M4.C, minimal config).
//!
//! Inverse of [`crate::parse_ac1015_entity_common`] for the
//! "minimal" subset of AC1015 entities — flagged options chosen so
//! the handle stream contains only two entries (NULL xdictionary +
//! layer handle) and the main stream stays free of EED, graphic,
//! reactor, link, and explicit linetype/plotstyle bytes.
//!
//! See `docs/plans/2026-05-09-dwg-m4c-entity-common-minimal-plan.md`
//! for the field-by-field rationale and the explicit list of
//! `lineweight = -3` / `owner_handle = NULL` known limitations
//! deferred to a follow-on `write_ac1015_entity_common_full` brick.

use crate::bit_writer::BitWriter;
use crate::object_header::HANDLE_CODE_HARD_OWNER;
use crate::DwgWriteError;
use h7cad_native_model::Handle;

/// Caller-supplied common-entity fields the M4.C minimal writer
/// honors. Fields hard-coded to a fixed default at this milestone
/// (EED / graphic / reactor_count / nolinks / linetype_flags /
/// plotstyle_flags / lineweight_index) are not exposed here.
#[derive(Debug, Clone, Copy)]
pub struct EntityCommonMinimal {
    /// Carried for forward compatibility with the future
    /// `write_ac1015_entity_common_full`. **Not encoded** at this
    /// milestone — the writer selects `entity_mode = 0b01`, which
    /// instructs the reader to skip the inline owner reference and
    /// surface `Handle::NULL` for `owner_handle` regardless of the
    /// caller's input.
    pub owner_block_handle: Handle,
    pub layer_handle: Handle,
    pub color_index: i16,
    pub linetype_scale: f64,
    /// Carried for forward compatibility. **Not encoded** at this
    /// milestone — the writer always emits `lineweight_index = 31`
    /// (ByDefault). The reader decodes that to `i16 = -3`.
    pub lineweight: i16,
    pub invisible: bool,
}

/// AC1015 hard-coded `lineweight_index` representing the "ByDefault"
/// sentinel. Reader's `dwg_lineweight_from_index` maps `31 → -3`.
const LINEWEIGHT_INDEX_BY_DEFAULT: u8 = 31;

/// Write the M4.C minimal AC1015 common entity header into the
/// provided `main_writer` / `handle_writer` pair, reproducing the
/// field order [`crate::parse_ac1015_entity_common`] expects.
///
/// The function does **not** prepend any framing (no MS prefix, no
/// object header) — those are produced by [`crate::compose_ac1015_object_slice`]
/// at a higher layer.
pub fn write_ac1015_entity_common_minimal(
    minimal: EntityCommonMinimal,
    main_writer: &mut BitWriter,
    handle_writer: &mut BitWriter,
) -> Result<(), DwgWriteError> {
    // EED loop terminator: the reader walks `read_bit_short()` until
    // it sees a value <= 0, then breaks. A single zero short is
    // therefore the smallest legal terminator.
    main_writer.write_bit_short(0)?;

    // has_graphic = 0 (no graphic image data follows).
    main_writer.write_bit(0)?;

    // entity_mode = 0b01: tells the reader to skip the inline owner
    // handle. `owner_handle = NULL` is the price paid for that
    // simplification (M4.C minimal known limitation).
    main_writer.write_bits(0b01, 2)?;

    // reactor_count = 0.
    main_writer.write_bit_long(0)?;

    // xdictionary handle reference (always read by parse), encoded
    // as a NULL handle (code 5 + value 0). The reader's
    // `consume_optional_handle` returns `None` for `Handle::NULL`,
    // exactly the M4.C minimal contract.
    handle_writer.write_handle(HANDLE_CODE_HARD_OWNER, 0)?;

    // nolinks = 1: skip prev/next entity handles.
    main_writer.write_bit(1)?;

    main_writer.write_bit_short(minimal.color_index)?;
    main_writer.write_bit_double(minimal.linetype_scale)?;

    // layer handle: always read by parse, always emitted here as an
    // explicit absolute reference.
    handle_writer.write_handle(HANDLE_CODE_HARD_OWNER, minimal.layer_handle.value())?;

    // linetype_flags = 0b00 (ByLayer): no linetype handle in the
    // handle stream.
    main_writer.write_bits(0b00, 2)?;

    // plotstyle_flags = 0b00 (ByLayer): no plotstyle handle.
    main_writer.write_bits(0b00, 2)?;

    main_writer.write_bit_short(if minimal.invisible { 1 } else { 0 })?;

    // lineweight: hard-coded ByDefault sentinel (M4.C minimal).
    main_writer.write_raw_u8(LINEWEIGHT_INDEX_BY_DEFAULT)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{parse_ac1015_entity_common, BitReader};
    use h7cad_native_model::Handle;

    /// Drive `parse_ac1015_entity_common` over the writer's output.
    /// The parser expects two independent BitReaders for the main
    /// and handle streams; the test builds them by reading the
    /// finished BitWriters back.
    fn round_trip(
        minimal: EntityCommonMinimal,
        object_handle: Handle,
    ) -> crate::entity_common::Ac1015EntityCommonData {
        let mut main = BitWriter::new();
        let mut handle = BitWriter::new();
        write_ac1015_entity_common_minimal(minimal, &mut main, &mut handle).expect("writes");

        let main_bytes = main.into_bytes();
        let handle_bytes = handle.into_bytes();

        let mut main_reader = BitReader::new(&main_bytes);
        let mut handle_reader = BitReader::new(&handle_bytes);
        parse_ac1015_entity_common(&mut main_reader, &mut handle_reader, object_handle)
            .expect("parser recovers fields")
    }

    #[test]
    fn round_trips_layer_color_linetype_scale_invisible() {
        let minimal = EntityCommonMinimal {
            owner_block_handle: Handle::new(0x100), // ignored at M4.C
            layer_handle: Handle::new(0x10),
            color_index: 7,
            linetype_scale: 0.5,
            lineweight: 13, // ignored at M4.C
            invisible: false,
        };
        let parsed = round_trip(minimal, Handle::new(0x42));
        assert_eq!(parsed.layer_handle, Handle::new(0x10));
        assert_eq!(parsed.color_index, 7);
        assert_eq!(parsed.linetype_scale, 0.5);
        assert_eq!(parsed.invisible, false);
        // M4.C known limitations:
        assert_eq!(
            parsed.owner_handle,
            Handle::NULL,
            "owner_handle is intentionally NULL at M4.C minimal"
        );
        assert_eq!(
            parsed.lineweight, -3,
            "lineweight is intentionally ByDefault (-3) at M4.C minimal"
        );
        // ByLayer linetype: handle is NULL, flags = 0.
        assert_eq!(parsed.linetype_flags, 0);
        assert_eq!(parsed.linetype_handle, Handle::NULL);
    }

    #[test]
    fn round_trips_invisible_flag() {
        for invisible in [false, true] {
            let minimal = EntityCommonMinimal {
                owner_block_handle: Handle::NULL,
                layer_handle: Handle::new(0x20),
                color_index: 1,
                linetype_scale: 1.0,
                lineweight: 0,
                invisible,
            };
            let parsed = round_trip(minimal, Handle::new(0x55));
            assert_eq!(
                parsed.invisible, invisible,
                "round-trip for invisible={invisible}"
            );
        }
    }

    #[test]
    fn round_trips_negative_color_index() {
        // Layers can override entity color via negative ACI; verify
        // the BitShort encoding path covers this case.
        let minimal = EntityCommonMinimal {
            owner_block_handle: Handle::NULL,
            layer_handle: Handle::new(0x30),
            color_index: -7,
            linetype_scale: 1.0,
            lineweight: 0,
            invisible: false,
        };
        let parsed = round_trip(minimal, Handle::new(0x60));
        assert_eq!(parsed.color_index, -7);
    }

    #[test]
    fn round_trips_lineweight_decodes_to_by_default() {
        // Locked known limitation: regardless of the caller's
        // `lineweight` field, the reader sees -3 (ByDefault).
        for caller_lineweight in [-3, -2, -1, 0, 13, 50] {
            let minimal = EntityCommonMinimal {
                owner_block_handle: Handle::NULL,
                layer_handle: Handle::new(0x40),
                color_index: 0,
                linetype_scale: 1.0,
                lineweight: caller_lineweight,
                invisible: false,
            };
            let parsed = round_trip(minimal, Handle::new(0x70));
            assert_eq!(
                parsed.lineweight, -3,
                "M4.C minimal hard-codes lineweight to ByDefault (-3) regardless of caller input ({caller_lineweight})"
            );
        }
    }

    #[test]
    fn round_trips_max_layer_handle_value() {
        // Stress the handle encoding path: a handle requiring the
        // full 8 raw bytes to fit in `u64::MAX`.
        let minimal = EntityCommonMinimal {
            owner_block_handle: Handle::NULL,
            layer_handle: Handle::new(u64::MAX),
            color_index: 0,
            linetype_scale: 1.0,
            lineweight: 0,
            invisible: false,
        };
        let parsed = round_trip(minimal, Handle::new(0x100));
        assert_eq!(parsed.layer_handle, Handle::new(u64::MAX));
    }
}
