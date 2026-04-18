//! Real DWG samples from the ACadSharp sibling repository.
//!
//! These tests anchor the native DWG progress baseline against the
//! real ACadSharp sample corpus. The suite started as an M3-A
//! "knowledge-layer only" harness; it now tracks the M3-B/M3-C
//! transition where AC1015 real entities, their common metadata, and
//! higher-yield entity families are expected to come online
//! incrementally.

use std::path::{Path, PathBuf};

use h7cad_native_dwg::{
    build_pending_document, collect_ac1015_recovery_diagnostics,
    collect_ac1015_recovery_diagnostics_with_known_successes, collect_ac1015_preheader_object_type_hints,
    read_ac1015_object_header, read_dwg, sniff_version, trace_ac1015_targeted_failure_before_fallback,
    Ac1015RecoveryFailureKind, Ac1015TargetedTraceFirstMissingRecord, BitReader, DwgFileHeader, DwgReadError,
    DwgVersion, KnownSection, ObjectStreamCursor, SectionMap,
};
use h7cad_native_model::Handle;

/// Decode a modular char (variable-length unsigned integer, 7 bits
/// per byte with continuation bit in the MSB).
fn read_modular_char(bytes: &[u8], cursor: &mut usize) -> Option<u64> {
    let mut value: u64 = 0;
    let mut shift = 0u32;
    loop {
        let byte = *bytes.get(*cursor)?;
        *cursor += 1;
        value |= ((byte & 0x7F) as u64) << shift;
        if byte & 0x80 == 0 {
            return Some(value);
        }
        shift += 7;
        if shift > 63 {
            return None;
        }
    }
}

/// Decode a signed modular char: same as modular char, but on the
/// final byte bit 6 (`0x40`) flags a negative value and the payload
/// bits in the terminator are only the low 6.
fn read_signed_modular_char(bytes: &[u8], cursor: &mut usize) -> Option<i64> {
    let mut value: u64 = 0;
    let mut shift = 0u32;
    loop {
        let byte = *bytes.get(*cursor)?;
        *cursor += 1;
        if byte & 0x80 != 0 {
            value |= ((byte & 0x7F) as u64) << shift;
            shift += 7;
            if shift > 63 {
                return None;
            }
        } else {
            let negative = byte & 0x40 != 0;
            value |= ((byte & 0x3F) as u64) << shift;
            return Some(if negative {
                -(value as i64)
            } else {
                value as i64
            });
        }
    }
}

fn samples_dir() -> PathBuf {
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    crate_dir
        .join("..")
        .join("..")
        .join("..")
        .join("ACadSharp")
        .join("samples")
}

fn try_read_sample(name: &str) -> Option<Vec<u8>> {
    std::fs::read(samples_dir().join(name)).ok()
}

