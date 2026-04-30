# R46-A: AC1018 encrypted metadata block parser

> 起稿：2026-04-28
> 前置：R46 主 plan `2026-04-28-r46-dwg-ac1018-bring-up-plan.md` 落盘。
> R46-A 是 R46 系列的入口砖：纯 byte-level 解码，零 LZ77 依赖，
> 用 sample_AC1018.dwg 的 `AcFssFcAJMB\0` fileId 字符串作 oracle，
> 一次 commit 闭环。

## 1. 范围

新增独立 module `src/file_header_ac1018.rs`，导出：

- `pub(crate) const AC1018_MAGIC_SEQUENCE: [u8; 256]` —— ACadSharp
  `DwgCheckSumCalculator::MagicSequence` 的 Rust 端口（LCG, seed=1,
  multiplier=0x343FD, increment=0x269EC3, take high 16 bits per byte）；
  实现为 const fn 计算，避免 hardcode 全表。
- `pub struct Ac1018EncryptedMetadata { fields… }` —— 解密后的 0x6C 块。
- `pub fn parse_ac1018_encrypted_metadata(bytes: &[u8]) -> Result<Ac1018EncryptedMetadata, DwgReadError>`
  —— 从 raw file bytes 读 0x80..0x80+0x6C，XOR 解密，验证 fileId，解析字段。

`file_header.rs::section_count_offset` 在 R46-A **保持原样**（仍对 AC1018
返回 `UnsupportedHeaderLayout`）—— R46-A 只新增独立 API，不改变
`DwgFileHeader::parse` 行为，避免引入回归。

## 2. 字段（按 ACadSharp `readFileHeaderAC18` 顺序）

| Offset | Size | 字段 | 类型 |
|---:|---:|---|---|
| 0x00 | 12 | `file_id` ("AcFssFcAJMB\0") | `[u8; 12]` |
| 0x0C | 4 | unknown_0c (= 0) | `i32` |
| 0x10 | 4 | unknown_10 (= 0x6C) | `i32` |
| 0x14 | 4 | unknown_14 (= 0x04) | `i32` |
| 0x18 | 4 | `root_tree_node_gap` | `i32` |
| 0x1C | 4 | `left_gap` | `i32` |
| 0x20 | 4 | `right_gap` | `i32` |
| 0x24 | 4 | unknown_24 (ODA writes 1) | `i32` |
| 0x28 | 4 | `last_page_id` | `i32` |
| 0x2C | 8 | `last_section_addr` | `u64` |
| 0x34 | 8 | `second_header_addr` | `u64` |
| 0x3C | 4 | `gap_amount` | `u32` |
| 0x40 | 4 | `section_amount` | `u32` |
| 0x44 | 4 | unknown_44 (= 0x20) | `i32` |
| 0x48 | 4 | unknown_48 (= 0x80) | `i32` |
| 0x4C | 4 | unknown_4c (= 0x40) | `i32` |
| 0x50 | 4 | `section_page_map_id` | `u32` |
| 0x54 | 8 | `page_map_address` (raw + 0x100) | `u64` |
| 0x5C | 4 | `section_map_id` | `u32` |
| 0x60 | 4 | `section_array_page_size` | `u32` |
| 0x64 | 4 | `gap_array_size` | `u32` |
| 0x68 | 4 | `crc_seed` | `u32` |

`page_map_address` 字段语义：raw u64 + 0x100 才是实际文件偏移（ACadSharp
`DwgReader.cs:514`）。本结构存 raw 值，让上层加 0x100。

## 3. 验收

```bash
cargo test -p h7cad-native-dwg --lib file_header_ac1018
cargo test --locked --workspace --all-targets
RUSTFLAGS=-Dwarnings cargo check --locked --workspace --all-targets
```

通过标准：

- 单元测试 `magic_sequence_first_16_bytes_match_oda_spec` pass：
  前 16 字节是 `29 23 BE 84 E1 6C D6 AE 52 90 49 F1 F1 BB E9 EB`；
