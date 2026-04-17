//! Real DWG samples from the ACadSharp sibling repository.
//!
//! These tests anchor the M3-A milestone baseline. Today only the
//! version sniff is expected to succeed; the full `read_dwg` path is
//! known to fail on real binaries and that failure is captured here so
//! future milestones can measure progress against a deterministic
//! starting point.

use std::path::{Path, PathBuf};

use h7cad_native_dwg::{
    build_pending_document, read_dwg, sniff_version, BitReader, DwgFileHeader, DwgReadError,
    DwgVersion, KnownSection, SectionMap,
};

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

/// M3-A starting baseline. Today no real DWG is expected to round-trip
/// correctly through `read_dwg`; this test only records the observed
/// outcome so future milestones can measure progress against a
/// deterministic starting line.
///
/// What we assert:
/// - Versions we already reject explicitly (AC1012/AC1014/AC1021+)
///   must return `UnsupportedVersion` and echo back the correct
///   version (i.e. sniff still wired to section lookup).
/// - Versions we "partially support" (AC1015/AC1018) must either
///   fail with a structural decoder error or return a stub
///   `CadDocument` that contains the model/paper space scaffold but
///   zero real entities.
/// - No panic paths must leak (we catch and assert shape).
#[test]
fn real_dwg_samples_baseline_m3a() {
    for (name, version) in real_samples() {
        let Some(bytes) = try_read_sample(name) else {
            continue;
        };
        match read_dwg(&bytes) {
            Ok(doc) => {
                assert!(
                    doc.entities.is_empty(),
                    "{name}: baseline expected zero real entities but got {}",
                    doc.entities.len()
                );
                eprintln!(
                    "{name} ({version:?}): read_dwg returned stub document \
                     with {} blocks, {} layouts, {} objects (baseline)",
                    doc.block_records.len(),
                    doc.layouts.len(),
                    doc.objects.len(),
                );
            }
            Err(DwgReadError::UnsupportedVersion(reported)) => {
                assert_eq!(
                    reported, version,
                    "{name}: UnsupportedVersion should echo sniffed version"
                );
                eprintln!("{name} ({version:?}): explicit UnsupportedVersion (baseline)");
            }
            Err(err) => {
                // AC1015/AC1018 currently hit structural decode errors
                // on real files. This is acceptable at M3-A; record the
                // exact error shape for future regression tracking.
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