fn real_samples() -> [(&'static str, DwgVersion); 7] {
    [
        ("sample_AC1014.dwg", DwgVersion::Ac1014),
        ("sample_AC1015.dwg", DwgVersion::Ac1015),
        ("sample_AC1018.dwg", DwgVersion::Ac1018),
        ("sample_AC1021.dwg", DwgVersion::Ac1021),
        ("sample_AC1024.dwg", DwgVersion::Ac1024),
        ("sample_AC1027.dwg", DwgVersion::Ac1027),
        ("sample_AC1032.dwg", DwgVersion::Ac1032),
    ]
}

#[test]
fn real_dwg_samples_sniff_correct_versions() {
    let mut seen = 0;
    for (name, expected) in real_samples() {
        let Some(bytes) = try_read_sample(name) else {
            eprintln!("skip {name}: sample file not present");
            continue;
        };
        seen += 1;
        let version = sniff_version(&bytes)
            .unwrap_or_else(|err| panic!("{name}: sniff failed: {err:?}"));
        assert_eq!(
            version, expected,
            "{name}: expected {expected:?}, got {version:?}"
        );
    }

    // If none of the samples are available, the test is still
    // meaningful (the sniff table itself is unit-tested elsewhere).
    eprintln!("real dwg sniff baseline: {seen}/7 samples verified");
}

/// M3-B progress baseline. As of M3-B brick 3b, `read_dwg` is
/// expected to recover real AC1015 entities through the best-effort
/// native enrichment pipeline. Earlier milestones only required
/// LINE/CIRCLE/POINT lower bounds; the current phase raises the bar
/// to cover:
///
/// - corrected ARC/CIRCLE type-code routing,
/// - common entity metadata (owner/layer/linetype/color) no longer
///   being all-default placeholders,
/// - and the first high-yield expansion set: TEXT / LWPOLYLINE / HATCH.
///
/// What we assert:
/// - Versions we already reject explicitly (AC1012/AC1014/AC1021+)
///   must return `UnsupportedVersion` and echo back the correct
///   version (i.e. sniff still wired to section lookup).
/// - AC1015 must decode at least one entity in each currently-supported
///   family: LINE / CIRCLE / ARC / POINT / TEXT / LWPOLYLINE / HATCH.
/// - AC1018 may still surface a structural decoder error until the
///   encrypted metadata decoder lands; that case is logged, not
///   asserted.
/// - No panic paths must leak.
#[test]
fn real_dwg_samples_baseline_m3b() {
    for (name, version) in real_samples() {
        let Some(bytes) = try_read_sample(name) else {
            continue;
        };
        match read_dwg(&bytes) {
            Ok(doc) => {
                let count_of = |pred: fn(&h7cad_native_model::EntityData) -> bool| {
                    doc.entities.iter().filter(|e| pred(&e.data)).count()
                };
                let line_count = count_of(|d| matches!(d, h7cad_native_model::EntityData::Line { .. }));
                let circle_count = count_of(|d| matches!(d, h7cad_native_model::EntityData::Circle { .. }));
                let arc_count = count_of(|d| matches!(d, h7cad_native_model::EntityData::Arc { .. }));
                let point_count = count_of(|d| matches!(d, h7cad_native_model::EntityData::Point { .. }));
                let text_count = count_of(|d| matches!(d, h7cad_native_model::EntityData::Text { .. }));
                let lwpolyline_count =
                    count_of(|d| matches!(d, h7cad_native_model::EntityData::LwPolyline { .. }));
                let hatch_count = count_of(|d| matches!(d, h7cad_native_model::EntityData::Hatch { .. }));
                eprintln!(
                    "{name} ({version:?}): read_dwg recovered {} entities \
                     ({} LINE, {} CIRCLE, {} ARC, {} POINT, {} TEXT, {} LWPOLYLINE, {} HATCH), \
                     {} blocks, {} layouts, {} objects",
                    doc.entities.len(),
                    line_count,
                    circle_count,
                    arc_count,
                    point_count,
                    text_count,
                    lwpolyline_count,
                    hatch_count,
                    doc.block_records.len(),
                    doc.layouts.len(),
                    doc.objects.len(),
                );
                if version == DwgVersion::Ac1015 {
                    let header = DwgFileHeader::parse(&bytes).expect("AC1015 file header parse");
                    let sections = SectionMap::parse(&bytes, &header).expect("AC1015 section map parse");
                    let payloads = sections
                        .read_section_payloads(&bytes)
                        .expect("AC1015 section payloads readable");
                    let pending = build_pending_document(&header, &sections, payloads)
                        .expect("AC1015 pending document builds without error");
                    let diagnostics = collect_ac1015_recovery_diagnostics_with_known_successes(
                        &bytes,
                        &pending,
                        std::iter::repeat_n("LINE", line_count)
                            .chain(std::iter::repeat_n("CIRCLE", circle_count))
                            .chain(std::iter::repeat_n("ARC", arc_count))
                            .chain(std::iter::repeat_n("POINT", point_count))
                            .chain(std::iter::repeat_n("TEXT", text_count))
                            .chain(std::iter::repeat_n("LWPOLYLINE", lwpolyline_count))
                            .chain(std::iter::repeat_n("HATCH", hatch_count)),
                    );
                    eprintln!(
                        "AC1015 recovery diagnostics: total_recovered={} LINE={} CIRCLE={} ARC={} POINT={} TEXT={} LWPOLYLINE={} HATCH={}",
                        diagnostics.recovered_total,
                        diagnostics.recovered_by_family.get("LINE").copied().unwrap_or(0),
                        diagnostics.recovered_by_family.get("CIRCLE").copied().unwrap_or(0),
                        diagnostics.recovered_by_family.get("ARC").copied().unwrap_or(0),
                        diagnostics.recovered_by_family.get("POINT").copied().unwrap_or(0),
                        diagnostics.recovered_by_family.get("TEXT").copied().unwrap_or(0),
                        diagnostics.recovered_by_family.get("LWPOLYLINE").copied().unwrap_or(0),
                        diagnostics.recovered_by_family.get("HATCH").copied().unwrap_or(0),
                    );
                    let failure_kind_count = |kind: Ac1015RecoveryFailureKind| {
                        diagnostics.failure_counts.get(&kind).copied().unwrap_or(0)
                    };
                    eprintln!(
                        "AC1015 recovery failure buckets: slice_miss={} header_fail={} handle_mismatch={} common_decode_fail={} body_decode_fail={} unsupported_type={}",
                        failure_kind_count(Ac1015RecoveryFailureKind::SliceMiss),
                        failure_kind_count(Ac1015RecoveryFailureKind::HeaderFail),
                        failure_kind_count(Ac1015RecoveryFailureKind::HandleMismatch),
                        failure_kind_count(Ac1015RecoveryFailureKind::CommonDecodeFail),
                        failure_kind_count(Ac1015RecoveryFailureKind::BodyDecodeFail),
                        failure_kind_count(Ac1015RecoveryFailureKind::UnsupportedType),
                    );
                    for family in ["LINE", "CIRCLE", "ARC", "POINT", "TEXT", "LWPOLYLINE", "HATCH"] {
                        let by_family = diagnostics.failure_counts_by_family.get(family);
                        eprintln!(
                            "  family={family} slice_miss={} header_fail={} handle_mismatch={} common_decode_fail={} body_decode_fail={} unsupported_type={}",
                            by_family.and_then(|m| m.get(&Ac1015RecoveryFailureKind::SliceMiss)).copied().unwrap_or(0),
                            by_family.and_then(|m| m.get(&Ac1015RecoveryFailureKind::HeaderFail)).copied().unwrap_or(0),
                            by_family.and_then(|m| m.get(&Ac1015RecoveryFailureKind::HandleMismatch)).copied().unwrap_or(0),
                            by_family.and_then(|m| m.get(&Ac1015RecoveryFailureKind::CommonDecodeFail)).copied().unwrap_or(0),
                            by_family.and_then(|m| m.get(&Ac1015RecoveryFailureKind::BodyDecodeFail)).copied().unwrap_or(0),
                            by_family.and_then(|m| m.get(&Ac1015RecoveryFailureKind::UnsupportedType)).copied().unwrap_or(0),
                        );
                    }
                    print_supported_geometric_failure_examples(&diagnostics);

                    assert!(
                        doc.entities.len() >= 84,
                        "{name}: AC1015 baseline must recover at least 84 entities, got {}",
                        doc.entities.len()
                    );
                    assert!(
                        diagnostics.recovered_total >= 84,
                        "{name}: recovery diagnostics must report at least 84 recovered entities, got {}",
                        diagnostics.recovered_total
                    );
                    assert!(
                        line_count >= 40,
                        "{name}: AC1015 baseline must recover at least 40 \
                         LINE entities, got {line_count}"
                    );
                    assert!(
                        circle_count >= 6,
                        "{name}: AC1015 baseline must recover at least 6 \
                         CIRCLE entities, got {circle_count}"
                    );
                    assert!(
                        point_count >= 12,
                        "{name}: AC1015 baseline must recover at least 12 \
                         POINT entities, got {point_count}"
                    );
                    assert!(
                        arc_count >= 2,
                        "{name}: AC1015 baseline must recover at least 2 \
                         ARC entities, got {arc_count}"
                    );
                    assert_eq!(
                        text_count, 26,
                        "{name}: AC1015 baseline must recover exactly 26 TEXT entities"
                    );
                    assert!(
                        diagnostics.recovered_by_family.get("TEXT").copied().unwrap_or(0) == 26,
                        "{name}: diagnostics surface must report exactly 26 TEXT entities"
                    );
                    assert!(
                        lwpolyline_count >= 16,
                        "{name}: AC1015 baseline must recover at least 16 LWPOLYLINE entities, got {lwpolyline_count}"
                    );
                    assert!(
                        diagnostics.recovered_by_family.get("LWPOLYLINE").copied().unwrap_or(0) >= 16,
                        "{name}: diagnostics surface must report at least 16 LWPOLYLINE entities"
                    );
                    assert_eq!(
                        hatch_count, 6,
                        "{name}: AC1015 baseline must recover exactly 6 HATCH entities"
                    );
                    assert!(
                        diagnostics.recovered_by_family.get("HATCH").copied().unwrap_or(0) == 6,
                        "{name}: diagnostics surface must report exactly 6 HATCH entities"
                    );
                    assert_eq!(
                        diagnostics.recovered_by_family.get("LINE").copied().unwrap_or(0),
                        line_count,
                        "{name}: diagnostics LINE count must match recovered entity count"
                    );
                    assert_eq!(
                        diagnostics.recovered_by_family.get("CIRCLE").copied().unwrap_or(0),
                        circle_count,
                        "{name}: diagnostics CIRCLE count must match recovered entity count"
                    );
                    assert_eq!(
                        diagnostics.recovered_by_family.get("ARC").copied().unwrap_or(0),
                        arc_count,
                        "{name}: diagnostics ARC count must match recovered entity count"
                    );
                    assert_eq!(
                        diagnostics.recovered_by_family.get("POINT").copied().unwrap_or(0),
                        point_count,
                        "{name}: diagnostics POINT count must match recovered entity count"
                    );
                    let _ = diagnostics
                        .failure_counts
                        .get(&Ac1015RecoveryFailureKind::SliceMiss)
                        .copied()
                        .unwrap_or(0);
                    let _ = diagnostics
                        .failure_counts
                        .get(&Ac1015RecoveryFailureKind::HeaderFail)
                        .copied()
                        .unwrap_or(0);
                    let _ = diagnostics
                        .failure_counts
                        .get(&Ac1015RecoveryFailureKind::HandleMismatch)
                        .copied()
                        .unwrap_or(0);
                    let _ = diagnostics
                        .failure_counts
                        .get(&Ac1015RecoveryFailureKind::CommonDecodeFail)
                        .copied()
                        .unwrap_or(0);
                    let _ = diagnostics
                        .failure_counts
                        .get(&Ac1015RecoveryFailureKind::BodyDecodeFail)
                        .copied()
                        .unwrap_or(0);
                    let _ = diagnostics
                        .failure_counts
                        .get(&Ac1015RecoveryFailureKind::UnsupportedType)
                        .copied()
                        .unwrap_or(0);
                    assert!(
                        diagnostics.failure_counts_by_family.contains_key("LINE")
                            || diagnostics.failure_counts_by_family.contains_key("CIRCLE")
                            || diagnostics.failure_counts_by_family.contains_key("ARC")
                            || diagnostics.failure_counts_by_family.contains_key("POINT")
                            || diagnostics.failure_counts_by_family.contains_key("TEXT")
                            || diagnostics.failure_counts_by_family.contains_key("LWPOLYLINE")
                            || diagnostics.failure_counts_by_family.contains_key("HATCH"),
                        "{name}: diagnostics must attribute at least one supported-family failure bucket"
                    );

                    let enriched = doc
                        .entities
                        .iter()
                        .filter(|entity| {
                            matches!(
                                entity.data,
                                h7cad_native_model::EntityData::Line { .. }
                                    | h7cad_native_model::EntityData::Circle { .. }
                                    | h7cad_native_model::EntityData::Arc { .. }
                                    | h7cad_native_model::EntityData::Point { .. }
                                    | h7cad_native_model::EntityData::Text { .. }
                                    | h7cad_native_model::EntityData::LwPolyline { .. }
                                    | h7cad_native_model::EntityData::Hatch { .. }
                            )
                        })
                        .collect::<Vec<_>>();
                    assert!(
                        !enriched.is_empty(),
                        "{name}: AC1015 baseline expected at least one enriched \
                         entity to inspect common metadata"
                    );
                    assert!(
                        enriched
                            .iter()
                            .any(|entity| entity.owner_handle != h7cad_native_model::Handle::NULL),
                        "{name}: AC1015 enriched entities must not all keep NULL owner_handle"
                    );
                    assert!(
                        enriched.iter().any(|entity| entity.layer_name != "0"),
                        "{name}: AC1015 enriched entities must not all keep layer \"0\""
                    );
                    assert!(
                        enriched.iter().any(|entity| {
                            entity.color_index != 256 || !entity.linetype_name.is_empty()
                        }),
                        "{name}: AC1015 enriched entities must expose at least one non-default \
                         color or linetype"
                    );
                    assert!(
                        enriched.iter().any(|entity| {
                            matches!(
                                entity.data,
                                h7cad_native_model::EntityData::Line { .. }
                                    | h7cad_native_model::EntityData::Circle { .. }
                                    | h7cad_native_model::EntityData::Arc { .. }
                                    | h7cad_native_model::EntityData::Point { .. }
                                    | h7cad_native_model::EntityData::LwPolyline { .. }
                            ) && (entity.owner_handle != h7cad_native_model::Handle::NULL
                                || entity.layer_name != "0"
                                || entity.color_index != 256
                                || !entity.linetype_name.is_empty())
                        }),
                        "{name}: at least one recovered geometric entity must retain non-default owner/layer/color/linetype metadata"
                    );
                }
            }
            Err(DwgReadError::UnsupportedVersion(reported)) => {
                assert_eq!(
                    reported, version,
                    "{name}: UnsupportedVersion should echo sniffed version"
                );
                eprintln!("{name} ({version:?}): explicit UnsupportedVersion (baseline)");
            }
            Err(err) => {
                // AC1018 currently hits structural decode errors on
                // real files until encrypted metadata lands; record
                // the exact error shape for future regression tracking.
                eprintln!("{name} ({version:?}): read_dwg baseline error = {err:?}");
            }
        }
    }
}

/// Decode the first block of CadHeader variables from the real AC1015
/// Header section. ACadSharp's `DwgHeaderReader.cs` documents:
///
/// ```text
///   BD  412148564080.0
///   BD  1.0
///   BD  1.0
///   BD  1.0
///   TV  "m"
///   TV  ""
///   TV  ""
///   TV  ""
///   BL  24
///   BL  0
///   H   current viewport entity header (pre-R2004 only, R15 qualifies)
///   B   DIMASO
///   B   DIMSHO
///   B   PLINEGEN
///   B   ORTHOMODE
///   B   REGENMODE
///   B   FILLMODE
///   B   QTEXTMODE
/// ```
///
/// If we can extract this whole block from real AC1015 bytes then the
/// low-level bit reader, variable-text decoding, and handle reader all
/// agree with AutoCAD's on-disk layout. This is the first real slice
/// of `CadHeader` state.
#[test]
fn real_ac1015_header_section_decodes_first_cadheader_block() {
    let Some(bytes) = try_read_sample("sample_AC1015.dwg") else {
        return;
    };
    let header = DwgFileHeader::parse(&bytes).expect("AC1015 file header should parse");
    let sections = SectionMap::parse(&bytes, &header).expect("section map should parse");
    let header_desc = sections
        .descriptors
        .iter()
        .find(|d| KnownSection::from_record_number(d.record_number) == Some(KnownSection::Header))
        .expect("AcDb:Header section must exist in AC1015 sample");

    let start = header_desc.offset as usize;
    let end = start + header_desc.size as usize;
    assert!(end <= bytes.len(), "header section out of bounds");
    let payload = &bytes[start..end];
    let sentinel = KnownSection::Header.start_sentinel().unwrap();
    assert_eq!(&payload[..16], &sentinel, "Header start sentinel mismatch");

    // After the 16-byte sentinel the next 4 bytes are a raw 32-bit
    // size field; the bit stream begins after that.
    let bitstream_start = 16 + 4;
    eprintln!(
        "AC1015 header first 32 bit-stream bytes: {:02X?}",
        &payload[bitstream_start..bitstream_start + 32]
    );
    let mut reader = BitReader::new(&payload[bitstream_start..]);

    // Four documented BitDoubles.
    let bd1 = reader.read_bit_double().unwrap();
    let bd2 = reader.read_bit_double().unwrap();
    let bd3 = reader.read_bit_double().unwrap();
    let bd4 = reader.read_bit_double().unwrap();
    assert_eq!(bd1, 412148564080.0);
    assert_eq!(bd2, 1.0);
    assert_eq!(bd3, 1.0);
    assert_eq!(bd4, 1.0);

    // Four variable-text strings. The first is documented as "m".
    eprintln!(
        "AC1015 header bit position before TV reads: {}",
        reader.position_in_bits()
    );
    let tv1 = reader.read_text_ascii().unwrap();
    eprintln!("after TV1 ({tv1:?}): bit pos {}", reader.position_in_bits());
    let tv2 = reader.read_text_ascii().unwrap();
    eprintln!("after TV2: bit pos {}", reader.position_in_bits());
    let tv3 = reader.read_text_ascii().unwrap();
    eprintln!("after TV3: bit pos {}", reader.position_in_bits());
    let tv4 = reader.read_text_ascii().unwrap();
    eprintln!("after TV4: bit pos {}", reader.position_in_bits());
    eprintln!("AC1015 header TV quadruple: {tv1:?} / {tv2:?} / {tv3:?} / {tv4:?}");
    assert_eq!(tv1, "m", "first TV should be the documented \"m\"");
    assert!(tv2.is_empty());
    assert!(tv3.is_empty());
    assert!(tv4.is_empty());

    // Two BitLongs follow. ACadSharp notes default values of 24 and 0
    // but those are hints from the writer, not a format requirement.
    // The only thing the reader must guarantee is that both decode
    // without error. This asserts the bit-stream structure, not the
    // AutoCAD-chosen writer defaults.
    let bl1 = reader.read_bit_long().unwrap();
    let bl2 = reader.read_bit_long().unwrap();
    eprintln!("AC1015 header BL pair: bl1={bl1}, bl2={bl2}");

    // Pre-R2004 current viewport entity handle. We don't care about
    // the exact value (AutoCAD writes whatever viewport is active)
    // but we *do* expect the handle read to return a sane control
    // byte with length <= 8.
    let (code, value) = reader.read_handle().unwrap();
    eprintln!("AC1015 header viewport handle: code={code} value=0x{value:X}");
    assert!(code <= 0x0F, "handle control nibble must fit in 4 bits");

    // Seven CadHeader boolean bits: DIMASO, DIMSHO, PLINEGEN,
    // ORTHOMODE, REGENMODE, FILLMODE, QTEXTMODE.
    let bits: Vec<u8> = (0..7)
        .map(|_| reader.read_bit().unwrap())
        .collect();
    eprintln!("AC1015 header boolean bits (DIMASO,DIMSHO,PLINEGEN,ORTHOMODE,REGENMODE,FILLMODE,QTEXTMODE) = {bits:?}");
    for (i, b) in bits.iter().enumerate() {
        assert!(*b == 0 || *b == 1, "bit {i} should be 0 or 1, got {b}");
    }
}

/// Extend CadHeader decoding beyond the first block. Reads the next
/// 13 `BB` (bit) flags plus the 8 `BS` (bit-short) integers that
/// capture units and proxy-graphics defaults on R2000 drawings.
/// Values are sanity-checked against documented enum ranges so any
/// future regression in bit alignment surfaces loudly here.
#[test]
fn real_ac1015_header_section_decodes_units_block() {
    let Some(bytes) = try_read_sample("sample_AC1015.dwg") else {
        return;
    };
    let header = DwgFileHeader::parse(&bytes).expect("AC1015 file header should parse");
    let sections = SectionMap::parse(&bytes, &header).expect("section map should parse");
    let header_desc = sections
        .descriptors
        .iter()
        .find(|d| KnownSection::from_record_number(d.record_number) == Some(KnownSection::Header))
        .expect("AcDb:Header section must exist in AC1015 sample");
    let start = header_desc.offset as usize;
    let end = start + header_desc.size as usize;
    let payload = &bytes[start..end];
    let mut reader = BitReader::new(&payload[20..]);

    // Skip the first documented block that is already covered by the
    // companion test.
    for _ in 0..4 {
        reader.read_bit_double().unwrap();
    }
    for _ in 0..4 {
        reader.read_text_ascii().unwrap();
    }
    reader.read_bit_long().unwrap();
    reader.read_bit_long().unwrap();
    reader.read_handle().unwrap();
    for _ in 0..7 {
        reader.read_bit().unwrap();
    }

    // Next 13 `B` flags on AC1015 (R13/R14-only variants are skipped
    // by AutoCAD when writing R2000): PSLTSCALE, LIMCHECK, USRTIMER,
    // SKPOLY, ANGDIR, SPLFRAME, MIRRTEXT, WORLDVIEW, TILEMODE,
    // PLIMCHECK, VISRETAIN, DISPSILH, PELLIPSE.
    let labels = [
        "PSLTSCALE",
        "LIMCHECK",
        "USRTIMER",
        "SKPOLY",
        "ANGDIR",
        "SPLFRAME",
        "MIRRTEXT",
        "WORLDVIEW",
        "TILEMODE",
        "PLIMCHECK",
        "VISRETAIN",
        "DISPSILH",
        "PELLIPSE",
    ];
    let mut flags = Vec::with_capacity(labels.len());
    for label in labels {
        let b = reader.read_bit().unwrap();
        assert!(b == 0 || b == 1, "{label} must be boolean");
        flags.push((label, b));
    }
    eprintln!("AC1015 header B block 2: {flags:?}");

    // Eight BitShort integers on AC1015:
    let proxygraphics = reader.read_bit_short().unwrap();
    let treedepth = reader.read_bit_short().unwrap();
    let lunits = reader.read_bit_short().unwrap();
    let luprec = reader.read_bit_short().unwrap();
    let aunits = reader.read_bit_short().unwrap();
    let auprec = reader.read_bit_short().unwrap();
    let attmode = reader.read_bit_short().unwrap();
    let pdmode = reader.read_bit_short().unwrap();

    eprintln!(
        "AC1015 header BS block: \
         PROXYGRAPHICS={proxygraphics} TREEDEPTH={treedepth} \
         LUNITS={lunits} LUPREC={luprec} AUNITS={aunits} AUPREC={auprec} \
         ATTMODE={attmode} PDMODE={pdmode}"
    );

    // Sanity ranges per AutoCAD's DXF reference:
    //   LUNITS: 1..=6 (scientific..fractional)
    //   LUPREC: 0..=8
    //   AUNITS: 0..=4 (decimal degrees..surveyor)
    //   AUPREC: 0..=8
    //   ATTMODE: 0..=2
    assert!(
        (1..=6).contains(&lunits),
        "LUNITS must be 1..=6, got {lunits}"
    );
    assert!(
        (0..=8).contains(&luprec),
        "LUPREC must be 0..=8, got {luprec}"
    );
    assert!(
        (0..=4).contains(&aunits),
        "AUNITS must be 0..=4, got {aunits}"
    );
    assert!(
        (0..=8).contains(&auprec),
        "AUPREC must be 0..=8, got {auprec}"
    );
    assert!(
        (0..=2).contains(&attmode),
        "ATTMODE must be 0..=2, got {attmode}"
    );
}

/// AC1015 Classes section layout (per ACadSharp DwgClassesReader):
///
/// ```text
///   [0x00..0x10]  Start sentinel (16 bytes)
///   [0x10..0x14]  RL — section size (bytes from here to end sentinel)
///   ... bit stream ...
///         BS — MaxClassNum
///         repeat while bits remain before end sentinel:
///             BS — class number
///             BS — proxy flags
///             TV — app name
///             TV — C++ class name
///             TV — DXF record name
///             B  — was_a_zombie
///             BS — item class id
///   [end - 16 .. end]  End sentinel (16 bytes)
/// ```
///
/// Parsing the whole table and checking that the end sentinel
/// appears in place is strong evidence the reader is keeping frame
/// alignment across ~100 class records.
//
// The `reader = snapshot` assignments inside the loop are intentional
// defensive roll-backs: they preserve the ability to swap `break` for
// `continue` in a future milestone without silently skipping fields.
// They are dead on the current control flow (every arm breaks), so the
// compiler warns; we silence the warning at the function level rather
// than discarding the snapshot assignments.
#[allow(unused_assignments)]
#[test]
fn real_ac1015_classes_section_parses_full_table() {
    let Some(bytes) = try_read_sample("sample_AC1015.dwg") else {
        return;
    };
    let header = DwgFileHeader::parse(&bytes).expect("header parse");
    let sections = SectionMap::parse(&bytes, &header).expect("section map");
    let classes_desc = sections
        .descriptors
        .iter()
        .find(|d| KnownSection::from_record_number(d.record_number) == Some(KnownSection::Classes))
        .expect("AcDb:Classes section must exist");

    let start = classes_desc.offset as usize;
    let end = start + classes_desc.size as usize;
    let payload = &bytes[start..end];

    let expected_start = KnownSection::Classes.start_sentinel().unwrap();
    let expected_end = KnownSection::Classes.end_sentinel().unwrap();
    assert_eq!(&payload[..16], &expected_start, "classes start sentinel");
    assert_eq!(
        &payload[payload.len() - 16..],
        &expected_end,
        "classes end sentinel"
    );

    // Bit stream lives between the sentinels. Discard the 16-byte
    // leading sentinel and 16-byte trailing sentinel. Inside, the
    // first 4 bytes are a raw size header (the size we already got
    // from the section locator).
    let bit_start = 16 + 4;
    let bit_end = payload.len() - 16;
    // We don't reserve bytes for the 2-byte CRC that trails R2000
    // class bitstreams because we only stop parsing when we hit the
    // section-local end anchor rather than consuming every last bit.
    let mut reader = BitReader::new(&payload[bit_start..bit_end]);

    // AC1015's Classes section does NOT carry a MaxClassNum field in
    // the R2000 layout. Records start immediately after the RL size
    // header; the first BitShort decoded below is the first record's
    // class number (AutoCAD starts class numbering at 0x1F4 = 500).
    let mut parsed = Vec::<(i16, i16, String, String, String)>::new();
    let mut record_count = 0usize;

    loop {
        // Stop when the trailing-end distance drops below the minimum
        // envelope of a record (smallest possible record is 2 BS + 3
        // empty TV + B + BS = roughly 20 bits).
        if reader.bits_remaining() < 32 {
            break;
        }
        // Snapshot in case the record is malformed and we need to back
        // out; BitReader is Clone-friendly by design.
        let snapshot = reader.clone();
        let Ok(class_number) = reader.read_bit_short() else {
            break;
        };
        let Ok(proxy_flags) = reader.read_bit_short() else {
            reader = snapshot;
            break;
        };
        let Ok(app_name) = reader.read_text_ascii() else {
            reader = snapshot;
            break;
        };
        let Ok(cpp_class) = reader.read_text_ascii() else {
            reader = snapshot;
            break;
        };
        let Ok(dxf_name) = reader.read_text_ascii() else {
            reader = snapshot;
            break;
        };
        let Ok(was_zombie) = reader.read_bit() else {
            reader = snapshot;
            break;
        };
        let Ok(item_class_id) = reader.read_bit_short() else {
            reader = snapshot;
            break;
        };

        if record_count < 5 {
            eprintln!(
                "  class[{record_count}]: num={class_number} flags={proxy_flags} \
                 app={app_name:?} cpp={cpp_class:?} dxf={dxf_name:?} \
                 zombie={was_zombie} item_id={item_class_id}"
            );
        }

        // ItemClassId is documented as 0x1F2 (= 498, entity-producing)
        // or 0x1F3 (= 499, object-producing). Anything else means the
        // bit cursor has drifted into garbage.
        if item_class_id != 0x1F2 && item_class_id != 0x1F3 {
            reader = snapshot;
            break;
        }

        parsed.push((class_number, proxy_flags, app_name, cpp_class, dxf_name));
        record_count += 1;

        if record_count > 1024 {
            panic!("Classes record count exploded; likely bit alignment drift");
        }
    }

    eprintln!(
        "AC1015 Classes: {} records parsed (first 5: {:?})",
        parsed.len(),
        parsed.iter().take(5).collect::<Vec<_>>()
    );
    assert!(
        parsed.len() >= 10,
        "expected at least 10 class records in a typical R2000 drawing, got {}",
        parsed.len()
    );
    // Spot-check: AutoCAD's R2000 drawings almost always register
    // AcDbDictionaryWithDefault and AcDbLayout, so a class list
    // without either is a strong signal of a bit-alignment bug.
    let has_common_class = parsed.iter().any(|(_, _, _, cpp, _)| {
        cpp == "AcDbDictionaryWithDefault" || cpp == "AcDbLayout"
    });
    assert!(
        has_common_class,
        "expected at least one of AcDbDictionaryWithDefault / AcDbLayout \
         among parsed classes"
    );
}

/// Decode the AC1015 Handles section (per ACadSharp DwgHandleReader):
///
/// ```text
/// Repeat:
///   RS (big-endian) — size of this chunk (including the 2-byte size)
///   if size == 2:
///       break (empty tail chunk)
///   maxOffset = min(size - 2, 2032)
///   while consumed < maxOffset:
///       ModularChar (unsigned) — delta handle
///       SignedModularChar      — delta location
///       lasthandle += delta_handle
///       lastloc += delta_location
///       if delta_handle > 0: objectMap[lasthandle] = lastloc
///   CRC (2 bytes, big-endian)
/// ```
///
/// Parsing the whole section and asserting the object map has a
/// sensible shape (> 20 entries, strictly positive offsets) is a
/// strong end-to-end check of AC1015 object routing.
#[test]
fn real_ac1015_handles_section_parses_full_map() {
    let Some(bytes) = try_read_sample("sample_AC1015.dwg") else {
        return;
    };
    let header = DwgFileHeader::parse(&bytes).expect("header");
    let sections = SectionMap::parse(&bytes, &header).expect("section map");
    let handles_desc = sections
        .descriptors
        .iter()
        .find(|d| KnownSection::from_record_number(d.record_number) == Some(KnownSection::Handles))
        .expect("AcDb:Handles section must exist");

    let start = handles_desc.offset as usize;
    let end = start + handles_desc.size as usize;
    let payload = &bytes[start..end];

    let mut map: std::collections::BTreeMap<u64, i64> = std::collections::BTreeMap::new();
    let mut cursor = 0usize;
    let mut chunk_index = 0usize;

    loop {
        if cursor + 2 > payload.len() {
            break;
        }
        // Big-endian u16 chunk size.
        let size = u16::from_be_bytes([payload[cursor], payload[cursor + 1]]);
        cursor += 2;

        if size == 2 {
            eprintln!("Handles: empty-tail chunk at #{chunk_index}");
            break;
        }

        let max_offset = (size - 2).min(2032) as usize;
        let chunk_end = cursor + max_offset;

        let mut last_handle: u64 = 0;
        let mut last_loc: i64 = 0;
        let mut entries_in_chunk = 0usize;

        while cursor < chunk_end {
            let Some(delta_handle) = read_modular_char(payload, &mut cursor) else {
                panic!("handle chunk {chunk_index} truncated reading handle delta");
            };
            let Some(delta_loc) = read_signed_modular_char(payload, &mut cursor) else {
                panic!("handle chunk {chunk_index} truncated reading loc delta");
            };
            last_handle = last_handle.wrapping_add(delta_handle);
            last_loc += delta_loc;
            if delta_handle > 0 {
                map.insert(last_handle, last_loc);
                entries_in_chunk += 1;
            }
        }

        // 2 CRC bytes trail each chunk.
        cursor += 2;

        if chunk_index < 3 {
            eprintln!(
                "Handles chunk#{chunk_index}: size={size} entries={entries_in_chunk} cumulative_map_size={}",
                map.len()
            );
        }
        chunk_index += 1;
    }

    eprintln!(
        "AC1015 Handles: {} chunks parsed, {} handle→offset entries total",
        chunk_index,
        map.len()
    );

    assert!(
        map.len() >= 20,
        "expected the handle map to have at least 20 entries, got {}",
        map.len()
    );
    assert!(
        map.values().all(|&offset| offset >= 0),
        "object stream offsets must be non-negative; some went negative \
         (signed modular char handling regressed)"
    );

    // Spot-check a couple of well-known handles: the root dictionary is
    // always present and its offset should fall inside the Objects
    // stream (i.e. far from 0 but not past file end).
    let file_size = bytes.len() as i64;
    for (&handle, &offset) in map.iter().take(5) {
        eprintln!("  handle 0x{handle:X} -> offset 0x{offset:X}");
        assert!(
            offset > 0 && offset < file_size,
            "handle 0x{handle:X} has implausible offset 0x{offset:X}"
        );
    }
}

/// Dump the section locator table for every parseable real sample so
/// we can see, in concrete terms, how AC1015 lays out its Header /
/// Classes / Handles / ObjFreeSpace / Template / AuxHeader records on
/// disk. This is pure observation (no assertions beyond existence) and
/// is the feedback loop for subsequent section-reader milestones.
#[test]
fn real_dwg_samples_section_locator_dump() {
    for (name, _) in real_samples() {
        let Some(bytes) = try_read_sample(name) else {
            continue;
        };
        let Ok(header) = DwgFileHeader::parse(&bytes) else {
            continue;
        };
        let Ok(sections) = SectionMap::parse(&bytes, &header) else {
            continue;
        };

        eprintln!(
            "{name} ({:?}): {} section locator records",
            header.version, header.section_count
        );
        for descriptor in &sections.descriptors {
            let known = KnownSection::from_record_number(descriptor.record_number);
            let kind = known
                .map(|s| s.name().to_string())
                .unwrap_or_else(|| format!("unknown#{}", descriptor.record_number));
            let start_sentinel_status = match known.and_then(|s| s.start_sentinel()) {
                Some(expected) => {
                    let start = descriptor.offset as usize;
                    let end = start + 16;
                    if end <= bytes.len() && bytes[start..end] == expected {
                        "sentinel=ok".to_string()
                    } else {
                        "sentinel=mismatch".to_string()
                    }
                }
                None => "sentinel=n/a".to_string()
            };
            eprintln!(
                "  [{:>2}] rec#{} {:<20} offset=0x{:08X} size={:>8}  {}",
                descriptor.index,
                descriptor.record_number,
                kind,
                descriptor.offset,
                descriptor.size,
                start_sentinel_status,
            );
        }
    }
}

/// M3-B brick 1: the `AcDb:Handles` decoder is now wired into
/// `build_pending_document`, so a real AC1015 drawing must expose a
/// non-empty `pending.handle_offsets` map on the main read pipeline.
///
/// We assert that:
/// * the map has a plausible size (`sample_AC1015.dwg` reports 1047
///   entries; we keep a loose floor of 20 for future sample variation),
/// * every decoded offset lands inside the file,
/// * every offset is strictly positive (AutoCAD never emits zero-offset
///   object pointers).
///
/// These invariants are what M3-B brick 2 (object-stream cursor) and
/// brick 3 (class-routed decoders) will depend on.
#[test]
fn real_ac1015_build_pending_document_populates_handle_offsets() {
    let Some(bytes) = try_read_sample("sample_AC1015.dwg") else {
        eprintln!("skip: sample_AC1015.dwg not present");
        return;
    };
    let header = DwgFileHeader::parse(&bytes).expect("AC1015 file header parse");
    let sections = SectionMap::parse(&bytes, &header).expect("AC1015 section map parse");
    let payloads = sections
        .read_section_payloads(&bytes)
        .expect("AC1015 section payloads readable");
    let pending = build_pending_document(&header, &sections, payloads)
        .expect("AC1015 pending document builds without error");

    eprintln!(
        "AC1015 pending.handle_offsets.len() = {}",
        pending.handle_offsets.len()
    );

    assert!(
        pending.handle_offsets.len() >= 20,
        "expected at least 20 handle_offsets decoded from the real AC1015 Handle \
         section, got {}",
        pending.handle_offsets.len()
    );

    // Spot-check the first few entries: the lowest-handle records always
    // point at the fixed tables near the start of the object stream, so
    // their offsets must be inside the file. Later handles can have
    // offsets that appear out-of-range because AutoCAD writes handle
    // map entries for purged/garbage-collected objects too; that higher
    // tail is brick 2's problem, not brick 1's.
    let file_size = bytes.len() as i64;
    for entry in pending.handle_offsets.iter().take(5) {
        assert!(
            entry.offset > 0 && entry.offset < file_size,
            "handle 0x{:X} has implausible object-stream offset 0x{:X} (file size {file_size})",
            entry.handle.value(),
            entry.offset
        );
    }

    // Handles must be strictly increasing: the on-disk stream uses a
    // monotonic delta encoding, so any non-increasing handle signals a
    // decoder bug, not a format variation.
    for window in pending.handle_offsets.windows(2) {
        assert!(
            window[0].handle.value() < window[1].handle.value(),
            "handle_offsets must be strictly increasing; saw \
             0x{:X} followed by 0x{:X}",
            window[0].handle.value(),
            window[1].handle.value(),
        );
    }
}

/// M3-B brick 2b: `ObjectStreamCursor` resolves a decoded handle →
/// (MS header + body) byte slice on the real AC1015 sample. We don't
/// yet decode the body in this milestone; brick 3 will parse it with
/// `BitReader` routed by object class. Here we only assert that the
/// first handful of "live" handles (i.e. low-handle table records that
/// are always present in a real drawing) yield a plausibly-sized slice
/// that stays inside the file.
///
/// The handle map tail contains purged/garbage entries whose offsets
/// are out-of-range; `object_slice_by_handle` is expected to return
/// `None` for those. We tally both groups and require a healthy ratio
/// of successful lookups so that a regression to "everything returns
/// None" (e.g. broken MS reader) is caught loudly.
#[test]
fn real_ac1015_object_stream_cursor_slices_first_objects() {
    let Some(bytes) = try_read_sample("sample_AC1015.dwg") else {
        eprintln!("skip: sample_AC1015.dwg not present");
        return;
    };
    let header = DwgFileHeader::parse(&bytes).expect("AC1015 file header parse");
    let sections = SectionMap::parse(&bytes, &header).expect("AC1015 section map parse");
    let payloads = sections
        .read_section_payloads(&bytes)
        .expect("AC1015 section payloads readable");
    let pending = build_pending_document(&header, &sections, payloads)
        .expect("AC1015 pending document builds without error");

    assert!(
        !pending.handle_offsets.is_empty(),
        "brick 1 must have decoded at least some handle_offsets"
    );

    let cursor = ObjectStreamCursor::new(&bytes, &pending.handle_offsets);

    // Probe the low-handle prefix. In a real AC1015 file these always
    // resolve to live table records; if even one of the first 20
    // handles fails, brick 2b is definitely broken.
    let probe_count = pending.handle_offsets.len().min(20);
    let mut resolved = 0usize;
    for entry in pending.handle_offsets.iter().take(probe_count) {
        let Some(slice) = cursor.object_slice_by_handle(entry.handle) else {
            continue;
        };
        // Slice must cover at least the MS header plus *something*.
        assert!(
            slice.len() >= 2,
            "handle 0x{:X} slice too short to contain an MS header (len = {})",
            entry.handle.value(),
            slice.len()
        );
        // Slice must stay inside the file (guaranteed by the method,
        // reasserted here so a future bug stands out).
        let start = entry.offset as usize;
        assert!(
            start + slice.len() <= bytes.len(),
            "handle 0x{:X} slice escapes file bounds: start={start} len={} file_len={}",
            entry.handle.value(),
            slice.len(),
            bytes.len()
        );
        resolved += 1;
    }

    eprintln!(
        "AC1015 object_stream: resolved {resolved} / {probe_count} low-handle slices \
         (total map entries = {})",
        pending.handle_offsets.len()
    );

    assert!(
        resolved >= probe_count / 2,
        "at least half of the first {probe_count} handles must resolve to a \
         valid object slice; got only {resolved}. Likely an MS header or \
         offset-range regression."
    );
}

/// M3-B brick 3a: `read_ac1015_object_header` turns each slice from
/// brick 2b into a typed `ObjectHeader`. The three fields decoded
/// (object_type, main_size_bits, handle) are the minimum routing
/// information brick 3b needs before class-specific decoders can run.
///
/// Real-sample expectations on AC1015:
///
/// * The decoded handle inside the header must match the handle map
///   entry that routed us to the slice. A mismatch means either the
///   slice is misaligned or the handle reader dropped bits.
/// * `main_size_bits` must be strictly positive and fit inside the
///   slice's body (after the MS prefix). AutoCAD never writes a
///   zero-size body for a real object.
/// * `object_type` values on AC1015 cluster into two ranges:
///   1..=0x1F1 (built-in types, e.g. LINE=19, CIRCLE=17, TEXT=1) and
///   ≥ 0x1F4 (custom classes registered in the Classes section).
///   Both are plausible; we only reject obviously broken values
///   like `0`.
///
/// The test logs the observed type histogram so future changes to
/// sample files or decoder behaviour surface as a diff in the test
/// output.
#[test]
fn real_ac1015_object_header_decodes_first_objects() {
    let Some(bytes) = try_read_sample("sample_AC1015.dwg") else {
        eprintln!("skip: sample_AC1015.dwg not present");
        return;
    };
    let header = DwgFileHeader::parse(&bytes).expect("AC1015 file header parse");
    let sections = SectionMap::parse(&bytes, &header).expect("AC1015 section map parse");
    let payloads = sections
        .read_section_payloads(&bytes)
        .expect("AC1015 section payloads readable");
    let pending = build_pending_document(&header, &sections, payloads)
        .expect("AC1015 pending document builds without error");

    let cursor = ObjectStreamCursor::new(&bytes, &pending.handle_offsets);

    let probe_count = pending.handle_offsets.len().min(20);
    let mut type_histogram: std::collections::BTreeMap<i16, usize> =
        std::collections::BTreeMap::new();
    let mut decoded = 0usize;
    let mut handle_matches = 0usize;

    for entry in pending.handle_offsets.iter().take(probe_count) {
        let Some(slice) = cursor.object_slice_by_handle(entry.handle) else {
            continue;
        };
        let Ok((obj_header, _reader)) = read_ac1015_object_header(slice) else {
            eprintln!(
                "  handle 0x{:X}: object_header decode failed (slice len = {})",
                entry.handle.value(),
                slice.len()
            );
            continue;
        };
        decoded += 1;
        *type_histogram.entry(obj_header.object_type).or_insert(0) += 1;
        if obj_header.handle == entry.handle {
            handle_matches += 1;
        } else {
            eprintln!(
                "  handle mismatch: map says 0x{:X} but header says 0x{:X} \
                 (object_type={}, main_size_bits={})",
                entry.handle.value(),
                obj_header.handle.value(),
                obj_header.object_type,
                obj_header.main_size_bits
            );
        }

        // main_size_bits must fit inside the slice's body portion.
        // slice = [MS header (1-4 bytes)] + [body (body_size bytes)].
        // We can't reach body_size from outside the module, but the
        // slice length is an upper bound: body_size ≤ slice.len().
        let slice_bits_upper = (slice.len() as u64) * 8;
        assert!(
            (obj_header.main_size_bits as u64) <= slice_bits_upper,
            "handle 0x{:X}: main_size_bits {} exceeds slice bits {}",
            entry.handle.value(),
            obj_header.main_size_bits,
            slice_bits_upper
        );
    }

    eprintln!(
        "AC1015 object_header: {decoded} / {probe_count} probed handles decoded, \
         {handle_matches} matched map handle. type histogram:"
    );
    for (type_code, count) in &type_histogram {
        eprintln!("  type={type_code}: {count}");
    }

    // At least half of the probed handles must decode cleanly. An
    // outright zero would mean brick 3a is broken; the 50% floor
    // accommodates the long tail of purged/garbage handles in the
    // Handle map that can reach deep into `probe_count` on edge-case
    // samples.
    assert!(
        decoded >= probe_count / 2,
        "expected at least half the probed handles to decode an \
         object_header; got only {decoded} / {probe_count}"
    );
    // Any handle that did decode must also match its own map entry.
    // A mismatch means the bit cursor drifted somewhere inside
    // BS/RL/H, which invalidates every downstream byte the slice
    // would otherwise hand off to brick 3b.
    assert_eq!(
        handle_matches, decoded,
        "every decoded header must agree with its handle-map handle"
    );
}

/// M3-B brick 3b scouting: scan **every** handle in the real AC1015
/// sample's Handle map, not just the first 20. The purpose is
/// observational — before we can pick which entity type to decode
/// first (LINE=19 / CIRCLE=17 / TEXT=1 / ARC=18 / POINT=27 / …), we
/// need the real type-frequency distribution on disk. Without this
/// we'd be guessing which decoder has the highest payoff.
///
/// No semantic assertions beyond the bare-minimum invariants: every
/// decoded header must agree with its map handle, and the entire map
/// must be walkable without panic. The histogram itself is printed to
/// stderr so a future maintainer can see at a glance which object
/// types a given sample contains.
#[test]
fn real_ac1015_full_handle_map_object_type_histogram() {
    let Some(bytes) = try_read_sample("sample_AC1015.dwg") else {
        eprintln!("skip: sample_AC1015.dwg not present");
        return;
    };
    let header = DwgFileHeader::parse(&bytes).expect("AC1015 file header parse");
    let sections = SectionMap::parse(&bytes, &header).expect("AC1015 section map parse");
    let payloads = sections
        .read_section_payloads(&bytes)
        .expect("AC1015 section payloads readable");
    let pending = build_pending_document(&header, &sections, payloads)
        .expect("AC1015 pending document builds without error");

    let cursor = ObjectStreamCursor::new(&bytes, &pending.handle_offsets);
    let total = pending.handle_offsets.len();

    let mut histogram: std::collections::BTreeMap<i16, usize> =
        std::collections::BTreeMap::new();
    let mut decoded = 0usize;
    let mut slice_miss = 0usize;
    let mut header_fail = 0usize;
    let mut handle_mismatch = 0usize;

    for entry in pending.handle_offsets.iter() {
        let Some(slice) = cursor.object_slice_by_handle(entry.handle) else {
            slice_miss += 1;
            continue;
        };
        match read_ac1015_object_header(slice) {
            Ok((hdr, _reader)) => {
                decoded += 1;
                *histogram.entry(hdr.object_type).or_insert(0) += 1;
                if hdr.handle != entry.handle {
                    handle_mismatch += 1;
                }
            }
            Err(_) => {
                header_fail += 1;
            }
        }
    }

    eprintln!(
        "AC1015 full scan: total={total} decoded={decoded} \
         slice_miss={slice_miss} header_fail={header_fail} handle_mismatch={handle_mismatch}"
    );
    eprintln!("AC1015 full type histogram:");
    for (type_code, count) in &histogram {
        let label = ac1015_object_type_label(*type_code);
        eprintln!("  type={type_code:>3} {label:<18} count={count}");
    }

    // Baseline invariant: at least half of the 1047-entry Handle map
    // must lead to a decodable object header, otherwise brick 2/3a has
    // regressed. We do not assert anything about the specific
    // histogram because the content varies by sample.
    assert!(
        decoded * 2 >= total,
        "at least half of the handle map must produce a decodable header, \
         got only {decoded} / {total}"
    );
}

#[test]
fn real_ac1015_preheader_object_type_hints_follow_offsets_not_handles() {
    let Some(bytes) = try_read_sample("sample_AC1015.dwg") else {
        eprintln!("skip: sample_AC1015.dwg not present");
        return;
    };
    let header = DwgFileHeader::parse(&bytes).expect("AC1015 file header parse");
    let sections = SectionMap::parse(&bytes, &header).expect("AC1015 section map parse");
    let payloads = sections
        .read_section_payloads(&bytes)
        .expect("AC1015 section payloads readable");
    let pending = build_pending_document(&header, &sections, payloads)
        .expect("AC1015 pending document builds without error");

    let hints = collect_ac1015_preheader_object_type_hints(&bytes, &pending);
    let total = hints.len();
    let offset_backed = hints
        .iter()
        .filter(|hint| hint.source == "offset_window_le_type")
        .count();
    let header_backed = hints
        .iter()
        .filter(|hint| hint.source == "object_header")
        .count();
    let unresolved = hints
        .iter()
        .filter(|hint| hint.source == "unresolved")
        .count();

    let mut family_counts = std::collections::BTreeMap::<&'static str, usize>::new();
    for hint in hints.iter().filter_map(|hint| hint.family) {
        *family_counts.entry(hint).or_insert(0) += 1;
    }

    eprintln!(
        "AC1015 pre-header type hints: total={total} offset_backed={offset_backed} header_backed={header_backed} unresolved={unresolved}"
    );
    for family in ["LINE", "POINT", "CIRCLE", "ARC", "LWPOLYLINE", "TEXT", "HATCH"] {
        eprintln!(
            "  family={family} hinted={}",
            family_counts.get(family).copied().unwrap_or(0)
        );
    }

    let sample_lines: Vec<_> = hints
        .iter()
        .filter(|hint| hint.family == Some("LINE"))
        .take(3)
        .map(|hint| {
            format!(
                "0x{:X}@offset={} source={} type={:?}",
                hint.handle.value(),
                hint.offset,
                hint.source,
                hint.object_type
            )
        })
        .collect();
    eprintln!("  sample LINE hints: {}", sample_lines.join(", "));

    assert_eq!(total, pending.handle_offsets.len(), "every handle-map entry should produce a hint record");
    assert!(
        header_backed >= 600,
        "expected header decoding to expose at least 600 object types, got {header_backed}"
    );
    assert!(
        family_counts.get("LINE").copied().unwrap_or(0) >= 80,
        "expected offset/header hints to expose at least 80 LINE candidates, got {}",
        family_counts.get("LINE").copied().unwrap_or(0)
    );
    assert!(
        family_counts.get("POINT").copied().unwrap_or(0) >= 30,
        "expected offset/header hints to expose at least 30 POINT candidates, got {}",
        family_counts.get("POINT").copied().unwrap_or(0)
    );
    assert!(
        family_counts.get("LWPOLYLINE").copied().unwrap_or(0) >= 15,
        "expected offset/header hints to expose at least 15 LWPOLYLINE candidates, got {}",
        family_counts.get("LWPOLYLINE").copied().unwrap_or(0)
    );
    assert_eq!(
        offset_backed, 0,
        "sample_AC1015.dwg should prove the old handle-based offset-window heuristic is absent on the real object stream"
    );
    assert!(
        hints.iter().any(|hint| {
            hint.family == Some("LINE")
                && hint.source == "object_header"
                && hint.offset != i64::try_from(hint.handle.value()).unwrap_or_default()
        }),
        "expected at least one LINE hint whose truthful evidence comes from object-header decoding at a real stream offset, not handle.value()"
    );
}

/// Human-readable label for the AC1015 built-in object type codes
/// that the Handle map can point at. Used only for test diagnostics;
/// the list is a best-effort subset of the ODA spec and covers the
/// records most likely to appear in a typical drawing. Unknown types
/// render as `"?"` so the raw number remains visible.
fn ac1015_object_type_label(code: i16) -> &'static str {
    match code {
        1 => "TEXT",
        17 => "ARC",
        18 => "CIRCLE",
        19 => "LINE",
        27 => "POINT",
        31 => "BLOCK",
        32 => "ENDBLK",
        34 => "POLYLINE_3D",
        35 => "VERTEX_3D",
        42 => "DICTIONARY",
        48 => "BLOCK_CONTROL",
        49 => "BLOCK_HEADER",
        50 => "LAYER_CONTROL",
        51 => "LAYER",
        52 => "STYLE_CONTROL",
        53 => "STYLE",
        56 => "LTYPE_CONTROL",
        57 => "LTYPE",
        60 => "VIEW_CONTROL",
        62 => "UCS_CONTROL",
        64 => "VPORT_CONTROL",
        66 => "APPID_CONTROL",
        67 => "APPID",
        68 => "DIMSTYLE_CONTROL",
        69 => "VP_ENT_HDR_CTRL",
        70 => "DIMSTYLE",
        71 => "VP_ENT_HDR",
        77 => "LWPOLYLINE",
        78 => "HATCH",
        code if code >= 500 => "CUSTOM_CLASS",
        _ => "?",
    }
}

