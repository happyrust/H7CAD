# R46-C: AC1018 page map decoder (system page header + LZ77 + records)

> 起稿：2026-04-28
> 前置：R46-A（encrypted metadata）+ R46-B（LZ77 解压缩器）落地。
> R46-C 把两者拼起来：在 R46-A 解出的 effective `page_map_address`
> 处读 system page header，用 R46-B 解压 page-map payload，再
> byte-level 解 records 数组。

## 1. 范围

新增独立 module `src/page_map_ac1018.rs`，导出：

- `pub const SYSTEM_PAGE_HEADER_LEN: usize = 0x14;` —— 20 字节。
- `pub const PAGE_MAP_SECTION_TYPE: u32 = 0x4163_0E3B;` —— ODA-spec
  常量，page map 特征值。
- `pub const SECTION_MAP_SECTION_TYPE: u32 = 0x4163_003B;` —— ODA-spec
  常量，section descriptor map 特征值（R46-D 复用）。
- `pub struct SystemPageHeader { section_type, decompressed_size,
  compressed_size, compression_type, checksum }` —— 1:1 反映
  ACadSharp `getPageHeaderData` 字段顺序。
- `pub fn parse_system_page_header(bytes: &[u8], offset: usize) ->
  Result<SystemPageHeader, PageMapDecodeError>`。
- `pub enum PageMapDecodeError { TruncatedInput, InvalidPageType,
  UnsupportedCompressionType, Lz77, TruncatedRecordStream,
  OversizedDecompressedSize }`。
- `pub struct PageMapRecord { number: i32, size: i32, seeker: i64 }`
  —— `number < 0` 表示 gap；`seeker` 是从 `0x100` 起累加 size 算出
  的 file 内 byte offset（仅对 `number >= 0` 有意义）。
- `pub struct PageMap { records: Vec<PageMapRecord> }`，附 `lookup`
  / `valid_records` / `valid_record` API。
- `pub fn parse_ac1018_page_map(bytes: &[u8], page_map_offset:
  usize) -> Result<PageMap, PageMapDecodeError>` —— end-to-end 入口。

R46-C **保持 `file_header.rs::section_count_offset` 仍对 AC1018
返回 `UnsupportedHeaderLayout`**，不动 `read_dwg` 顶层路径，
零回归。R46-E 才接通 build_pending_document。

## 2. ACadSharp 参考映射（DwgReader.cs L529..L569）

```text
// 1) Position to PageMapAddress (effective = raw + 0x100, R46-A)
sreader.Position = (long)fileheader.PageMapAddress;

// 2) Read 20-byte system page header
this.getPageHeaderData(sreader,
    out _,                       // sectionType (must equal PAGE_MAP_SECTION_TYPE)
    out long decompressedSize,   // u32 little-endian
    out _,                       // compressedSize
    out _,                       // compressionType (== 0x02)
    out _);                      // checksum

// 3) Decompress with R46-B
StreamIO decompressed = new StreamIO(
    DwgLZ77AC18Decompressor.Decompress(sreader.Stream, decompressedSize));

// 4) Iterate records until decompressed buffer is consumed
int total = 0x100;
while (decompressed.Position < decompressed.Length) {
    record.Number = decompressed.ReadInt();
    record.Size = decompressed.ReadInt();
    if (record.Number >= 0) {
        record.Seeker = total;
        records[record.Number] = record;
    } else {
        // 16-byte gap padding: Parent, Left, Right, 0x00
        decompressed.ReadInt();
        decompressed.ReadInt();
        decompressed.ReadInt();
        decompressed.ReadInt();
    }
    total += (int)record.Size;       // gap segments occupy file space too
}
```

## 3. SystemPageHeader 字段（ACadSharp `getPageHeaderData`，DwgReader.cs L649..L670）

| Offset | Size | Field | Type |
|---:|---:|---|---|
| 0x00 | 4 | `section_type` (PAGE_MAP=0x4163_0E3B / SECTION_MAP=0x4163_003B) | `u32` |
| 0x04 | 4 | `decompressed_size` | `u32` |
| 0x08 | 4 | `compressed_size` | `u32` |
| 0x0C | 4 | `compression_type` (== 0x02) | `u32` |
| 0x10 | 4 | `checksum` | `u32` |

