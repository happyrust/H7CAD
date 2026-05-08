# R46-E1: AC1018 single section payload reassembly

> 起稿：2026-04-29
> 前置：R46-A（encrypted metadata）+ R46-B（LZ77 解压器）+ R46-C
> （page map）+ R46-D（section descriptors map）落地。
> R46-E1 是 R46-E 的第一块砖：把单个 AC1018 section 的多个
> LocalSectionMap pages 重组成一段完整的解压后 payload，
> 不动 `read_dwg` 顶层。R46-E2 才会把这个 payload 桥接到
> AC1015 风格的 `(SectionMap, payloads)` + `build_pending_document`。
>
> 拆分理由：R46-E 整体涉及 4 件事——单 page 重组 + section 重组 +
> 桥接 SectionMap + 接通 read_dwg。一砖一接需要先把"按 seeker 解
> 32 字节加密 page header + LZ77 解压"做对，否则后面所有桥接都
> 是空中楼阁。R46-E1 只做这第一步，独立可测、零回归。

## 1. 范围

新增独立 module `src/section_data_ac1018.rs`，导出：

- `pub const PAGE_HEADER_LEN: usize = 0x20;` —— 32 字节加密 page
  header 长度。
- `pub const PAGE_HEADER_XOR_MAGIC: u32 = 0x4164_536B;` ——
  ACadSharp `decryptDataSection` 的 XOR base：
  `secMask = PAGE_HEADER_XOR_MAGIC ^ (position as u32)`。
- `pub const DATA_SECTION_PAGE_TYPE: u32 = 0x4163_043B;` ——
  解密后 page header 的 `section_type` 字段必须等于此值。
- `pub struct EncryptedPageHeader { section_type, section_number,
  compressed_size, page_size, start_offset, page_header_checksum,
  data_checksum, oda }` —— 8 个 little-endian u32 字段。
- `pub fn decrypt_page_header(bytes: &[u8], page_offset: usize) ->
  Result<EncryptedPageHeader, SectionDataDecodeError>` —— 读 32 字节
  原始数据，按 `secMask = MAGIC ^ position` XOR 解密，逐字段返回。
- `pub fn read_section_payload(bytes: &[u8], descriptor:
  &Ac1018SectionDescriptor) -> Result<Vec<u8>, SectionDataDecodeError>`
  —— end-to-end 入口：遍历 `descriptor.local_sections`，对每个
  page 跳到 `seeker`，解密 32 字节 page header，按
  `descriptor.compressed_code` 决定 LZ77 解压或直读，按 ACadSharp
  `getSectionBuffer18` 拼装顺序写入输出 buffer；遇到 `IsEmpty`
  page 用 0 填充。
- `pub enum SectionDataDecodeError { TruncatedPageHeader,
  InvalidPageType, PageOutOfBounds, Lz77, EmptyDescriptor,
  CompressedSizeMismatch }` —— 显式错误，独立于 `DwgReadError`。

R46-E1 **不修改** `file_header.rs::section_count_offset` /
`DwgFileHeader::parse` / `read_dwg`，零回归。R46-E2 才接通顶层。

## 2. ACadSharp 参考映射

### 2.1 `getSectionBuffer18`（DwgReader.cs L1032..L1076）

```text
foreach (LocalSectionMap section in descriptor.LocalSections) {
    if (section.IsEmpty) {
        // Fill DecompressedSize bytes of 0
        memoryStream.Write(new byte[section.DecompressedSize]);
    } else {
        sreader.Position = section.Seeker;
        decryptDataSection(section, sreader);  // 32-byte XOR'd header
        if (descriptor.IsCompressed) {
            // Sets CompressedSize from the decrypted header
            DwgLZ77AC18Decompressor.DecompressToDest(
                sreader.Stream, memoryStream);
        } else {
            // Verbatim copy of CompressedSize bytes
            sreader.Stream.CopyTo(memoryStream, section.CompressedSize);
        }
    }
}
```

### 2.2 `decryptDataSection`（DwgReader.cs L1078..L1100）

```text
int secMask = 0x4164536B ^ (int)sreader.Position;
//0x00 4 page_type (== 0x4163043B)
var pageType            = ReadRawLong() ^ secMask;
//0x04 4 section_number
var sectionNumber       = ReadRawLong() ^ secMask;
//0x08 4 compressed_size (overwrites descriptor's value)
section.CompressedSize  = (ulong)(ReadRawLong() ^ secMask);
//0x0C 4 page_size (decompressed)
section.PageSize        = ReadRawLong() ^ secMask;
//0x10 4 start_offset (in decompressed buffer)
var startOffset         = ReadRawLong() ^ secMask;
//0x14 4 page_header_checksum
var checksum            = ReadRawLong() ^ secMask;
section.Offset          = (ulong)(checksum + startOffset);
//0x18 4 data_checksum
section.Checksum        = (uint)(ReadRawLong() ^ secMask);
//0x1C 4 ODA (== 0)
var oda                 = (uint)(ReadRawLong() ^ secMask);
```

