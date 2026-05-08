# R46-D: AC1018 section descriptors map decoder

> 起稿：2026-04-29
> 前置：R46-A（encrypted metadata）+ R46-B（LZ77 解压缩器）+ R46-C
> （page map）落地。
> R46-D 把 R46-A/B/C 拼起来：用 R46-C 解出的 `PageMap` 找到
> `section_map_id` 对应 page 的物理 seeker，从该 seeker 处读 system
> page header（section_type = `SECTION_MAP_SECTION_TYPE`），LZ77
> 解压后解 SectionDescriptor 数组（含 8+ 核心 section：
> AcDb:Handles / AcDb:AcDbObjects / AcDb:Header / AcDb:Classes 等）。

## 1. 范围

新增独立 module `src/section_map_ac1018.rs`，导出：

- `pub struct LocalSectionMap { page_number, compressed_size, offset,
  decompressed_size, seeker }` —— 单个 section 内的 page 信息，
  `seeker` 来自 page_map.lookup(page_number)。
- `pub struct SectionDescriptor { compressed_size, page_count,
  decompressed_size, compressed_code, section_id, encrypted, name,
  local_sections }` —— ACadSharp `DwgSectionDescriptor` 的 Rust
  端口；只保留 read path 必要字段。
- `pub struct SectionDescriptorMap { descriptors: BTreeMap<String,
  SectionDescriptor> }` —— 用 name 索引，配 `lookup` / `iter` /
  `len` API。
- `pub enum SectionMapDecodeError { PageMap, InvalidPageType,
  UnsupportedCompressionType, Lz77, TruncatedDescriptorStream,
  TruncatedHeaderStream, MissingPageInPageMap, InvalidNameEncoding,
  OversizedDecompressedSize }` —— 显式错误，独立于
  `DwgReadError`，保持 R46 brick 风格一致；R46-E 接通时再做
  `From` 转换。
- `pub fn parse_ac1018_section_map(bytes: &[u8], page_map: &PageMap,
  section_map_id: u32) -> Result<SectionDescriptorMap,
  SectionMapDecodeError>` —— end-to-end 入口，复用 R46-C 的
  `parse_system_page_header` + `decompress_ac18_lz77`。

R46-D **保持 `file_header.rs::section_count_offset` 仍对 AC1018
返回 `UnsupportedHeaderLayout`**，不动 `read_dwg` 顶层路径，
零回归。R46-E 才接通 `build_pending_document`。

## 2. ACadSharp 参考映射（DwgReader.cs L571..L646）

```text
// 1) Position to section descriptor map page seeker via page_map
sreader.Position = fileheader.Records[(int)fileheader.SectionMapId].Seeker;

// 2) Read 20-byte system page header (section_type = SECTION_MAP_SECTION_TYPE)
this.getPageHeaderData(sreader,
    out _,                       // sectionType (must equal SECTION_MAP_SECTION_TYPE = 0x4163_003B)
    out long decompressedSize,
    out _,                       // compressedSize
    out _,                       // compressionType (== 0x02)
    out _);                      // checksum

// 3) Decompress with R46-B
StreamIO decompressed = new StreamIO(
    DwgLZ77AC18Decompressor.Decompress(sreader.Stream, decompressedSize));

// 4) Read 20-byte SectionDescriptorMap header
int ndescriptions = decompressed.ReadInt();    // 0x00 4
decompressed.ReadInt();                         // 0x04 4 (== 0x02)
decompressed.ReadInt();                         // 0x08 4 (== 0x7400)
decompressed.ReadInt();                         // 0x0C 4 (== 0x00)
decompressed.ReadInt();                         // 0x10 4 (Unknown, ODA writes ndescriptions)

// 5) For each descriptor (variable size: 100 bytes + 12 * page_count)
for (int i = 0; i < ndescriptions; ++i) {
    DwgSectionDescriptor descriptor = new DwgSectionDescriptor();
    descriptor.CompressedSize  = decompressed.ReadULong();    // 0x00 8
    descriptor.PageCount       = decompressed.ReadInt();      // 0x08 4
    descriptor.DecompressedSize = (ulong)decompressed.ReadInt(); // 0x0C 4
    decompressed.ReadInt();                                    // 0x10 4 Unknown
    descriptor.CompressedCode  = decompressed.ReadInt();      // 0x14 4 (1 = no, 2 = yes)
    descriptor.SectionId       = decompressed.ReadInt();      // 0x18 4
    descriptor.Encrypted       = decompressed.ReadInt();      // 0x1C 4
    descriptor.Name            = decompressed.ReadString(64).Split('\0')[0]; // 0x20 64

    for (int j = 0; j < descriptor.PageCount; ++j) {
        DwgLocalSectionMap localmap = new DwgLocalSectionMap();
        localmap.PageNumber     = decompressed.ReadInt();     // 0x00 4
        localmap.CompressedSize = (ulong)decompressed.ReadInt(); // 0x04 4
        localmap.Offset         = decompressed.ReadULong();   // 0x08 8

        localmap.DecompressedSize = descriptor.DecompressedSize;
        localmap.Seeker = fileheader.Records[localmap.PageNumber].Seeker;
        descriptor.LocalSections.Add(localmap);
    }

    // Final-page tail size correction
    uint sizeLeft = (uint)(descriptor.CompressedSize % descriptor.DecompressedSize);
    if (sizeLeft > 0U && descriptor.LocalSections.Count > 0)
        descriptor.LocalSections[Last].DecompressedSize = sizeLeft;

    fileheader.Descriptors.Add(descriptor.Name, descriptor);
}
```

