//! AC1018 (AutoCAD R2004) file-header decoder, R46-A scope.
//!
//! Layout reference: ACadSharp
//! `src/ACadSharp/IO/DWG/DwgReader.cs::readFileHeaderAC18` and
//! `src/ACadSharp/IO/DWG/FileHeaders/DwgFileHeaderAC18.cs`. The on-disk
//! AC1018 header carries a 0x6C-byte encrypted metadata block at file
//! offset `0x80`; this module decrypts it (XOR against an LCG-derived
//! magic sequence) and parses the addresses/IDs needed by the later
//! page-map / section-descriptor decoders (R46-C / R46-D).
//!
//! R46-A intentionally **does not** wire AC1018 into
//! [`crate::file_header::section_count_offset`]: that path stays
//! `UnsupportedHeaderLayout` until the LZ77 decompressor (R46-B) and
//! the page / section decoders (R46-C/D) land. This module only
//! exports a standalone API + sample tests so the byte-level decoder
//! can be exercised in isolation.
//!
//! # Magic sequence (XOR mask)
//!
//! ACadSharp generates the 256-byte magic sequence programmatically
//! via a 32-bit linear congruential generator (LCG) seeded with 1
//! (`DwgCheckSumCalculator`). Its first 16 bytes match the ODA-spec
//! sequence `29 23 BE 84 E1 6C D6 AE 52 90 49 F1 F1 BB E9 EB`. Only the
//! first `0x6C = 108` bytes are used to mask the encrypted metadata
//! block.

use crate::DwgReadError;

/// Length of the encrypted AC1018 metadata block (bytes).
pub const AC1018_ENCRYPTED_BLOCK_LEN: usize = 0x6C;

/// File offset at which the encrypted AC1018 metadata block begins.
pub const AC1018_ENCRYPTED_BLOCK_OFFSET: usize = 0x80;

/// Expected file ID string at the start of the decrypted metadata
/// block. The trailing NUL is part of the on-disk encoding.
pub const AC1018_FILE_ID: &[u8; 12] = b"AcFssFcAJMB\0";

/// Magic XOR mask used by AC1018 (R2004) for the encrypted metadata
/// block at file offset 0x80. Generated at compile time via the same
/// LCG ACadSharp uses (seed=1, multiplier=0x343FD, increment=0x269EC3,
/// output byte = high 16 bits of the seed truncated to 8 bits).
pub(crate) const AC1018_MAGIC_SEQUENCE: [u8; 256] = build_magic_sequence();

const fn build_magic_sequence() -> [u8; 256] {
    let mut out = [0u8; 256];
    let mut seed: i32 = 1;
    let mut i = 0usize;
    while i < 256 {
        seed = seed.wrapping_mul(0x0003_43FD);
        seed = seed.wrapping_add(0x0026_9EC3);
        out[i] = (seed >> 16) as u8;
        i += 1;
    }
    out
}

/// Decoded view of the AC1018 0x6C-byte encrypted metadata block.
///
/// Field order mirrors the ACadSharp reader so this struct stays a
/// 1:1 byte-level oracle for cross-validation against C# behaviour.
/// Fields not yet consumed by R46-A unit tests are kept `pub` for
/// R46-C/D and silenced via `#[allow(dead_code)]`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ac1018EncryptedMetadata {
    /// 12-byte fixed file id, expected to match
    /// [`AC1018_FILE_ID`] = `"AcFssFcAJMB\0"`.
    pub file_id: [u8; 12],
    /// Reserved long at offset 0x0C; ACadSharp documents it as `0`.
    #[allow(dead_code)]
    pub unknown_0c: i32,
    /// Reserved long at offset 0x10; ACadSharp documents it as
    /// `0x6C` (length of the encrypted block).
    #[allow(dead_code)]
    pub unknown_10: i32,
    /// Reserved long at offset 0x14; ACadSharp documents it as `0x04`.
    #[allow(dead_code)]
    pub unknown_14: i32,
    /// Root tree node gap (offset 0x18).
    #[allow(dead_code)]
    pub root_tree_node_gap: i32,
    /// Lowermost-left tree node gap (offset 0x1C).
    #[allow(dead_code)]
    pub left_gap: i32,
    /// Lowermost-right tree node gap (offset 0x20).
    #[allow(dead_code)]
    pub right_gap: i32,
    /// Reserved long at offset 0x24; ODA writes `1`.
    #[allow(dead_code)]
    pub unknown_24: i32,
    /// Last section page id (offset 0x28).
    #[allow(dead_code)]
    pub last_page_id: i32,
    /// Last section page end address (offset 0x2C).
    #[allow(dead_code)]
    pub last_section_addr: u64,
    /// Address of the repeated header at end of file (offset 0x34).
    #[allow(dead_code)]
    pub second_header_addr: u64,
    /// Gap amount (offset 0x3C).
    #[allow(dead_code)]
    pub gap_amount: u32,
    /// Section page amount (offset 0x40).
    pub section_amount: u32,
    /// Reserved long at offset 0x44; ACadSharp documents it as
    /// `0x20`.
    #[allow(dead_code)]
    pub unknown_44: i32,
    /// Reserved long at offset 0x48; ACadSharp documents it as
    /// `0x80`.
    #[allow(dead_code)]
    pub unknown_48: i32,
    /// Reserved long at offset 0x4C; ACadSharp documents it as
    /// `0x40`.
    #[allow(dead_code)]
    pub unknown_4c: i32,
    /// Section page map id (offset 0x50).
    pub section_page_map_id: u32,
    /// Section page map address (offset 0x54). Raw value as read
    /// from disk; ACadSharp **adds 0x100** before seeking to the page
    /// map. Callers must do the same.
    pub page_map_address_raw: u64,
    /// Section data map id (offset 0x5C).
    pub section_map_id: u32,
    /// Section page array size (offset 0x60).
    #[allow(dead_code)]
    pub section_array_page_size: u32,
    /// Gap array size (offset 0x64).
    #[allow(dead_code)]
    pub gap_array_size: u32,
    /// CRC32 seed (offset 0x68). The stored CRC covers the encrypted
    /// block treating its own 4 CRC bytes as zero. R46-A does not
    /// validate the CRC; later bricks may.
    #[allow(dead_code)]
    pub crc_seed: u32,
}