ACadSharp 没有显式校验 `pageType == 0x4163043B` 的步骤；R46-E1
**会**校验，因为这是发现"我们读错位置 / 把 system page 当 data page
读"的最强信号。

## 3. EncryptedPageHeader 字段

| Offset | Size | Field | Notes |
|---:|---:|---|---|
| 0x00 | 4 | `section_type` (i32 LE, must equal 0x4163_043B) | XOR-decrypted |
| 0x04 | 4 | `section_number` (i32 LE) | section descriptor 的 section_id |
| 0x08 | 4 | `compressed_size` (u32 LE) | 覆盖 descriptor 的 compressed_size |
| 0x0C | 4 | `page_size` (u32 LE) | decompressed page size |
| 0x10 | 4 | `start_offset` (u32 LE) | 在解压后 buffer 内的起始偏移 |
| 0x14 | 4 | `page_header_checksum` (u32 LE) | 仅记录，不验证 |
| 0x18 | 4 | `data_checksum` (u32 LE) | 仅记录，不验证 |
| 0x1C | 4 | `oda` (u32 LE, == 0) | ACadSharp 写 0 |

## 4. 算法

```rust
pub fn read_section_payload(
    bytes: &[u8],
    descriptor: &Ac1018SectionDescriptor,
) -> Result<Vec<u8>, SectionDataDecodeError> {
    if descriptor.local_sections.is_empty() {
        return Err(SectionDataDecodeError::EmptyDescriptor {
            name: descriptor.name.clone(),
        });
    }
    let mut out = Vec::with_capacity(
        (descriptor.decompressed_size as usize)
            .saturating_mul(descriptor.local_sections.len()),
    );
    for local in &descriptor.local_sections {
        // R46-E1 does not yet model "empty page" detection — ACadSharp
        // sets IsEmpty when the LocalSectionMap is synthesised for a
        // gap; our descriptor map does not synthesise gap entries
        // (R46-D parses them straight from disk). All R46-D
        // local_sections are present on disk → always read.

        let page_offset: usize = local.seeker.try_into().map_err(|_| {
            SectionDataDecodeError::PageOutOfBounds {
                seeker: local.seeker,
                file_len: bytes.len(),
            }
        })?;
        let header = decrypt_page_header(bytes, page_offset)?;
        if header.section_type != DATA_SECTION_PAGE_TYPE {
            return Err(SectionDataDecodeError::InvalidPageType {
                actual: header.section_type,
                offset: page_offset,
            });
        }

        let payload_start = page_offset + PAGE_HEADER_LEN;
        let compressed_size = header.compressed_size as usize;
        let payload_end = payload_start.checked_add(compressed_size).ok_or(
            SectionDataDecodeError::PageOutOfBounds {
                seeker: local.seeker,
                file_len: bytes.len(),
            },
        )?;
        if payload_end > bytes.len() {
            return Err(SectionDataDecodeError::PageOutOfBounds {
                seeker: local.seeker,
                file_len: bytes.len(),
            });
        }
        let compressed = &bytes[payload_start..payload_end];

        if descriptor.compressed_code == 2 {
            // LZ77-compressed page. Decompressed size is given by
            // `header.page_size` — same as descriptor.decompressed_size
            // for non-tail pages, sizeLeft for the last page.
            let decompressed = decompress_ac18_lz77(
                compressed,
                local.decompressed_size as usize,
            )?;
            out.extend_from_slice(&decompressed);
        } else {
            // compressed_code == 1 (no compression) or other: copy
            // verbatim. ACadSharp also defaults to 2; we accept 1
            // gracefully but reject anything else.
            if descriptor.compressed_code != 1 {
                return Err(SectionDataDecodeError::UnsupportedCompressedCode {
                    actual: descriptor.compressed_code,
                });
            }
            out.extend_from_slice(compressed);
        }
    }
    Ok(out)
}
```

## 5. 验收

```bash
cargo test -p h7cad-native-dwg --lib section_data_ac1018
cargo test -p h7cad-native-dwg --test real_samples ac1018_section_data -- --nocapture
cargo test --locked --workspace --all-targets
RUSTFLAGS=-Dwarnings cargo check --locked --workspace --all-targets
```

通过标准：

- 单元测试 `decrypt_page_header_decodes_synthetic` pass：合成 32
  字节加密 header（pos=0x100，所有字段 XOR'd）解出与原始字段
  一致。
- 单元测试 `decrypt_page_header_position_dependent_xor` pass：相同
  原始字段在不同 page_offset 上加密后，解密结果保持一致（验证
  position-aware secMask 实现）。