## 3. SectionDescriptorMap 头部字段（20 字节）

| Offset | Size | Field | Notes |
|---:|---:|---|---|
| 0x00 | 4 | `num_descriptions` (i32 LE) | 描述符数量 |
| 0x04 | 4 | `unknown_04` (i32, == 0x02) | 固定 |
| 0x08 | 4 | `unknown_08` (i32, == 0x7400) | 固定 max page size |
| 0x0C | 4 | `unknown_0c` (i32, == 0x00) | 固定 |
| 0x10 | 4 | `unknown_10` (i32, ODA writes num_descriptions) | 信息冗余 |

## 4. SectionDescriptor 字段（每条 96 字节 + 16 * page_count）

| Offset | Size | Field | Type |
|---:|---:|---|---|
| 0x00 | 8 | `compressed_size` | `u64` |
| 0x08 | 4 | `page_count` | `i32` |
| 0x0C | 4 | `decompressed_size` (normally 0x7400) | `u32` (i32 on wire, but always positive) |
| 0x10 | 4 | unknown | `i32` |
| 0x14 | 4 | `compressed_code` (1=no, 2=yes) | `i32` |
| 0x18 | 4 | `section_id` | `i32` |
| 0x1C | 4 | `encrypted` (0/1/2) | `i32` |
| 0x20 | 64 | `name` (null-terminated string) | `[u8; 64]` → `String` |

## 5. LocalSectionMap 字段（每条 16 字节）

| Offset | Size | Field | Notes |
|---:|---:|---|---|
| 0x00 | 4 | `page_number` (i32 LE) | index into PageMap |
| 0x04 | 4 | `compressed_size` (i32 LE on wire) | upgraded to u64 in struct |
| 0x08 | 8 | `offset` (u64 LE) | logical offset within section |
| (computed) | — | `seeker` | resolved via `page_map.lookup(page_number).seeker` |
| (computed) | — | `decompressed_size` | inherited from descriptor; tail-corrected for last page |

## 6. 验收

```bash
cargo test -p h7cad-native-dwg --lib section_map_ac1018
cargo test -p h7cad-native-dwg --test real_samples ac1018_section_map -- --nocapture
cargo test --locked --workspace --all-targets
RUSTFLAGS=-Dwarnings cargo check --locked --workspace --all-targets
```

通过标准：

- 单元测试 `parse_section_descriptor_decodes_synthetic` pass：
  20 字节头 + 1 描述符（page_count=0，96 字节）+ 1 描述符
  （page_count=2，96 + 32 = 128 字节）合成 fixture 解出 2 个
  SectionDescriptor，包含 name / id / sizes / local_sections。
- 单元测试 `parse_section_descriptor_handles_zero_page_count` pass：
  page_count=0 的 descriptor（如 AcDb:Empty）解出后 local_sections
  为空。
- 单元测试 `parse_section_descriptor_resolves_local_section_seeker_via_page_map`
  pass：合成 PageMap 包含 page_number 引用，local_section.seeker
  正确取自 page_map.lookup(page_number).seeker。