- 单元测试 `magic_sequence_full_table_is_deterministic` pass：256 字节
  hash / lengths sanity；
- 集成测试 `ac1018_encrypted_metadata_decodes_real_sample_file_id` pass（sample
  缺失 soft-skip）：sample_AC1018.dwg 解密后 fileId == "AcFssFcAJMB\0"；
- 集成测试 `ac1018_encrypted_metadata_decodes_real_sample_addresses_are_sane`
  pass：page_map_address > 0、section_map_id > 0、section_amount > 0；
- workspace test 全 ok / 0 failed；
- `-Dwarnings cargo check workspace` 干净；
- 不修改 `DwgFileHeader::parse` 行为，无回归。

## 4. 任务

| T | 描述 | 状态 |
|---:|---|---|
| T1 | 落 R46-A 子 plan（本文件） | ✅ 完成 |
| T2 | 新增 `src/file_header_ac1018.rs`：const MAGIC_SEQUENCE + parse 函数 + struct + 单元测试 | ✅ 完成（5 个单元测试 pass） |
| T3 | 在 `lib.rs` 接 `mod file_header_ac1018` 并 pub 关键 API | ✅ 完成 |
| T4 | 在 `tests/real_samples.rs` 加 2 个 AC1018 sample 集成测试（soft-skip） | ✅ 完成（2 个 sample test pass） |
| T5 | 双重门验收 | ✅ 完成 |

## 5. 不纳入

- LZ77 解压缩器（R46-B）；
- page map 解析（R46-C）；
- section descriptors map 解析（R46-D）；
- 修改 `DwgFileHeader::parse`（要等 R46-D/E 之后）；
- 修改 `read_dwg` 顶层路径（要等 R46-E）；
- AC1018 写出。

## 6. 风险

- **LCG 实现差异**：C# `(byte)(int >> 16)` 与 Rust 截断行为差异——C#
  对 `int` 是 signed 32-bit，`>> 16` 是 arithmetic shift，但 `(byte)`
  会取低 8 位；Rust 用 `i32` 类型一致即可，最终 `as u8` 截断。已用
  `magic_sequence_first_16_bytes_match_oda_spec` oracle 测试覆盖。
- **fileId 字符串对齐**：ACadSharp 注释 `"AcFssFcAJMB\0"` 是 12 bytes
  含 null terminator，注意不要 trim。
- **`#[allow(dead_code)]` 规避**：`-Dwarnings` 下未使用字段会 fail；本砖
  字段全部 `pub` 且测试访问其中关键字段（page_map_address、
  section_map_id、section_amount、file_id），其余字段在 R46-C/D 才用到，
  暂用 `#[allow(dead_code)]` 标记，R46-C 落地时去掉。

## 7. 状态

- [x] T1 R46-A plan
- [x] T2 file_header_ac1018.rs 实现 + 单元测试（5 个 pass：oda 16-byte oracle、full table sanity、truncated reject、wrong file_id reject、synthetic roundtrip）
- [x] T3 lib.rs mod 接通 + pub（parse_ac1018_encrypted_metadata、Ac1018EncryptedMetadata、AC1018_ENCRYPTED_BLOCK_LEN、AC1018_ENCRYPTED_BLOCK_OFFSET、AC1018_FILE_ID）
- [x] T4 real_samples.rs 集成测试（fileId match + addresses sane）
- [x] T5 双重门验收（workspace test 46 binary 0 failed; lib.rs 99→104; real_samples 27→29; -Dwarnings cargo check 2.67s ok）

## 8. sample_AC1018.dwg 实测数据（R46-C/D 用）

```text
section_amount=56
section_page_map_id=58
page_map_address_raw=0x10BB20
page_map_address(eff)=0x10BC20  (sample 文件大小 0x10BDFE，effective 在文件内)
section_map_id=57
section_array_page_size=58
```