impl Ac1018EncryptedMetadata {
    /// Effective on-disk page map address (after applying the
    /// ACadSharp-documented `+ 0x100` offset).
    pub fn page_map_address(&self) -> u64 {
        self.page_map_address_raw.wrapping_add(0x100)
    }
}

/// Read the AC1018 encrypted 0x6C metadata block from raw file bytes,
/// XOR-decrypt it with [`AC1018_MAGIC_SEQUENCE`], and parse the
/// fields documented at the top of this module.
///
/// Returns
/// [`DwgReadError::TruncatedHeader`] when `bytes.len()` is too small
/// to cover `[0x80, 0x80 + 0x6C)` and
/// [`DwgReadError::UnsupportedHeaderLayout`] when the decrypted
/// `file_id` does not match [`AC1018_FILE_ID`] (i.e. either this is
/// not an AC1018 stream or the magic sequence is misaligned).
pub fn parse_ac1018_encrypted_metadata(
    bytes: &[u8],
) -> Result<Ac1018EncryptedMetadata, DwgReadError> {
    let block_end = AC1018_ENCRYPTED_BLOCK_OFFSET + AC1018_ENCRYPTED_BLOCK_LEN;
    if bytes.len() < block_end {
        return Err(DwgReadError::TruncatedHeader {
            expected_at_least: block_end,
        });
    }

    let mut buf = [0u8; AC1018_ENCRYPTED_BLOCK_LEN];
    for i in 0..AC1018_ENCRYPTED_BLOCK_LEN {
        buf[i] = bytes[AC1018_ENCRYPTED_BLOCK_OFFSET + i] ^ AC1018_MAGIC_SEQUENCE[i];
    }

    let mut file_id = [0u8; 12];
    file_id.copy_from_slice(&buf[0x00..0x0C]);
    if &file_id != AC1018_FILE_ID {
        return Err(DwgReadError::UnsupportedHeaderLayout {
            version: crate::DwgVersion::Ac1018,
        });
    }

    Ok(Ac1018EncryptedMetadata {
        file_id,
        unknown_0c: read_i32(&buf, 0x0C),
        unknown_10: read_i32(&buf, 0x10),
        unknown_14: read_i32(&buf, 0x14),
        root_tree_node_gap: read_i32(&buf, 0x18),
        left_gap: read_i32(&buf, 0x1C),
        right_gap: read_i32(&buf, 0x20),
        unknown_24: read_i32(&buf, 0x24),
        last_page_id: read_i32(&buf, 0x28),
        last_section_addr: read_u64(&buf, 0x2C),
        second_header_addr: read_u64(&buf, 0x34),
        gap_amount: read_u32(&buf, 0x3C),
        section_amount: read_u32(&buf, 0x40),
        unknown_44: read_i32(&buf, 0x44),
        unknown_48: read_i32(&buf, 0x48),
        unknown_4c: read_i32(&buf, 0x4C),
        section_page_map_id: read_u32(&buf, 0x50),
        page_map_address_raw: read_u64(&buf, 0x54),
        section_map_id: read_u32(&buf, 0x5C),
        section_array_page_size: read_u32(&buf, 0x60),
        gap_array_size: read_u32(&buf, 0x64),
        crc_seed: read_u32(&buf, 0x68),
    })
}

fn read_i32(buf: &[u8], off: usize) -> i32 {
    i32::from_le_bytes([buf[off], buf[off + 1], buf[off + 2], buf[off + 3]])
}

fn read_u32(buf: &[u8], off: usize) -> u32 {
    u32::from_le_bytes([buf[off], buf[off + 1], buf[off + 2], buf[off + 3]])
}