- 单元测试 `decrypt_page_header_rejects_truncated_input` pass：
  31 字节输入返回 `TruncatedPageHeader`。
- 单元测试 `read_section_payload_decompresses_single_page` pass：
  合成 1 page, compressed_code=2, LZ77 leading-literal preamble，
  解出与原始 raw bytes 一致。
- 单元测试 `read_section_payload_concatenates_multiple_pages` pass：
  合成 2 page (compressed_code=2)，解出 buffer 长度等于两 page 的
  解压尺寸之和，内容是两 page 拼接。
- 单元测试 `read_section_payload_handles_uncompressed_section` pass：
  compressed_code=1 时直接复制 compressed_size 字节。
- 单元测试 `read_section_payload_rejects_invalid_page_type` pass：
  pageType ≠ 0x4163_043B 时报 `InvalidPageType`。
- 单元测试 `read_section_payload_rejects_unsupported_compression_code`
  pass：compressed_code 不在 {1, 2} 时报
  `UnsupportedCompressedCode`。
- 单元测试 `read_section_payload_rejects_page_out_of_bounds` pass：
  seeker > bytes.len() 时报 `PageOutOfBounds`。
- 单元测试 `read_section_payload_rejects_empty_descriptor` pass：
  descriptor.local_sections.is_empty() 时报 `EmptyDescriptor`。
- 单元测试 `section_data_decode_error_display_strings_include_diagnostics`
  pass：每个错误变体的 Display 字符串含关键诊断信息。
- 集成测试 `ac1018_section_data_decompresses_real_acdb_header` pass
  （sample 缺失 soft-skip）：sample_AC1018.dwg 解出 `AcDb:Header`
  section payload，前 16 字节匹配 `KnownSection::Header.start_sentinel()`
  （`CF 7B 1F 23 FD DE 38 A9 5F 7C 68 B8 4E 6D 33 5F`）。
- 集成测试 `ac1018_section_data_decompresses_real_acdb_handles` pass
  （sample 缺失 soft-skip）：sample_AC1018.dwg 解出 `AcDb:Handles`
  section payload，长度 ≥ 64（保守下界，用于检测真实样本上的
  R46-E1 端到端能否产出非空 handle map）。
- workspace test 全 ok / 0 failed；
- `-Dwarnings cargo check workspace` 干净；
- 不修改 `DwgFileHeader::parse` / `read_dwg` 行为，无回归。

## 6. 任务

| T | 描述 | 状态 |
|---:|---|---|
| T1 | 落 R46-E1 子 plan（本文件） | ✅ 完成 |
| T2 | 新增 `src/section_data_ac1018.rs`：types + parsers + 12 个单元测试 | ✅ 完成 |
| T3 | 在 `lib.rs` 接 `mod section_data_ac1018` 并 pub 关键 API | ✅ 完成 |
| T4 | 在 `tests/real_samples.rs` 加 2 个 AC1018 sample 集成测试（AcDb:Header sentinel match + AcDb:Handles ≥ 64 bytes） | ✅ 完成 |
| T5 | 双重门验收（cargo test workspace 945/0；-Dwarnings cargo check 1.7s ok） | ✅ 完成 |

## 7. 不纳入

- AC1018 read_dwg 顶层接通（R46-E2）；
- AC1018 → AC1015 风格 SectionMap 桥接（R46-E2）；
- 修改 `DwgFileHeader::parse`（R46-E2）；
- AC1018 写出（writer 路径）；
- ACIS / SummaryInfo / VbaProject 等可选 section 解析（不在 R46
  scope）；
- page header checksum 验证；
- data checksum 验证；
- IsEmpty page 处理（R46-D 解出的 local_sections 都是真实 page，不
  含 ACadSharp 的合成 gap entry）。

## 8. 风险

- **secMask signedness**：ACadSharp 用 `int` (signed 32-bit)，但
  XOR 是位级操作，signedness 不影响结果；Rust 端用 `u32::wrapping_*`
  保持一致。
- **Position-aware secMask**：`secMask = MAGIC ^ position` 意味着
  page header 不能 cache，必须每次重算。R46-E1 的 `decrypt_page_header`
  把 `page_offset` 作为参数显式传入，避免 hidden state。
- **compressed_size header vs descriptor**：ACadSharp 的
  `decryptDataSection` **覆盖**了 `section.CompressedSize`，因为
  descriptor 表里的值是"全 section 总和"，header 里的才是"本 page"
  的真实值。R46-E1 的 `read_section_payload` 用 `header.compressed_size`，
  忽略 `local.compressed_size`，但保留后者用于 R46-E2 的诊断。
- **decompressed_size for last page**：R46-D 已应用 ACadSharp 的
  tail correction（`decompressed_size = compressed_size %
  descriptor.decompressed_size`），R46-E1 直接用
  `local.decompressed_size`。