- 单元测试 `parse_section_descriptor_corrects_last_page_decompressed_size`
  pass：当 compressed_size % decompressed_size > 0 时，最后一个
  local_section 的 decompressed_size 被修正为 sizeLeft。
- 单元测试 `parse_ac1018_section_map_rejects_invalid_section_type`
  pass：page_type != SECTION_MAP_SECTION_TYPE 报 `InvalidPageType`。
- 单元测试 `parse_ac1018_section_map_rejects_unsupported_compression`
  pass：compression_type != 0x02 报 `UnsupportedCompressionType`。
- 单元测试 `parse_ac1018_section_map_rejects_oversized_decompressed_size`
  pass：decompressed_size > 16 MiB 报 `OversizedDecompressedSize`。
- 单元测试 `parse_ac1018_section_map_rejects_truncated_descriptor_stream`
  pass：descriptor 字段在 100 字节边界外被截断时报
  `TruncatedDescriptorStream`。
- 单元测试 `parse_ac1018_section_map_rejects_missing_page_in_page_map`
  pass：local_section.page_number 在 PageMap 里查不到时报
  `MissingPageInPageMap`。
- 单元测试 `parse_ac1018_section_map_handles_invalid_utf8_name_gracefully`
  pass：name 字段含非 UTF-8 字节时退化为 lossy（不 panic），仍能
  把 descriptor 入库。
- 单元测试 `section_map_decode_error_display_strings_include_diagnostics`
  pass：每个错误变体的 Display 字符串包含关键诊断信息（offset /
  page_number / actual value）。
- 集成测试 `ac1018_section_map_decodes_real_sample` pass（sample
  缺失 soft-skip）：sample_AC1018.dwg 在 R46-C 实测的
  `section_descriptor_map.seeker = 0x10B880` 处解出 N ≥ 8 个
  descriptors。
- 集成测试 `ac1018_section_map_real_sample_contains_core_sections`
  pass：descriptors 字典中包含至少这些核心 section：
  `AcDb:Handles` / `AcDb:AcDbObjects` / `AcDb:Header` /
  `AcDb:Classes` / `AcDb:ObjFreeSpace` / `AcDb:Template` /
  `AcDb:AuxHeader` / `AcDb:SummaryInfo`（至少 6 个出现）。
- workspace test 全 ok / 0 failed；
- `-Dwarnings cargo check workspace` 干净；
- 不修改 `DwgFileHeader::parse` / `read_dwg` 行为，无回归。

## 7. 任务

| T | 描述 | 状态 |
|---:|---|---|
| T1 | 落 R46-D 子 plan（本文件） | ✅ 完成 |
| T2 | 新增 `src/section_map_ac1018.rs`：types + parsers + 单元测试 | ✅ 完成（14 个单元测试 pass） |
| T3 | 在 `lib.rs` 接 `mod section_map_ac1018` 并 pub 关键 API（带 `Ac1018` 前缀避免与 AC1015 `SectionDescriptor` 同名冲突） | ✅ 完成 |
| T4 | 在 `tests/real_samples.rs` 加 2 个 AC1018 sample 集成测试 | ✅ 完成（2 个 sample test pass） |
| T5 | 双重门验收 | ✅ 完成 |

## 8. 不纳入

- AC1018 build_pending_document 接通（R46-E）；
- 修改 `DwgFileHeader::parse`（要等 R46-E）；
- 修改 `read_dwg` 顶层路径（R46-E）；
- AC1018 写出（writer 路径）；
- section data 解密（`encrypted == 1`，real-world AC1018 几乎全 0）；
- section data LZ77 解压（不在 R46-D scope；R46-E 才需要）；
- compressed_code = 1（无压缩）的 descriptor 处理（real-world 几乎
  全 2，R46-D 接受但 R46-E 才需要消费）。

## 9. 风险

- **`seeker` 类型分裂**：ACadSharp 用 `long` 存 seeker，但 R46-C 已
  把 `PageMapRecord.seeker` 定为 `i64`。R46-D 的 `LocalSectionMap.seeker`
  必须保持 `i64` 一致，避免 R46-E 接通时类型错位。
- **64 字节 name 的 UTF-8 假设**：DWG 把 name 视为
  Windows-1252，但实测 sample 的 name 都是 ASCII（`AcDb:Handles`
  等）。R46-D 用 `String::from_utf8_lossy` 退化处理非 ASCII，避免
  false negative，但留 `InvalidNameEncoding` error 给未来更严格的
  验证模式。