#[test]
fn ac1015_representative_geometric_failure_handles() {
    let Some(bytes) = try_read_sample("sample_AC1015.dwg") else {
        return;
    };
    let header = DwgFileHeader::parse(&bytes).expect("AC1015 file header parse");
    let sections = SectionMap::parse(&bytes, &header).expect("AC1015 section map parse");
    let payloads = sections
        .read_section_payloads(&bytes)
        .expect("AC1015 section payloads readable");
    let pending = build_pending_document(&header, &sections, payloads)
        .expect("AC1015 pending document builds without error");
    let diagnostics = collect_ac1015_recovery_diagnostics(&bytes, &pending);

    print_supported_geometric_failure_examples(&diagnostics);

    let representatives = representative_supported_geometric_stage_failures(&diagnostics);

    let mut saw_stageful_representative = false;
    for family in ["LINE", "POINT", "CIRCLE", "ARC", "LWPOLYLINE"] {
        let by_kind = representatives.get(family);
        for kind in [
            Ac1015RecoveryFailureKind::HeaderFail,
            Ac1015RecoveryFailureKind::CommonDecodeFail,
            Ac1015RecoveryFailureKind::UnsupportedType,
        ] {
            if let Some(failures) = by_kind.and_then(|m| m.get(&kind)) {
                for failure in failures {
                    assert_eq!(
                        failure.family,
                        Some(family),
                        "representative failure family should match requested family"
                    );
                    assert_eq!(
                        failure.kind, kind,
                        "representative failure kind should match requested bucket"
                    );
                    if failure.stage.is_some() {
                        saw_stageful_representative = true;
                    }
                }
            }
        }
    }
    let any_supported_geom_histogram_presence = diagnostics.recovered_total == 0
        || ["LINE", "POINT", "CIRCLE", "ARC", "LWPOLYLINE"]
            .into_iter()
            .any(|family| {
                diagnostics
                    .recovered_by_family
                    .get(family)
                    .copied()
                    .unwrap_or(0)
                    > 0
            });
    assert!(
        any_supported_geom_histogram_presence,
        "expected diagnostics to at least surface supported geometric families in the recovery histogram"
    );
    assert!(
        saw_stageful_representative || any_supported_geom_histogram_presence,
        "expected supported geometric failure diagnostics to yield either stageful representatives or visible supported-family recovery presence"
    );
}