- **LZ77 输出长度未校验**：R46-B 的 `decompress_ac18_lz77` 接受
  `decompressed_size` 但只用作 OOM 防御 cap，不强制 == 期望值。
  R46-E1 不强制等长（实际 sample 上 page_size 与 LZ77 输出可能
  差 1 字节，因 ODA 的 page header padding；R46-E2 接通时再观察
  是否需要严格校验）。

## 9. 状态

- [x] T1 R46-E1 plan（本文件）
- [x] T2 section_data_ac1018.rs 实现 + 12 个单元测试 pass（synthetic header round-trip at offset 0、position-dependent XOR cross-check、truncated header reject、single-page LZ77 round-trip、two-page concat、uncompressed-section verbatim copy、invalid page_type reject、unsupported compressed_code reject、page out-of-bounds reject、negative seeker reject、empty descriptor reject、Display 字符串含诊断）
- [x] T3 lib.rs mod 接通 + pub（`read_ac1018_section_payload`、`decrypt_ac1018_page_header`、`Ac1018EncryptedPageHeader`、`SectionDataDecodeError`、3 个常量 `DATA_SECTION_PAGE_TYPE` / `PAGE_HEADER_LEN` / `PAGE_HEADER_XOR_MAGIC`）
- [x] T4 real_samples.rs 集成测试（2 个 pass：`ac1018_section_data_decompresses_real_acdb_header` AcDb:Header 解出 29696 bytes，前 16 字节匹配 ODA start sentinel；`ac1018_section_data_decompresses_real_acdb_handles` AcDb:Handles 解出 ≥ 64 bytes）
- [x] T5 双重门验收（workspace test **945 passed / 0 failed**；h7cad-native-dwg lib 139→151；real_samples 33→35；RUSTFLAGS=-Dwarnings cargo check workspace 1.7s ok）

## 11. 真实 sample 实测发现（R46-E2 衔接）

```text
sample_AC1018.dwg AcDb:Header section:
  page #0 seeker=0x10B560
  page header (decrypted): section_type=0x4163043B section_number=1
                           compressed_size=740 page_size=800
                           start_offset=0 oda=470342425
  LZ77 stream first 18 bytes: 00 3A CF 7B 1F 23 FD DE 38 A9 5F 7C
                              68 B8 4E 6D 33 5F
  → opcode1=0x00 → leading literal_count chain
    chain byte=0x3A → lowbits=0x0F+0x3A=0x49=73, +3=76 literal bytes
  → first 16 literal bytes = ODA Header start sentinel
    (CF 7B 1F 23 FD DE 38 A9 5F 7C 68 B8 4E 6D 33 5F) ✓
  → total decompressed: 29696 bytes (≈ 0x7400, ODA "max page size")
```

**关键陷阱**（必须记入 R46-E2 知识库）：

1. **page header 上的 `page_size` 字段不可信**。sample_AC1018.dwg 的
   AcDb:Header page header 写 `page_size=800`，但实际 LZ77 解出
   29696 bytes（远超声明）。ACadSharp 的 `DecompressToDest` 不用
   `page_size` 作为 cap，只读到 0x11 终止符。R46-E1 用
   `MAX_LZ77_OUTPUT_PER_PAGE = 16 MiB` 作为防御 cap，与 R46-C
   的 `MAX_DECOMPRESSED_SIZE` 对齐。
2. **page header 的 `oda` 字段不为 0**。ACadSharp 文档说"ODA writes
   0"，但 AutoCAD 实际写入非零值（实测 0x1C0E0E59）。R46-E1 不
   校验该字段，仅记录。
3. **page header 的 `start_offset` / `page_header_checksum` /
   `data_checksum`** 当前未消费；R46-E2 接通 `build_pending_document`
   时若不需要可永久忽略，若需要再按 ACadSharp 的
   `decryptDataSection` 末尾的 `Offset = checksum + startOffset`
   公式合成。

## 10. R46-E2 衔接

R46-E2 用 R46-E1 的 `read_section_payload` 给每个 R46-D 解出的
descriptor 生成完整的 section payload，按 name → record_number 映射
（用 `KnownSection::from_name` 反向查找）合成 AC1015 风格的
`SectionMap` + `Vec<Vec<u8>>` payloads，然后调用现有的
`build_pending_document` + `resolve_document` +
`enrich_with_real_entities`，让 `read_dwg(sample_AC1018_bytes)`
返回 Ok(doc) 且 entity_count > 0。

`DwgFileHeader::parse` 在 R46-E2 才会扩展到 AC1018：AC1018 分支
依次调用 `parse_ac1018_encrypted_metadata` →
`parse_ac1018_page_map` → `parse_ac1018_section_map` →
（每个 descriptor）`read_section_payload` → 合成 SectionMap，
让 `read_dwg` 顶层无缝复用。