ACadSharp 使用 `ReadRawLong` 但 spec 字段都是 4 字节，且语义上是
非负长度。我们用 `u32`（更符合 ODA spec），i32 在 ACadSharp 里只是
为了 `long` 兼容。

`compression_type != 0x02` 时返回 `UnsupportedCompressionType`：
ACadSharp 不显式校验，但 R46-C 是 read-only 接读 path，未来如果
遇到 compression_type=0x01（无压缩）等罕见 case 再扩展。

## 4. PageMapRecord 字段（ACadSharp DwgReader.cs L541..L565）

| Offset | Size | Field | Notes |
|---:|---:|---|---|
| 0x00 | 4 | `number` (i32 LE) | `>= 0` 是 valid record，`< 0` 是 gap |
| 0x04 | 4 | `size` (i32 LE) | 即使是 gap 也参与 seeker 累加 |
| (gap only) 0x08 | 4 | `parent` (i32 LE) | discarded |
| (gap only) 0x0C | 4 | `left` (i32 LE) | discarded |
| (gap only) 0x10 | 4 | `right` (i32 LE) | discarded |
| (gap only) 0x14 | 4 | `0x00` (i32 LE) | discarded |

`seeker` 是上层算出的累加 byte offset，从 `0x100` 起步，每条 record
（无论 valid 还是 gap）累加 `size`。这就是 R46-D 后续找 section
descriptor map 的入口（`fileheader.Records[SectionMapId].Seeker`）。

## 5. 验收

```bash
cargo test -p h7cad-native-dwg --lib page_map_ac1018
cargo test --locked --workspace --all-targets
RUSTFLAGS=-Dwarnings cargo check --locked --workspace --all-targets
```

通过标准：

- 单元测试 `parse_system_page_header_decodes_synthetic_header` pass：
  20 字节 fixture 解出 5 个字段。
- 单元测试 `parse_system_page_header_rejects_truncated_input` pass：
  19 字节 fixture 返回 `TruncatedInput`。
- 单元测试 `parse_ac1018_page_map_decodes_two_valid_records` pass：
  合成 page header + LZ77 leading-literal stream 解出 2 条 record，
  seeker 累加正确。
- 单元测试 `parse_ac1018_page_map_handles_negative_record_with_gap_padding`
  pass：合成 stream 含 1 valid + 1 negative + 1 valid 的 mix，
  验证 negative record 的 16 字节 padding 被正确 skip，下一条 valid
  record 的 seeker 包含 negative record 的 size。
- 单元测试 `parse_ac1018_page_map_rejects_invalid_section_type` pass：
  page_type != PAGE_MAP_SECTION_TYPE 时报 `InvalidPageType`。
- 单元测试 `parse_ac1018_page_map_rejects_unsupported_compression`
  pass：compression_type != 0x02 时报 `UnsupportedCompressionType`。
- 单元测试 `parse_ac1018_page_map_rejects_oversized_decompressed_size`
  pass：decompressed_size > sane cap (16 MiB) 时报
  `OversizedDecompressedSize`。
- 集成测试 `ac1018_page_map_decodes_real_sample` pass（sample 缺失
  soft-skip）：sample_AC1018.dwg 在 R46-A 实测的
  `page_map_address(eff)=0x10BC20` 处解出 records，且 records 中
  存在 `number == section_page_map_id (=58)` 与 `number ==
  section_map_id (=57)` 两条（因为 page map 自己也是一个 page，
  section descriptor map 同理）。
- 集成测试 `ac1018_page_map_decodes_real_sample_records_total_matches_sample`
  pass：records 总数应至少 ≥ 56（即 R46-A 实测的 section_amount）。
  实际 records 可能更多（含 gap），但 valid records 应该 ≥
  section_amount 的某个下界。
- workspace test 全 ok / 0 failed；
- `-Dwarnings cargo check workspace` 干净；
- 不修改 `DwgFileHeader::parse` / `read_dwg` 行为，无回归。

## 6. 任务