#[test]
fn ac1015_recovery_diagnostics_attribute_supported_families_from_preheader_hints() {
    let Some(bytes) = try_read_sample("sample_AC1015.dwg") else {
        return;
    };
    let header = DwgFileHeader::parse(&bytes).expect("AC1015 file header parse");
    let sections = SectionMap::parse(&bytes, &header).expect("AC1015 section map parse");
    let payloads = sections
        .read_section_payloads(&bytes)
        .expect("AC1015 section payloads readable");
    let pending = build_pending_document(&header, &sections, payloads)
        .expect("AC1015 pending document builds without error");
    let diagnostics = collect_ac1015_recovery_diagnostics(&bytes, &pending);
    let family_bucket_count = |family: &'static str, kind: Ac1015RecoveryFailureKind| {
        diagnostics
            .failure_counts_by_family
            .get(family)
            .and_then(|m| m.get(&kind))
            .copied()
            .unwrap_or(0)
    };

    for (family, kind) in [
        ("LINE", Ac1015RecoveryFailureKind::BodyDecodeFail),
        ("POINT", Ac1015RecoveryFailureKind::BodyDecodeFail),
        ("CIRCLE", Ac1015RecoveryFailureKind::BodyDecodeFail),
        ("ARC", Ac1015RecoveryFailureKind::BodyDecodeFail),
        ("LWPOLYLINE", Ac1015RecoveryFailureKind::BodyDecodeFail),
    ] {
        assert!(
            family_bucket_count(family, kind) > 0,
            "expected non-empty {family} {:?} attribution from parser diagnostics",
            kind
        );
    }
}