- **Tail-page decompressed_size 修正**：ACadSharp 的逻辑
  `sizeLeft = compressed_size % decompressed_size; if sizeLeft >
  0` 在 Rust 端必须用 `u64` 算，且只在 `local_sections` 非空时
  修正。
- **PageMap 缺失 page_number**：理论上不应发生（每个 LocalSection
  必定指向一个 valid PageMap entry），但若 sample 异常我们也要给
  明确错误。`MissingPageInPageMap` 直接报错 + handle 信息，便于
  R46-E 调试。
- **section_map_id 来自 R46-A**：本砖通过 caller 显式传入，避免
  依赖 R46-A 数据结构；这样单元测试不需要构造完整的
  `Ac1018EncryptedMetadata`，只需要构造 `PageMap` + `section_map_id`。

## 10. R46-E 衔接

R46-E 用 R46-D 解出的 `SectionDescriptorMap` 找到核心 section
（`AcDb:Handles` / `AcDb:AcDbObjects` 等）的 `LocalSectionMap`
列表，按 `seeker` 读每个 page 的 LZ77 数据，解压并拼装出
section payload，然后**复用 AC1015 entity decoding**
（`build_pending_document` + `enrich_with_real_entities`）跑出
entity recovery。

`DwgFileHeader::parse` 在 R46-E 才会扩展到 AC1018：
- AC1018 分支调用 `parse_ac1018_encrypted_metadata` →
  `parse_ac1018_page_map` → `parse_ac1018_section_map` →
  装配出与 AC1015 兼容的 `(SectionMap, payloads)`，
  让 `read_dwg` 顶层 `pending → resolve` 链路无缝复用。

## 11. 状态

- [x] T1 R46-D plan（本文件）
- [x] T2 section_map_ac1018.rs 实现 + 单元测试（14 个 pass：synthetic two-descriptor payload、zero page_count、seeker resolution via PageMap、tail-page decompressed_size correction、truncated header stream reject、truncated descriptor stream reject、missing page_in_page_map reject、invalid utf8 name graceful、end-to-end synthetic via parse_ac1018_section_map happy path、invalid section_type reject、unsupported compression reject、oversized decompressed_size reject、page_map error fallthrough when section_id missing、Display 字符串含诊断）
- [x] T3 lib.rs mod 接通 + pub（`parse_ac1018_section_map`、`parse_ac1018_section_descriptors`、`Ac1018LocalSectionMap`、`Ac1018SectionDescriptor`、`Ac1018SectionDescriptorMap`、`SectionMapDecodeError`、4 个常量）
- [x] T4 real_samples.rs 集成测试（2 个 pass：`ac1018_section_map_decodes_real_sample` descriptor_count=13；`ac1018_section_map_real_sample_contains_core_sections` AcDb:Header / Handles / AcDbObjects / Classes / ObjFreeSpace / Template / AuxHeader 全部命中）
- [x] T5 双重门验收（workspace test 全 ok / 0 failed；h7cad-native-dwg lib 125→139；real_samples 31→33；RUSTFLAGS=-Dwarnings cargo check workspace 5.0s ok）

## 12. sample_AC1018.dwg 实测数据（R46-E 用）

```text
descriptor_count=13
order_head=["", "AcDb:AppInfoHistory", "AcDb:AppInfo", "AcDb:Preview",
            "AcDb:SummaryInfo", "AcDb:RevHistory", "AcDb:AcDbObjects",
            "AcDb:ObjFreeSpace"]
required core (R46-E will index by name):
  AcDb:Header / AcDb:Handles / AcDb:AcDbObjects / AcDb:Classes /
  AcDb:ObjFreeSpace / AcDb:Template / AcDb:AuxHeader  ← 全部 ✓
optional metadata sections present on this sample:
  AcDb:AppInfoHistory / AcDb:AppInfo / AcDb:Preview /
  AcDb:SummaryInfo / AcDb:RevHistory
empty-name section "" 出现在第 0 位（ACadSharp 的 "section 0 is the
conventional empty section, the rest count down from N-1 to 1"，与
DwgReader.cs L611 的注释一致；R46-E 通过 `section_id` 区分）
```