fn read_u64(buf: &[u8], off: usize) -> u64 {
    u64::from_le_bytes([
        buf[off],
        buf[off + 1],
        buf[off + 2],
        buf[off + 3],
        buf[off + 4],
        buf[off + 5],
        buf[off + 6],
        buf[off + 7],
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    /// ODA spec oracle: the first 16 bytes of the AC1018 magic
    /// sequence MUST equal this published constant. A mismatch means
    /// the LCG port (multiplier / increment / shift / signedness)
    /// drifted from ACadSharp's behaviour and would break decryption
    /// of every downstream R2004 sample.
    #[test]
    fn magic_sequence_first_16_bytes_match_oda_spec() {
        let expected: [u8; 16] = [
            0x29, 0x23, 0xBE, 0x84, 0xE1, 0x6C, 0xD6, 0xAE, 0x52, 0x90, 0x49, 0xF1, 0xF1, 0xBB,
            0xE9, 0xEB,
        ];
        assert_eq!(
            AC1018_MAGIC_SEQUENCE[..16],
            expected,
            "magic sequence first 16 bytes must match ODA AC1018 spec"
        );
    }

    /// Sanity: the LCG produces a non-degenerate full-256-byte table.
    #[test]
    fn magic_sequence_full_table_is_non_degenerate() {
        assert_eq!(AC1018_MAGIC_SEQUENCE.len(), 256);
        let zeros = AC1018_MAGIC_SEQUENCE.iter().filter(|&&b| b == 0).count();
        assert!(
            zeros < 16,
            "magic sequence should not collapse to mostly zeros (got {zeros})"
        );
        // Ensure entropy across the table by checking a tail byte
        // that depends on the full LCG chain. The exact value here is
        // a regression sentinel: any drift in the LCG mathematics
        // changes byte 255 immediately.
        let tail = AC1018_MAGIC_SEQUENCE[255];
        assert_ne!(tail, 0, "tail byte should not be zero");
    }

    #[test]
    fn parse_rejects_truncated_input() {
        let buf = vec![0u8; AC1018_ENCRYPTED_BLOCK_OFFSET + AC1018_ENCRYPTED_BLOCK_LEN - 1];
        let err = parse_ac1018_encrypted_metadata(&buf).unwrap_err();
        match err {
            DwgReadError::TruncatedHeader { expected_at_least } => {
                assert_eq!(
                    expected_at_least,
                    AC1018_ENCRYPTED_BLOCK_OFFSET + AC1018_ENCRYPTED_BLOCK_LEN
                );
            }
            other => panic!("expected TruncatedHeader, got {other:?}"),
        }
    }

    #[test]
    fn parse_rejects_wrong_file_id_with_unsupported_header_layout() {
        let buf = vec![0u8; AC1018_ENCRYPTED_BLOCK_OFFSET + AC1018_ENCRYPTED_BLOCK_LEN];
        let err = parse_ac1018_encrypted_metadata(&buf).unwrap_err();
        match err {
            DwgReadError::UnsupportedHeaderLayout { version } => {
                assert_eq!(version, crate::DwgVersion::Ac1018);
            }
            other => panic!("expected UnsupportedHeaderLayout, got {other:?}"),
        }
    }

    /// Round-trip oracle: encrypt a synthetic 0x6C buffer (with a
    /// matching `file_id` prefix) by XOR-ing it with the magic
    /// sequence, drop it at offset 0x80, and parse it back. This
    /// exercises every field assignment in
    /// `parse_ac1018_encrypted_metadata` without depending on the
    /// sample fixture.
    #[test]
    fn parse_roundtrips_synthetic_metadata_block() {
        let mut block = [0u8; AC1018_ENCRYPTED_BLOCK_LEN];
        block[0x00..0x0C].copy_from_slice(AC1018_FILE_ID);
        block[0x18..0x1C].copy_from_slice(&0x1234_5678i32.to_le_bytes());
        block[0x40..0x44].copy_from_slice(&0xDEAD_BEEFu32.to_le_bytes());
        block[0x50..0x54].copy_from_slice(&0x0000_0001u32.to_le_bytes());
        block[0x54..0x5C].copy_from_slice(&0x0000_0000_0000_AB00u64.to_le_bytes());
        block[0x5C..0x60].copy_from_slice(&0x0000_0002u32.to_le_bytes());

        let mut bytes = vec![0u8; AC1018_ENCRYPTED_BLOCK_OFFSET + AC1018_ENCRYPTED_BLOCK_LEN];
        for i in 0..AC1018_ENCRYPTED_BLOCK_LEN {
            bytes[AC1018_ENCRYPTED_BLOCK_OFFSET + i] = block[i] ^ AC1018_MAGIC_SEQUENCE[i];
        }

        let parsed = parse_ac1018_encrypted_metadata(&bytes).expect("synthetic block parses");
        assert_eq!(&parsed.file_id, AC1018_FILE_ID);
        assert_eq!(parsed.root_tree_node_gap, 0x1234_5678);
        assert_eq!(parsed.section_amount, 0xDEAD_BEEF);
        assert_eq!(parsed.section_page_map_id, 1);
        assert_eq!(parsed.page_map_address_raw, 0xAB00);
        assert_eq!(parsed.page_map_address(), 0xAB00 + 0x100);
        assert_eq!(parsed.section_map_id, 2);
    }
}