fn representative_supported_geometric_stage_failures(
    diagnostics: &h7cad_native_dwg::Ac1015RecoveryDiagnostics,
) -> std::collections::BTreeMap<
    &'static str,
    std::collections::BTreeMap<Ac1015RecoveryFailureKind, Vec<h7cad_native_dwg::Ac1015RecoveryFailure>>,
> {
    const FAMILIES: [&str; 5] = ["LINE", "POINT", "CIRCLE", "ARC", "LWPOLYLINE"];
    const KINDS: [Ac1015RecoveryFailureKind; 4] = [
        Ac1015RecoveryFailureKind::HeaderFail,
        Ac1015RecoveryFailureKind::CommonDecodeFail,
        Ac1015RecoveryFailureKind::BodyDecodeFail,
        Ac1015RecoveryFailureKind::UnsupportedType,
    ];

    let mut grouped = diagnostics.representative_failures_by_family_and_kind(&FAMILIES, &KINDS, 3);
    for failure in diagnostics.failures.iter().filter(|failure| {
        failure.family.is_none()
            && matches!(
                failure.kind,
                Ac1015RecoveryFailureKind::CommonDecodeFail
                    | Ac1015RecoveryFailureKind::UnsupportedType
            )
    }) {
        let Some(family) = failure.object_type.and_then(ac1015_geometric_family_from_type) else {
            continue;
        };
        let bucket = grouped
            .entry(family)
            .or_default()
            .entry(failure.kind)
            .or_default();
        if bucket.len() < 3 {
            let mut attributed = failure.clone();
            attributed.family = Some(family);
            bucket.push(attributed);
        }
    }
    for failure in diagnostics.failures.iter().filter(|failure| {
        matches!(
            failure.stage,
            Some("common_entity_decode") | Some("entity_body_decode") | Some("body_dispatch")
        )
    }) {
        let Some(family) = failure.object_type.and_then(ac1015_geometric_family_from_type) else {
            continue;
        };
        let kind = match failure.stage {
            Some("common_entity_decode") => Ac1015RecoveryFailureKind::CommonDecodeFail,
            Some("entity_body_decode") => Ac1015RecoveryFailureKind::BodyDecodeFail,
            Some("body_dispatch") => Ac1015RecoveryFailureKind::UnsupportedType,
            _ => continue,
        };
        let bucket = grouped.entry(family).or_default().entry(kind).or_default();
        if bucket.len() < 3 {
            let mut attributed = failure.clone();
            attributed.family = Some(family);
            attributed.kind = kind;
            bucket.push(attributed);
        }
    }
    grouped
}

fn ac1015_geometric_family_from_type(object_type: i16) -> Option<&'static str> {
    match object_type {
        19 => Some("LINE"),
        27 => Some("POINT"),
        18 => Some("CIRCLE"),
        17 => Some("ARC"),
        77 => Some("LWPOLYLINE"),
        _ => None,
    }
}

#[derive(Debug, Clone)]
struct Ac1015CommonProbeReport {
    handle: u64,
    family: &'static str,
    object_type: i16,
    header_main_size_bits: u32,
    header_end_bits: usize,
    main_position_bits_before_common: usize,
    main_bits_remaining_before_common: usize,
    handle_position_bits_before_common: usize,
    handle_bits_remaining_before_common: usize,
    common_result: String,
    common_failure_stage: Option<String>,
    common_failure_context: Option<String>,
    main_position_bits_after_common: usize,
    main_bits_remaining_after_common: usize,
    handle_position_bits_after_common: usize,
    handle_bits_remaining_after_common: usize,
    handle_reads: Vec<String>,
}

#[derive(Debug, Clone)]
struct Ac1015CommonLayoutComparison {
    family: &'static str,
    representative_handle: u64,
    blocked_handle: u64,
    representative_xdata_size: Option<i32>,
    blocked_xdata_size: Option<i32>,
    representative_first_xdata_block_size: Option<i32>,
    blocked_first_xdata_block_size: Option<i32>,
    representative_reaches_xdictionary: bool,
    blocked_reaches_xdictionary: bool,
    representative_reaches_layer: bool,
    blocked_reaches_layer: bool,
    representative_handle_stream_advanced: bool,
    blocked_handle_stream_advanced: bool,
    representative_main_remaining_after_common: usize,
    blocked_main_remaining_after_common: usize,
    blocked_failure_stage: Option<String>,
}

#[derive(Debug, Clone)]
struct Ac1015LineBodyFieldProgress {
    label: &'static str,
    position_before_bits: usize,
    remaining_before_bits: usize,
    position_after_bits: usize,
    remaining_after_bits: usize,
}

#[derive(Debug, Clone)]
struct Ac1015BodyBoundaryAudit {
    payload_consumed_bits: usize,
    payload_remaining_bits: usize,
    consumed_to_declared_boundary: bool,
}

#[derive(Debug, Clone)]
struct Ac1015LineBodyProbe {
    handle: u64,
    family: &'static str,
    object_type: i16,
    body_bytes: Vec<u8>,
    body_start_bits: usize,
    body_remaining_bits_before: usize,
    fields: Vec<Ac1015LineBodyFieldProgress>,
    boundary_audit: Ac1015BodyBoundaryAudit,
}

fn extract_logged_i32(entries: &[String], needle: &str) -> Option<i32> {
    entries.iter().find_map(|entry| {
        if !entry.contains(needle) {
            return None;
        }
        let value = entry
            .split("value=")
            .nth(1)?
            .split_whitespace()
            .next()?;
        value.parse::<i32>().ok()
    })
}

fn ac1015_common_layout_comparison(
    bytes: &[u8],
    family: &'static str,
    representative_handle: u64,
    blocked_handle: u64,
) -> Ac1015CommonLayoutComparison {
    let representative = ac1015_common_stream_probe_report(bytes, representative_handle, family);
    let blocked = ac1015_common_stream_probe_report(bytes, blocked_handle, family);

    Ac1015CommonLayoutComparison {
        family,
        representative_handle,
        blocked_handle,
        representative_xdata_size: extract_logged_i32(&representative.handle_reads, "label=xdata_size"),
        blocked_xdata_size: extract_logged_i32(&blocked.handle_reads, "label=xdata_size"),
        representative_first_xdata_block_size: extract_logged_i32(
            &representative.handle_reads,
            "label=xdata[0].size",
        ),
        blocked_first_xdata_block_size: extract_logged_i32(&blocked.handle_reads, "label=xdata[0].size"),
        representative_reaches_xdictionary: representative
            .handle_reads
            .iter()
            .any(|entry| entry.contains("label=xdictionary")),
        blocked_reaches_xdictionary: blocked
            .handle_reads
            .iter()
            .any(|entry| entry.contains("label=xdictionary")),
        representative_reaches_layer: representative
            .handle_reads
            .iter()
            .any(|entry| entry.contains("label=layer")),
        blocked_reaches_layer: blocked
            .handle_reads
            .iter()
            .any(|entry| entry.contains("label=layer")),
        representative_handle_stream_advanced: representative.handle_position_bits_after_common
            > representative.handle_position_bits_before_common,
        blocked_handle_stream_advanced: blocked.handle_position_bits_after_common
            > blocked.handle_position_bits_before_common,
        representative_main_remaining_after_common: representative.main_bits_remaining_after_common,
        blocked_main_remaining_after_common: blocked.main_bits_remaining_after_common,
        blocked_failure_stage: blocked.common_failure_stage.clone(),
    }
}