| T | 描述 | 状态 |
|---:|---|---|
| T1 | 落 R46-C 子 plan（本文件） | ✅ 完成 |
| T2 | 新增 `src/page_map_ac1018.rs`：types + parsers + 单元测试 | ✅ 完成（9 个单元测试 pass） |
| T3 | 在 `lib.rs` 接 `mod page_map_ac1018` 并 pub 关键 API | ✅ 完成 |
| T4 | 在 `tests/real_samples.rs` 加 1-2 个 AC1018 sample 集成测试 | ✅ 完成（2 个 sample test pass） |
| T5 | 双重门验收 | ✅ 完成 |

## 7. 不纳入

- AC1018 section descriptors map 解析（R46-D；R46-D 复用本砖
  `SystemPageHeader` + `parse_system_page_header` + `decompress_ac18_lz77`）；
- 修改 `DwgFileHeader::parse`（要等 R46-E）；
- 修改 `read_dwg` 顶层路径（要等 R46-E）；
- AC1018 写出（writer 路径）；
- system page checksum 验证（read path 不强制）；
- compression_type = 0x01（无压缩）的处理（real-world AC1018 几乎
  全 0x02）。

## 8. 风险

- **i32 size 累加溢出**：`total = 0x100 + Σ size` 用 i32 在大文件
  上可能溢出（i32::MAX = 2 GiB）。R46-C 使用 i64 容纳，比 ACadSharp
  原始实现略宽。
- **LZ77 解压尺寸预算**：`decompressed_size` 来自 page header，
  破损 input 可能给非常大的值导致 OOM。R46-C 加 16 MiB sane cap
  （`OversizedDecompressedSize`），实测 sample_AC1018.dwg 的 page
  map decompressed 远小于 1 MiB。
- **records 数量上限**：合法 AC1018 的 page map records 数量与
  section_amount + gap 数量正相关，sample 实测在百级。R46-C 不显式
  限制 records.len()，依靠 `decompressed_size` cap 间接限制。
- **section_type byte order**：ACadSharp 的 `ReadRawLong` 是
  little-endian 4 字节读取，常量 `0x41630E3B` 写到磁盘是
  `3B 0E 63 41`。Rust 端口用 `u32::from_le_bytes` 一致即可，单元
  测试用 `0x4163_0E3B` 字面量做 oracle。

## 9. 状态

- [x] T1 R46-C plan（本文件）
- [x] T2 page_map_ac1018.rs 实现 + 单元测试（9 个 pass：synthetic system page header round-trip、truncated header reject、two valid records、negative record + gap padding、invalid section_type reject、unsupported compression reject、oversized decompressed_size reject、truncated record stream reject、Display 字符串含诊断）
- [x] T3 lib.rs mod 接通 + pub（`parse_ac1018_page_map`、`parse_system_page_header`、`PageMap`、`PageMapDecodeError`、`PageMapRecord`、`SystemPageHeader`、6 个常量）
- [x] T4 real_samples.rs 集成测试（2 个 pass：`ac1018_page_map_decodes_real_sample` total=56 valid=56 self_seeker=0x10BC20 与 R46-A 一致；`ac1018_page_map_real_sample_records_total_at_least_section_amount` valid_records ≥ section_amount=56）
- [x] T5 双重门验收（workspace test 全 ok / 0 failed; h7cad-native-dwg lib 116→125; real_samples 29→31; RUSTFLAGS=-Dwarnings cargo check workspace 2.95s ok）

## 11. sample_AC1018.dwg page map 实测数据（R46-D 用）

```text
total_records=56, valid_records=56 (no gap pages on this sample)
page_map_self.seeker = 0x10BC20  (= R46-A 实测的 effective page_map_address，sanity check ✓)
section_descriptor_map.seeker = 0x10B880  (R46-D 入口：从这里读 SECTION_MAP system page)
```

## 10. R46-D 衔接

R46-D 用 R46-C 解出的 `PageMap` 找到 `lookup(section_map_id)
= page_map_records[57].seeker`，从该 seeker 处读 system page header（
section_type = `SECTION_MAP_SECTION_TYPE = 0x4163_003B`），LZ77
解压后解析 SectionDescriptor 数组（含 8 个核心 section：
AcDb:Handles / AcDb:AcDbObjects / AcDb:AppInfo / AcDb:Header / etc.）。