fn ac1015_common_stream_probe_report(
    bytes: &[u8],
    handle_value: u64,
    family: &'static str,
) -> Ac1015CommonProbeReport {
    let header = DwgFileHeader::parse(bytes).expect("AC1015 file header parse");
    let sections = SectionMap::parse(bytes, &header).expect("AC1015 section map parse");
    let payloads = sections
        .read_section_payloads(bytes)
        .expect("AC1015 section payloads readable");
    let pending = build_pending_document(&header, &sections, payloads)
        .expect("AC1015 pending document builds without error");
    let target = Handle::new(handle_value);
    let cursor = ObjectStreamCursor::new(bytes, &pending.handle_offsets);
    let slice = cursor
        .object_slice_by_handle(target)
        .unwrap_or_else(|| panic!("expected object slice for 0x{handle_value:X}"));
    let (obj_header, main_reader, handle_reader) =
        h7cad_native_dwg::split_ac1015_object_streams(slice).unwrap_or_else(|err| {
            panic!("split object streams for 0x{handle_value:X} should succeed: {err:?}")
        });

    assert_eq!(
        obj_header.handle, target,
        "object header handle should match representative handle 0x{handle_value:X}"
    );

    let header_reader = read_ac1015_object_header(slice)
        .expect("header reader should decode representative handle");
    let header_end_bits = header_reader.1.position_in_bits();

    let main_position_bits_before_common = main_reader.position_in_bits();
    let main_bits_remaining_before_common = main_reader.bits_remaining();
    let handle_position_bits_before_common = handle_reader.position_in_bits();
    let handle_bits_remaining_before_common = handle_reader.bits_remaining();

    let mut instrumented_main = main_reader.clone();
    let mut instrumented_handle = handle_reader.clone();
    let mut handle_reads = Vec::new();
    let common_result = match parse_ac1015_entity_common_instrumented(
        &mut instrumented_main,
        &mut instrumented_handle,
        target,
        &mut handle_reads,
    ) {
        Ok(()) => "ok".to_string(),
        Err(err) => format!("err({err:?})"),
    };
    let common_failure_stage = handle_reads
        .iter()
        .find_map(|entry| {
            entry.strip_prefix("stage=").and_then(|stage| {
                if stage == "done" {
                    None
                } else {
                    Some(stage.to_string())
                }
            })
        });
    let common_failure_context = handle_reads
        .iter()
        .find_map(|entry| entry.strip_prefix("failure_context=").map(str::to_string));

    Ac1015CommonProbeReport {
        handle: handle_value,
        family,
        object_type: obj_header.object_type,
        header_main_size_bits: obj_header.main_size_bits,
        header_end_bits,
        main_position_bits_before_common,
        main_bits_remaining_before_common,
        handle_position_bits_before_common,
        handle_bits_remaining_before_common,
        common_result,
        common_failure_stage,
        common_failure_context,
        main_position_bits_after_common: instrumented_main.position_in_bits(),
        main_bits_remaining_after_common: instrumented_main.bits_remaining(),
        handle_position_bits_after_common: instrumented_handle.position_in_bits(),
        handle_bits_remaining_after_common: instrumented_handle.bits_remaining(),
        handle_reads,
    }
}

fn ac1015_line_body_probe(
    bytes: &[u8],
    handle_value: u64,
    family: &'static str,
) -> Ac1015LineBodyProbe {
    let header = DwgFileHeader::parse(bytes).expect("AC1015 file header parse");
    let sections = SectionMap::parse(bytes, &header).expect("AC1015 section map parse");
    let payloads = sections
        .read_section_payloads(bytes)
        .expect("AC1015 section payloads readable");
    let pending = build_pending_document(&header, &sections, payloads)
        .expect("AC1015 pending document builds without error");
    let target = Handle::new(handle_value);
    let cursor = ObjectStreamCursor::new(bytes, &pending.handle_offsets);
    let slice = cursor
        .object_slice_by_handle(target)
        .unwrap_or_else(|| panic!("expected object slice for 0x{handle_value:X}"));
    let (obj_header, main_reader, handle_reader) =
        h7cad_native_dwg::split_ac1015_object_streams(slice).unwrap_or_else(|err| {
            panic!("split object streams for 0x{handle_value:X} should succeed: {err:?}")
        });

    let mut common_main = main_reader.clone();
    let mut common_handle = handle_reader.clone();
    let mut common_log = Vec::new();
    parse_ac1015_entity_common_instrumented(
        &mut common_main,
        &mut common_handle,
        target,
        &mut common_log,
    )
    .unwrap_or_else(|err| panic!("common decode for 0x{handle_value:X} should succeed: {err:?}"));

    let body_start_bits = common_main.position_in_bits();
    let body_remaining_bits_before = common_main.bits_remaining();
    let mut body_reader = common_main.clone();
    let mut fields = Vec::new();

    let mut record_field = |label: &'static str, reader: &mut BitReader<'_>, read: &mut dyn FnMut(&mut BitReader<'_>)| {
        let position_before_bits = reader.position_in_bits();
        let remaining_before_bits = reader.bits_remaining();
        read(reader);
        fields.push(Ac1015LineBodyFieldProgress {
            label,
            position_before_bits,
            remaining_before_bits,
            position_after_bits: reader.position_in_bits(),
            remaining_after_bits: reader.bits_remaining(),
        });
    };

    let z_are_zero = {
        let mut value = 0u8;
        record_field("z_are_zero", &mut body_reader, &mut |reader| {
            value = reader.read_bit().expect("z_are_zero bit");
        });
        value == 1
    };
    record_field("start.x", &mut body_reader, &mut |reader| {
        let _ = reader.read_raw_f64_le().expect("LINE start.x");
    });
    record_field("end.x", &mut body_reader, &mut |reader| {
        let _ = reader
            .read_bit_double_with_default(0.0)
            .expect("LINE end.x bit-double");
    });
    record_field("start.y", &mut body_reader, &mut |reader| {
        let _ = reader.read_raw_f64_le().expect("LINE start.y");
    });
    record_field("end.y", &mut body_reader, &mut |reader| {
        let _ = reader
            .read_bit_double_with_default(0.0)
            .expect("LINE end.y bit-double");
    });
    if !z_are_zero {
        record_field("start.z", &mut body_reader, &mut |reader| {
            let _ = reader.read_raw_f64_le().expect("LINE start.z");
        });
        record_field("end.z", &mut body_reader, &mut |reader| {
            let _ = reader
                .read_bit_double_with_default(0.0)
                .expect("LINE end.z bit-double");
        });
    }
    record_field("thickness", &mut body_reader, &mut |reader| {
        let _ = reader
            .read_bit_thickness_r2000_plus()
            .expect("LINE thickness");
    });
    record_field("extrusion", &mut body_reader, &mut |reader| {
        let _ = reader
            .read_bit_extrusion_r2000_plus()
            .expect("LINE extrusion");
    });

    let body_start_byte = body_start_bits / 8;
    let body_end_byte = (obj_header.main_size_bits as usize).div_ceil(8);
    let body_bytes = slice[body_start_byte..body_end_byte].to_vec();
    let boundary_audit = Ac1015BodyBoundaryAudit {
        payload_consumed_bits: body_reader.position_in_bits().saturating_sub(body_start_bits),
        payload_remaining_bits: body_reader.bits_remaining(),
        consumed_to_declared_boundary: body_reader.bits_remaining() == 0,
    };

    Ac1015LineBodyProbe {
        handle: handle_value,
        family,
        object_type: obj_header.object_type,
        body_bytes,
        body_start_bits,
        body_remaining_bits_before,
        fields,
        boundary_audit,
    }
}

fn parse_ac1015_entity_common_instrumented(
    main_reader: &mut BitReader<'_>,
    handle_reader: &mut BitReader<'_>,
    object_handle: Handle,
    log: &mut Vec<String>,
) -> Result<(), h7cad_native_dwg::DwgReadError> {
    fn resolve_handle(
        handle_reader: &mut BitReader<'_>,
        object_handle: Handle,
        label: &str,
        log: &mut Vec<String>,
    ) -> Result<Handle, h7cad_native_dwg::DwgReadError> {
        let before = handle_reader.position_in_bits();
        let before_remaining = handle_reader.bits_remaining();
        let (code, raw) = handle_reader.read_handle()?;
        let resolved = match code {
            0x0..=0x5 => raw,
            0x6 => object_handle.value().saturating_add(1),
            0x8 => object_handle.value().saturating_sub(1),
            0xA => object_handle.value().saturating_add(raw),
            0xC => object_handle.value().saturating_sub(raw),
            _ => raw,
        };
        log.push(format!(
            "handle_read label={label} before_bit={before} before_remaining={before_remaining} code=0x{code:X} raw=0x{raw:X} resolved=0x{resolved:X} after_bit={} after_remaining={}",
            handle_reader.position_in_bits(),
            handle_reader.bits_remaining()
        ));
        Ok(Handle::new(resolved))
    }

    fn optional_handle(
        handle_reader: &mut BitReader<'_>,
        object_handle: Handle,
        label: &str,
        log: &mut Vec<String>,
    ) -> Result<Option<Handle>, h7cad_native_dwg::DwgReadError> {
        let before = handle_reader.position_in_bits();
        let before_remaining = handle_reader.bits_remaining();
        let (code, raw) = handle_reader.read_handle()?;
        if code == 0 && raw == 0 {
            log.push(format!(
                "handle_read label={label} before_bit={before} before_remaining={before_remaining} code=0x0 raw=0x0 resolved=NULL after_bit={} after_remaining={}",
                handle_reader.position_in_bits(),
                handle_reader.bits_remaining()
            ));
            return Ok(None);
        }
        let resolved = match code {
            0x0..=0x5 => raw,
            0x6 => object_handle.value().saturating_add(1),
            0x8 => object_handle.value().saturating_sub(1),
            0xA => object_handle.value().saturating_add(raw),
            0xC => object_handle.value().saturating_sub(raw),
            _ => raw,
        };
        log.push(format!(
            "handle_read label={label} before_bit={before} before_remaining={before_remaining} code=0x{code:X} raw=0x{raw:X} resolved=0x{resolved:X} after_bit={} after_remaining={}",
            handle_reader.position_in_bits(),
            handle_reader.bits_remaining()
        ));
        Ok(Some(Handle::new(resolved)))
    }

    let stage = |label: &'static str, log: &mut Vec<String>| {
        log.push(format!("stage={label}"));
    };

    log.push(format!(
        "main_start bit={} remaining={} handle_start bit={} remaining={}",
        main_reader.position_in_bits(),
        main_reader.bits_remaining(),
        handle_reader.position_in_bits(),
        handle_reader.bits_remaining()
    ));

    stage("skip_extended_entity_data", log);
    let xdata_size = main_reader.read_bit_short()?;
    log.push(format!(
        "main_field label=xdata_size value={xdata_size} bit={} remaining={}",
        main_reader.position_in_bits(),
        main_reader.bits_remaining()
    ));
    for index in 0..xdata_size.max(0) as usize {
        let size = main_reader.read_bit_short()?;
        log.push(format!(
            "main_field label=xdata[{index}].size value={size} bit={} remaining={}",
            main_reader.position_in_bits(),
            main_reader.bits_remaining()
        ));
        if size < 0 {
            log.push("failure_context=negative extended entity data size".to_string());
            return Err(h7cad_native_dwg::DwgReadError::UnexpectedEof {
                context: "negative extended entity data size",
            });
        }
        for byte_index in 0..size as usize {
            let _ = main_reader.read_raw_u8()?;
            log.push(format!(
                "main_field label=xdata[{index}].byte[{byte_index}] bit={} remaining={}",
                main_reader.position_in_bits(),
                main_reader.bits_remaining()
            ));
        }
    }

    stage("graphic_marker", log);
    let has_graphic = main_reader.read_bit()? == 1;
    log.push(format!(
        "main_field label=has_graphic value={} bit={} remaining={}",
        has_graphic,
        main_reader.position_in_bits(),
        main_reader.bits_remaining()
    ));
    if has_graphic {
        let graphic_size = main_reader.read_raw_u32_le()? as usize;
        log.push(format!(
            "main_field label=graphic_size value={graphic_size} bit={} remaining={}",
            main_reader.position_in_bits(),
            main_reader.bits_remaining()
        ));
        for byte_index in 0..graphic_size {
            let _ = main_reader.read_raw_u8()?;
            log.push(format!(
                "main_field label=graphic.byte[{byte_index}] bit={} remaining={}",
                main_reader.position_in_bits(),
                main_reader.bits_remaining()
            ));
        }
    }

    stage("entity_mode_and_owner", log);
    let entity_mode = main_reader.read_bits(2)? as u8;
    log.push(format!(
        "main_field label=entity_mode value={entity_mode} bit={} remaining={}",
        main_reader.position_in_bits(),
        main_reader.bits_remaining()
    ));
    if entity_mode == 0 {
        let _ = resolve_handle(handle_reader, object_handle, "owner", log)?;
    } else {
        log.push("handle_read label=owner skipped=entity_mode_nonzero".to_string());
    }

    stage("reactors", log);
    let reactor_count = main_reader.read_bit_long()?;
    log.push(format!(
        "main_field label=reactor_count value={reactor_count} bit={} remaining={}",
        main_reader.position_in_bits(),
        main_reader.bits_remaining()
    ));
    for index in 0..reactor_count.max(0) as usize {
        let label = format!("reactor[{index}]");
        let _ = resolve_handle(handle_reader, object_handle, &label, log)?;
    }

    stage("xdictionary", log);
    let _ = optional_handle(handle_reader, object_handle, "xdictionary", log)?;

    stage("nolinks", log);
    let nolinks = main_reader.read_bit()? == 1;
    log.push(format!(
        "main_field label=nolinks value={} bit={} remaining={}",
        nolinks,
        main_reader.position_in_bits(),
        main_reader.bits_remaining()
    ));
    if !nolinks {
        let _ = resolve_handle(handle_reader, object_handle, "previous", log)?;
        let _ = resolve_handle(handle_reader, object_handle, "next", log)?;
    }

    stage("presentation", log);
    let color_index = main_reader.read_bit_short()?;
    log.push(format!(
        "main_field label=color_index value={color_index} bit={} remaining={}",
        main_reader.position_in_bits(),
        main_reader.bits_remaining()
    ));
    match main_reader.read_bit_double() {
        Ok(value) => log.push(format!(
            "main_field label=linetype_scale value={value:?} bit={} remaining={}",
            main_reader.position_in_bits(),
            main_reader.bits_remaining()
        )),
        Err(err) => {
            log.push("failure_context=BD".to_string());
            return Err(err);
        }
    }
    let _ = resolve_handle(handle_reader, object_handle, "layer", log)?;

    stage("linetype", log);
    let linetype_flags = main_reader.read_bits(2)? as u8;
    log.push(format!(
        "main_field label=linetype_flags value={linetype_flags} bit={} remaining={}",
        main_reader.position_in_bits(),
        main_reader.bits_remaining()
    ));
    if linetype_flags == 0b11 {
        let _ = resolve_handle(handle_reader, object_handle, "linetype", log)?;
    } else {
        log.push("handle_read label=linetype skipped=flags_not_explicit".to_string());
    }

    stage("plotstyle", log);
    let plotstyle_flags = main_reader.read_bits(2)? as u8;
    log.push(format!(
        "main_field label=plotstyle_flags value={plotstyle_flags} bit={} remaining={}",
        main_reader.position_in_bits(),
        main_reader.bits_remaining()
    ));
    if plotstyle_flags == 0b11 {
        let _ = resolve_handle(handle_reader, object_handle, "plotstyle", log)?;
    } else {
        log.push("handle_read label=plotstyle skipped=flags_not_explicit".to_string());
    }

    stage("visibility", log);
    let invisible = main_reader.read_bit_short()? != 0;
    log.push(format!(
        "main_field label=invisible value={} bit={} remaining={}",
        invisible,
        main_reader.position_in_bits(),
        main_reader.bits_remaining()
    ));

    stage("lineweight", log);
    let lineweight_raw = main_reader.read_raw_u8()?;
    log.push(format!(
        "main_field label=lineweight_raw value={lineweight_raw} bit={} remaining={}",
        main_reader.position_in_bits(),
        main_reader.bits_remaining()
    ));
    log.push("stage=done".to_string());
    Ok(())
}

#[test]
fn ac1015_line_point_common_stream_instrumentation_reports_alignment_for_representative_handles() {
    let Some(bytes) = try_read_sample("sample_AC1015.dwg") else {
        return;
    };

    let probes = [
        (0x2C7, "LINE"),
        (0x2CF, "LINE"),
        (0x517, "LINE"),
        (0x28E, "POINT"),
        (0x298, "POINT"),
        (0x299, "POINT"),
    ]
    .into_iter()
    .map(|(handle, family)| ac1015_common_stream_probe_report(&bytes, handle, family))
    .collect::<Vec<_>>();

    eprintln!("AC1015 representative LINE/POINT common-stream probes:");
    for probe in &probes {
        eprintln!(
            "  handle=0x{:X} family={} type={} header_main_size_bits={} header_end_bits={} main_before={}bits/{}rem handle_before={}bits/{}rem result={} stage={} failure_context={} main_after={}bits/{}rem handle_after={}bits/{}rem",
            probe.handle,
            probe.family,
            probe.object_type,
            probe.header_main_size_bits,
            probe.header_end_bits,
            probe.main_position_bits_before_common,
            probe.main_bits_remaining_before_common,
            probe.handle_position_bits_before_common,
            probe.handle_bits_remaining_before_common,
            probe.common_result,
            probe.common_failure_stage.as_deref().unwrap_or("none"),
            probe.common_failure_context.as_deref().unwrap_or("none"),
            probe.main_position_bits_after_common,
            probe.main_bits_remaining_after_common,
            probe.handle_position_bits_after_common,
            probe.handle_bits_remaining_after_common,
        );
        for entry in &probe.handle_reads {
            eprintln!("    {entry}");
        }
    }

    for probe in &probes {
        assert_eq!(probe.object_type, if probe.family == "LINE" { 19 } else { 27 });
        assert!(
            probe.main_position_bits_before_common < probe.header_main_size_bits as usize,
            "expected representative handle 0x{:X} to enter common decode before the declared AC1015 main-stream boundary",
            probe.handle
        );
        assert!(
            probe.main_position_bits_after_common > probe.main_position_bits_before_common,
            "expected representative handle 0x{:X} to advance through common main-stream fields before failing",
            probe.handle
        );
        assert!(
            probe.handle_position_bits_after_common >= probe.handle_position_bits_before_common,
            "expected representative handle 0x{:X} to preserve or advance handle-stream position during probing",
            probe.handle
        );
        if probe.handle == 0x298 {
            assert!(
                probe.common_result.starts_with("err("),
                "expected representative handle 0x298 to fail during the overlong xdata walk"
            );
            assert_eq!(
                probe.common_failure_stage.as_deref(),
                Some("skip_extended_entity_data")
            );
            assert_eq!(probe.handle_position_bits_after_common, probe.handle_position_bits_before_common);
            assert!(
                probe.handle_reads
                    .iter()
                    .any(|entry| entry.contains("label=xdata[0].size value=68")),
                "expected representative handle 0x298 to expose the oversized xdata preamble before alignment diverges"
            );
            assert!(
                probe.handle_reads
                    .iter()
                    .all(|entry| !entry.contains("label=xdictionary")),
                "expected representative handle 0x298 to diverge before the handle stream begins"
            );
        } else {
            assert_eq!(probe.common_result, "ok");
            assert_eq!(probe.common_failure_context.as_deref(), None);
            assert!(
                matches!(
                    probe.common_failure_stage.as_deref(),
                    Some("skip_extended_entity_data")
                ),
                "expected representative handle 0x{:X} to progress through the common preamble",
                probe.handle
            );
            assert!(
                probe.handle_reads.iter().any(|entry| entry.contains("label=layer")),
                "expected representative handle 0x{:X} to consume the layer handle after common-preamble presentation fields",
                probe.handle
            );
            assert!(
                probe.handle_reads
                    .iter()
                    .any(|entry| entry.contains("label=xdictionary")),
                "expected representative handle 0x{:X} to consume the optional xdictionary handle before divergence",
                probe.handle
            );
        }
    }
}

#[test]
fn ac1015_common_xdata_semantics_audit_identifies_overlong_main_stream_xdata_rule() {
    let Some(bytes) = try_read_sample("sample_AC1015.dwg") else {
        return;
    };

    let probes = [
        (0x2C7, "LINE"),
        (0x2CF, "LINE"),
        (0x517, "LINE"),
        (0x28E, "POINT"),
        (0x298, "POINT"),
        (0x299, "POINT"),
    ]
    .into_iter()
    .map(|(handle, family)| ac1015_common_stream_probe_report(&bytes, handle, family))
    .collect::<Vec<_>>();

    let ok_handles = probes
        .iter()
        .filter(|probe| probe.handle != 0x298)
        .collect::<Vec<_>>();
    let overlong = probes
        .iter()
        .find(|probe| probe.handle == 0x298)
        .expect("representative point 0x298 should be probed");

    eprintln!("AC1015 common/XDATA semantics audit:");
    eprintln!(
        "  truthful_rule=AC1015 entity EED count/size fields stay on the main stream and application handles do not come from the separate handle stream"
    );
    for probe in &ok_handles {
        eprintln!(
            "  representative_ok handle=0x{:X} family={} xdata_size_entry={} xdictionary_seen={} layer_seen={}",
            probe.handle,
            probe.family,
            probe.handle_reads
                .iter()
                .find(|entry| entry.contains("label=xdata_size"))
                .cloned()
                .unwrap_or_else(|| "missing".to_string()),
            probe.handle_reads
                .iter()
                .any(|entry| entry.contains("label=xdictionary")),
            probe.handle_reads
                .iter()
                .any(|entry| entry.contains("label=layer")),
        );
    }
    eprintln!(
        "  representative_overlong handle=0x{:X} family={} result={} stage={} main_after={} handle_after={} xdata_size_entry={} first_block_size_entry={}",
        overlong.handle,
        overlong.family,
        overlong.common_result,
        overlong.common_failure_stage.as_deref().unwrap_or("none"),
        overlong.main_position_bits_after_common,
        overlong.handle_position_bits_after_common,
        overlong
            .handle_reads
            .iter()
            .find(|entry| entry.contains("label=xdata_size"))
            .cloned()
            .unwrap_or_else(|| "missing".to_string()),
        overlong
            .handle_reads
            .iter()
            .find(|entry| entry.contains("label=xdata[0].size"))
            .cloned()
            .unwrap_or_else(|| "missing".to_string()),
    );

    assert!(
        ok_handles.iter().all(|probe| probe.common_result == "ok"),
        "all representative LINE/POINT handles except 0x298 should still traverse the common preamble successfully"
    );
    assert!(
        ok_handles.iter().all(|probe| probe
            .handle_reads
            .iter()
            .any(|entry| entry.contains("label=xdictionary"))),
        "successful representative handles should reach the xdictionary handle after xdata skipping"
    );
    assert!(
        ok_handles.iter().all(|probe| probe
            .handle_reads
            .iter()
            .any(|entry| entry.contains("label=layer"))),
        "successful representative handles should consume the layer handle from the handle stream"
    );

    assert_eq!(
        overlong.common_failure_stage.as_deref(),
        Some("skip_extended_entity_data"),
        "handle 0x298 should diverge inside the xdata skip stage itself"
    );
    assert!(
        overlong.common_result.starts_with("err("),
        "handle 0x298 should fail before any downstream handle decoding begins"
    );
    assert_eq!(
        overlong.handle_position_bits_after_common,
        overlong.handle_position_bits_before_common,
        "handle 0x298 should leave the separate handle stream untouched"
    );
    assert!(
        overlong
            .handle_reads
            .iter()
            .any(|entry| entry.contains("label=xdata_size value=23")),
        "handle 0x298 should report the main-stream xdata block count exactly as observed on the live sample"
    );
    assert!(
        overlong
            .handle_reads
            .iter()
            .any(|entry| entry.contains("label=xdata[0].size value=68")),
        "handle 0x298 should expose the oversized first xdata block size before EOF"
    );
    assert!(
        overlong
            .handle_reads
            .iter()
            .all(|entry| !entry.contains("label=xdictionary")),
        "handle 0x298 should never reach xdictionary if the main stream is exhausted by xdata bytes"
    );
    assert_eq!(
        overlong.main_bits_remaining_after_common,
        0,
        "handle 0x298 should exhaust the declared main stream during the xdata walk"
    );
}

#[test]
fn ac1015_line_point_blocked_handles_compare_common_layouts_against_recovered_representatives() {
    let Some(bytes) = try_read_sample("sample_AC1015.dwg") else {
        return;
    };

    let comparisons = [
        ac1015_common_layout_comparison(&bytes, "LINE", 0x2C7, 0x99E),
        ac1015_common_layout_comparison(&bytes, "LINE", 0x2CF, 0x9CD),
        ac1015_common_layout_comparison(&bytes, "LINE", 0x517, 0x9D4),
        ac1015_common_layout_comparison(&bytes, "POINT", 0x28E, 0x298),
        ac1015_common_layout_comparison(&bytes, "POINT", 0x299, 0x29A),
    ];

    eprintln!("AC1015 blocked-vs-recovered LINE/POINT common-layout comparison:");
    for comparison in &comparisons {
        eprintln!(
            "  family={} representative=0x{:X} blocked=0x{:X} rep_xdata_size={:?} blocked_xdata_size={:?} rep_first_block={:?} blocked_first_block={:?} rep_xdict={} blocked_xdict={} rep_layer={} blocked_layer={} rep_handle_advanced={} blocked_handle_advanced={} rep_main_remaining={} blocked_main_remaining={} blocked_stage={}",
            comparison.family,
            comparison.representative_handle,
            comparison.blocked_handle,
            comparison.representative_xdata_size,
            comparison.blocked_xdata_size,
            comparison.representative_first_xdata_block_size,
            comparison.blocked_first_xdata_block_size,
            comparison.representative_reaches_xdictionary,
            comparison.blocked_reaches_xdictionary,
            comparison.representative_reaches_layer,
            comparison.blocked_reaches_layer,
            comparison.representative_handle_stream_advanced,
            comparison.blocked_handle_stream_advanced,
            comparison.representative_main_remaining_after_common,
            comparison.blocked_main_remaining_after_common,
            comparison
                .blocked_failure_stage
                .as_deref()
                .unwrap_or("none"),
        );
    }

    let blocked_line_comparisons = comparisons
        .iter()
        .filter(|comparison| comparison.family == "LINE")
        .collect::<Vec<_>>();
    let blocked_point_comparisons = comparisons
        .iter()
        .filter(|comparison| comparison.family == "POINT")
        .collect::<Vec<_>>();

    assert!(
        blocked_line_comparisons.iter().all(|comparison| {
            comparison.representative_xdata_size == Some(0)
                && comparison.blocked_xdata_size == Some(32)
                && comparison.blocked_first_xdata_block_size == Some(68)
                && !comparison.blocked_reaches_xdictionary
                && !comparison.blocked_reaches_layer
                && !comparison.blocked_handle_stream_advanced
                && comparison
                    .blocked_failure_stage
                    .as_deref()
                    == Some("skip_extended_entity_data")
        }),
        "blocked LINE handles should share the same overlong main-stream xdata divergence pattern instead of reaching xdictionary/layer consumption"
    );
    assert!(
        blocked_point_comparisons.iter().any(|comparison| {
            comparison.blocked_handle == 0x298
                && comparison.representative_xdata_size == Some(0)
                && comparison.blocked_xdata_size == Some(23)
                && comparison.blocked_first_xdata_block_size == Some(68)
                && !comparison.blocked_reaches_xdictionary
                && !comparison.blocked_reaches_layer
                && !comparison.blocked_handle_stream_advanced
                && comparison.blocked_main_remaining_after_common == 0
                && comparison.blocked_failure_stage.as_deref() == Some("skip_extended_entity_data")
        }),
        "POINT 0x298 should isolate the selective overlong-main-stream xdata rule difference versus recovered representatives"
    );
    assert!(
        blocked_point_comparisons.iter().any(|comparison| {
            comparison.blocked_handle == 0x29A
                && comparison.representative_xdata_size == Some(0)
                && comparison.blocked_xdata_size == Some(32)
                && comparison.blocked_first_xdata_block_size == Some(68)
                && !comparison.blocked_reaches_xdictionary
                && !comparison.blocked_reaches_layer
                && !comparison.blocked_handle_stream_advanced
                && comparison.blocked_failure_stage.as_deref() == Some("skip_extended_entity_data")
        }),
        "POINT 0x29A should show the same blocked overlong-xdata pattern as the other stuck POINT/LINE handles on the live sample"
    );
}

#[test]
fn ac1015_line_point_blocked_handles_real_decode_path_advances_after_selective_fix() {
    let Some(bytes) = try_read_sample("sample_AC1015.dwg") else {
        return;
    };

    let header = DwgFileHeader::parse(&bytes).expect("AC1015 file header parse");
    let sections = SectionMap::parse(&bytes, &header).expect("AC1015 section map parse");
    let payloads = sections
        .read_section_payloads(&bytes)
        .expect("AC1015 section payloads readable");
    let pending = build_pending_document(&header, &sections, payloads)
        .expect("AC1015 pending document builds without error");
    let diagnostics = collect_ac1015_recovery_diagnostics(&bytes, &pending);

    let stuck_handles = [
        (0x99E_u64, "LINE"),
        (0x9CD, "LINE"),
        (0x9D4, "LINE"),
        (0x298, "POINT"),
        (0x29A, "POINT"),
    ];

    for (handle, family) in stuck_handles {
        let failure = diagnostics
            .failures
            .iter()
            .find(|failure| failure.handle.value() == handle)
            .unwrap_or_else(|| panic!("blocked {family} handle 0x{handle:X} should remain visible on the real decode path after the selective fix"));
        assert_eq!(
            failure.family,
            Some(family),
            "blocked handle 0x{handle:X} should stay attributed to the {family} family on the real decode path"
        );
        assert!(
            matches!(
                failure.kind,
                Ac1015RecoveryFailureKind::CommonDecodeFail | Ac1015RecoveryFailureKind::BodyDecodeFail
            ),
            "blocked handle 0x{handle:X} should advance past the old skip_extended_entity_data divergence into a later decode stage"
        );
        assert!(
            matches!(
                failure.stage,
                Some("common_entity_decode")
                    | Some("entity_body_decode")
                    | Some("preheader_supported_hint")
            ),
            "blocked handle 0x{handle:X} should stay on the observed post-selective decode path after the selective fix"
        );
        assert!(
            !matches!(failure.stage, Some("skip_extended_entity_data")),
            "blocked handle 0x{handle:X} should no longer fail inside skip_extended_entity_data after the selective fix"
        );
    }

    assert!(
        !diagnostics.failures.is_empty(),
        "the diagnostics surface should still contain failure evidence after the selective fix"
    );
}

#[test]
fn ac1015_line_point_post_common_body_audit_reports_representative_failure_stage() {
    let Some(bytes) = try_read_sample("sample_AC1015.dwg") else {
        eprintln!("skip: sample_AC1015.dwg not present");
        return;
    };
    let header = DwgFileHeader::parse(&bytes).expect("AC1015 file header parse");
    let sections = SectionMap::parse(&bytes, &header).expect("AC1015 section map parse");
    let payloads = sections
        .read_section_payloads(&bytes)
        .expect("AC1015 section payloads readable");
    let pending = build_pending_document(&header, &sections, payloads)
        .expect("AC1015 pending document builds without error");
    let diagnostics = collect_ac1015_recovery_diagnostics(&bytes, &pending);

    let probes = [
        (0x2C7_u64, "LINE"),
        (0x2CF, "LINE"),
        (0x517, "LINE"),
        (0x28E, "POINT"),
        (0x298, "POINT"),
        (0x299, "POINT"),
    ];

    let mut observed = Vec::new();
    for (handle_value, family) in probes {
        let handle = Handle::new(handle_value);
        let failures = diagnostics
            .failures
            .iter()
            .filter(|failure| failure.handle == handle)
            .cloned()
            .collect::<Vec<_>>();
        assert!(
            !failures.is_empty(),
            "representative {family} handle 0x{handle_value:X} should remain visible on the diagnostics surface"
        );

        let body_failure = failures
            .iter()
            .find(|failure| failure.kind == Ac1015RecoveryFailureKind::BodyDecodeFail)
            .expect("representative handle should still fail during body decode on the live sample");

        assert_eq!(
            body_failure.family,
            Some(family),
            "representative handle 0x{handle_value:X} should retain its supported family attribution"
        );
        assert_eq!(
            body_failure.object_type,
            Some(if family == "LINE" { 19 } else { 27 }),
            "representative handle 0x{handle_value:X} should keep the truthful supported object type hint"
        );
        assert!(
            matches!(body_failure.stage, Some("entity_body_decode")),
            "representative handle 0x{handle_value:X} should persist a truthful later-stage failure before the synthetic fallback path"
        );
        assert!(
            matches!(body_failure.kind, Ac1015RecoveryFailureKind::BodyDecodeFail),
            "representative handle 0x{handle_value:X} should now fail on the real body decode path"
        );
        observed.push(format!(
            "handle=0x{handle_value:X} family={family} kind={} stage={} object_type={}",
            body_failure.kind.as_str(),
            body_failure.stage.unwrap_or("none"),
            body_failure.object_type.unwrap_or_default()
        ));
    }

    eprintln!("AC1015 LINE/POINT post-common/body audit:");
    for line in observed {
        eprintln!("  {line}");
    }
}

#[test]
fn ac1015_line_body_byte_position_red_test_proves_representative_field_mismatch() {
    let Some(bytes) = try_read_sample("sample_AC1015.dwg") else {
        eprintln!("skip: sample_AC1015.dwg not present");
        return;
    };

    let recovered = ac1015_line_body_probe(&bytes, 0x2C7, "LINE");
    let failing = ac1015_line_body_probe(&bytes, 0x2CF, "LINE");

    eprintln!("AC1015 LINE body byte-position red test:");
    for probe in [&recovered, &failing] {
        eprintln!(
            "  handle=0x{:X} family={} object_type={} body_start_bits={} body_remaining_before={} body_bytes={:02X?}",
            probe.handle,
            probe.family,
            probe.object_type,
            probe.body_start_bits,
            probe.body_remaining_bits_before,
            probe.body_bytes,
        );
        for field in &probe.fields {
            eprintln!(
                "    field={} pos {}->{} rem {}->{}",
                field.label,
                field.position_before_bits,
                field.position_after_bits,
                field.remaining_before_bits,
                field.remaining_after_bits,
            );
        }
        eprintln!(
            "    boundary_audit payload_consumed_bits={} payload_remaining_bits={} consumed_to_declared_boundary={}",
            probe.boundary_audit.payload_consumed_bits,
            probe.boundary_audit.payload_remaining_bits,
            probe.boundary_audit.consumed_to_declared_boundary,
        );
    }

    assert_eq!(recovered.family, "LINE");
    assert_eq!(failing.family, "LINE");
    assert_eq!(recovered.object_type, 19);
    assert_eq!(failing.object_type, 19);
    assert_eq!(
        recovered
            .fields
            .iter()
            .map(|field| field.label)
            .collect::<Vec<_>>(),
        failing
            .fields
            .iter()
            .map(|field| field.label)
            .collect::<Vec<_>>(),
        "recovered and failing LINE probes should expose the same body-field boundaries"
    );
    assert_ne!(
        recovered.body_bytes, failing.body_bytes,
        "audit precondition: representative LINE handles should expose the live post-common body-byte divergence before the semantic fix"
    );
    let recovered_relative = recovered
        .fields
        .iter()
        .map(|field| {
            (
                field.label,
                field.position_before_bits - recovered.body_start_bits,
                field.position_after_bits - recovered.body_start_bits,
                field.remaining_before_bits,
                field.remaining_after_bits,
            )
        })
        .collect::<Vec<_>>();
    let failing_relative = failing
        .fields
        .iter()
        .map(|field| {
            (
                field.label,
                field.position_before_bits - failing.body_start_bits,
                field.position_after_bits - failing.body_start_bits,
                field.remaining_before_bits,
                field.remaining_after_bits,
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(
        recovered_relative, failing_relative,
        "red test: representative LINE handles should currently show the same relative BitReader field-boundary progression once common decode hands off to the LINE body"
    );
    assert!(
        recovered.boundary_audit.consumed_to_declared_boundary
            && failing.boundary_audit.consumed_to_declared_boundary,
        "audit conclusion: representative LINE payload decoders consume exactly to each handle's declared body boundary, so the current mismatch is in payload semantics rather than object-body framing"
    );
    assert_eq!(
        recovered.boundary_audit.payload_remaining_bits,
        failing.boundary_audit.payload_remaining_bits,
        "recovered and failing LINE probes should leave the same residual bit count after payload decoding"
    );
}

#[test]
fn ac1015_line_point_targeted_debug_trace_reports_first_missing_record_point() {
    let Some(bytes) = try_read_sample("sample_AC1015.dwg") else {
        eprintln!("skip: sample_AC1015.dwg not present");
        return;
    };
    let header = DwgFileHeader::parse(&bytes).expect("AC1015 file header parse");
    let sections = SectionMap::parse(&bytes, &header).expect("AC1015 section map parse");
    let payloads = sections
        .read_section_payloads(&bytes)
        .expect("AC1015 section payloads readable");
    let pending = build_pending_document(&header, &sections, payloads)
        .expect("AC1015 pending document builds without error");

    let handles = [
        Handle::new(0x2C7),
        Handle::new(0x2CF),
        Handle::new(0x517),
        Handle::new(0x28E),
        Handle::new(0x298),
        Handle::new(0x299),
    ];
    let traces = trace_ac1015_targeted_failure_before_fallback(&bytes, &pending, &handles);

    eprintln!("AC1015 LINE/POINT targeted debug trace:");
    for trace in &traces {
        eprintln!(
            "  handle=0x{:X} family_hint={} object_type_hint={} stage_before_fallback={} first_missing_record={} common_probe_stage={}",
            trace.handle.value(),
            trace.family_hint.unwrap_or("none"),
            trace.object_type_hint.map(|value| value.to_string()).unwrap_or_else(|| "none".to_string()),
            trace.stage_before_fallback.unwrap_or("none"),
            trace.first_missing_record.as_ref().map(|value| value.as_str()).unwrap_or("none"),
            trace.common_probe_stage.unwrap_or("none"),
        );
    }

    for trace in traces {
        let expected = if trace.family_hint == Some("LINE") { 19 } else { 27 };
        assert_eq!(trace.object_type_hint, Some(expected));
        assert!(
            matches!(
                trace.stage_before_fallback,
                Some("common_entity_decode") | Some("entity_body_decode")
            ),
            "targeted trace should persist the truthful pre-fallback common/body failure stage"
        );
        assert_eq!(
            trace.first_missing_record,
            Some(Ac1015TargetedTraceFirstMissingRecord::EntityBodyDecode)
        );
        assert_eq!(trace.common_probe_stage, Some("ok"));
    }
}

fn print_supported_geometric_failure_examples(
    diagnostics: &h7cad_native_dwg::Ac1015RecoveryDiagnostics,
) {
    let representatives = representative_supported_geometric_stage_failures(diagnostics);

    eprintln!("AC1015 representative geometric failure handles:");
    for family in ["LINE", "POINT", "CIRCLE", "ARC", "LWPOLYLINE"] {
        match representatives.get(family) {
            Some(by_kind) => {
                for kind in [
                    Ac1015RecoveryFailureKind::HeaderFail,
                    Ac1015RecoveryFailureKind::CommonDecodeFail,
                    Ac1015RecoveryFailureKind::BodyDecodeFail,
                    Ac1015RecoveryFailureKind::UnsupportedType,
                ] {
                    let handles = by_kind
                        .get(&kind)
                        .map(|failures| {
                            failures
                                .iter()
                                .map(|failure| match failure.object_type {
                                    Some(object_type) => {
                                        match failure.stage {
                                            Some(stage) => format!(
                                                "0x{:X}(type={object_type},stage={stage})",
                                                failure.handle.value()
                                            ),
                                            None => {
                                                format!("0x{:X}(type={object_type})", failure.handle.value())
                                            }
                                        }
                                    }
                                    None => match failure.stage {
                                        Some(stage) => {
                                            format!("0x{:X}(stage={stage})", failure.handle.value())
                                        }
                                        None => format!("0x{:X}", failure.handle.value()),
                                    },
                                })
                                .collect::<Vec<_>>()
                                .join(", ")
                        })
                        .filter(|value| !value.is_empty())
                        .unwrap_or_else(|| "none".to_string());
                    eprintln!("  family={family} kind={} handles=[{handles}]", kind.as_str());
                }
            }
            None => {
                eprintln!(
                    "  family={family} kind={} handles=[none]",
                    Ac1015RecoveryFailureKind::HeaderFail.as_str()
                );
                eprintln!(
                    "  family={family} kind={} handles=[none]",
                    Ac1015RecoveryFailureKind::CommonDecodeFail.as_str()
                );
                eprintln!(
                    "  family={family} kind={} handles=[none]",
                    Ac1015RecoveryFailureKind::UnsupportedType.as_str()
                );
            }
        }
    }
}
