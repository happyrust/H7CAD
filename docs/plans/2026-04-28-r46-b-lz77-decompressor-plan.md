# R46-B: AC1018 DWG-LZ77 decompressor (standalone module)

> 起稿：2026-04-28
> 前置：R46 主 plan `2026-04-28-r46-dwg-ac1018-bring-up-plan.md` 落盘，R46-A
> 已完成（`file_header_ac1018.rs` + sample fileId match）。
> R46-B 是 R46 系列的第二块砖：纯算法，零文件 header / section
> 依赖，可在不接通顶层 `read_dwg` 的情况下用合成 fixture 单元测试 + ODA
> spec round-trip 闭环验收。

## 1. 范围

新增独立 module `src/lz77_ac18.rs`，导出：

- `pub enum Lz77DecodeError { TruncatedInput, OffsetOutOfRange { … }, OutputOverflow { … } }`
  —— 不复用 `DwgReadError`，让 R46-B 保持纯算法、不依赖顶层 error 类型，
  R46-C / R46-D 调用方再做 `From` 转换。
- `pub fn decompress_ac18_lz77(compressed: &[u8], decompressed_size: usize) -> Result<Vec<u8>, Lz77DecodeError>`
  —— 把 ACadSharp `DwgLZ77AC18Decompressor.DecompressToDest` 端口为
  纯函数：吃压缩流 + 已知解压尺寸，吐解压字节。

`lib.rs` 仅 `mod lz77_ac18;` 接通并 `pub use` 关键 API；不动
`file_header.rs::section_count_offset`（仍对 AC1018 返回
`UnsupportedHeaderLayout`）—— R46-B 只新增独立 API，零回归。

## 2. 算法（参考 ACadSharp `DwgLZ77AC18Decompressor.cs`）

### 2.1 Top-level state machine

```text
opcode1 = src[0]; src cursor advances

if (opcode1 & 0xF0) == 0:
    # 起始的纯字面量（罕见但合法）
    opcode1 = copy(literalCount(opcode1) + 3)

while opcode1 != 0x11:        # 0x11 是终止 opcode
    compOffset = 0
    compressedBytes = 0

    if opcode1 < 0x10 || opcode1 >= 0x40:
        # 短 back-reference: 2-byte 编码
        compressedBytes = (opcode1 >> 4) - 1
        opcode2 = src[next];
        compOffset = ((opcode1 >> 2) & 3) | (opcode2 << 2)) + 1
    elif opcode1 < 0x20:
        # 长 back-reference 变体 1
        compressedBytes = readCompressedBytes(opcode1, mask=0b0111)
        compOffset = (opcode1 & 8) << 11
        opcode1 = twoByteOffset(&compOffset, addedValue=0x4000, src)
    elif opcode1 >= 0x20:
        # 长 back-reference 变体 2
        compressedBytes = readCompressedBytes(opcode1, mask=0b00011111)
        opcode1 = twoByteOffset(&compOffset, addedValue=1, src)

    # 复制 back-reference: 从 dst[pos - compOffset] 拿 compressedBytes 字节
    # 写到 dst 末尾。当 compressedBytes > compOffset 时，是 RLE 重复展开
    apply_back_reference(compOffset, compressedBytes)

    # back-reference 后跟随的字面量字节数（0..3 编码在低 2 位）
    litCount = opcode1 & 3
    if litCount == 0:
        opcode1 = src[next]
        if (opcode1 & 0xF0) == 0:
            litCount = literalCount(opcode1) + 3

    if litCount > 0:
        opcode1 = copy(litCount)
```

### 2.2 Helpers

- `literalCount(code)`：低 4 位作为字面量长度；如果为 0，则连读字节累加：
  `0x00` 加 0xFF 继续读；非 0 字节加 (lowbits + 0x0F + last_byte)，停止。
- `readCompressedBytes(opcode1, validBits)`：低 `validBits` 作为长度；
  如果为 0，则连读字节累加同上；最后总和 +2。
- `twoByteOffset(&offset, addedValue, src)`：读 2 字节扩展 offset：
  - `firstByte` = src[next]
  - `offset |= firstByte >> 2`
  - `offset |= src[next] << 6`
  - `offset += addedValue`
  - 返回 `firstByte`（作为下一轮的 opcode1）
- `copy(count)`：从 src 读 count 字节字面量直接 append 到 dst，再读 1 字节
  作为下一轮的 opcode1 返回。

### 2.3 Rust 端口要点

- 用 cursor pattern：`src_pos: usize` + `dst: Vec<u8>`，避免 `Stream`
  抽象（C# 的 `MemoryStream.Position` 在 Rust 用 `dst.len()` 或显式索引）。
- back-reference 拷贝在 `compressedBytes > compOffset` 情况下要逐字节
  写（C# `tempBuf` 是为了支持 RLE 展开），简洁实现：
  ```
  for i in 0..compressedBytes {
      let b = dst[dst.len() - compOffset];
      dst.push(b);
  }
  ```
  这种方式天然支持 RLE（写出的字节立刻进入回看窗口）。
- 越界检测：每次读 src 用 `src.get(pos).copied().ok_or(TruncatedInput)?`；
  back-reference 用 `dst.len() < compOffset` 判越界 `OffsetOutOfRange`；
  decompressed_size 防御：`dst.len() > decompressed_size` 时返回
  `OutputOverflow`（防 OOM）。

## 3. 验收

```bash
cargo test -p h7cad-native-dwg --lib lz77_ac18
cargo test --locked --workspace --all-targets
RUSTFLAGS=-Dwarnings cargo check --locked --workspace --all-targets
```

通过标准：

- 单元测试 `terminator_only_returns_empty` pass：单字节 0x11 → 解压为空。
- 单元测试 `pure_literal_block_round_trips` pass：`(0xF0 & 0x0F = 0)`
  路径，纯字面量模式，输入 `[0x0C, b'H', b'e', b'l', b'l', b'o', b'-', b'W', b'o', b'r', b'l', b'd', b'!', b'!', b'!', 0x11]`，
  解压为 `b"Hello-World!!!"` 的某个变体（具体 length 计算靠
  literalCount = 0x0C & 0x0F = 12; +3 = 15 字面量字节后跟终止）。
- 单元测试 `back_reference_basic` pass：构造 "ABABAB"（compOffset=2、
  compressedBytes=4 的 RLE 展开），验证 dst 正确生长。
- 单元测试 `back_reference_rle_self_overlap` pass：compOffset=1、
  compressedBytes=N 的全相同字节展开（aaaaaa…），验证不要从静态 buffer
  读旧值，必须从生长中的 dst 读。
- 单元测试 `literal_count_extended_chain` pass：0x00 链式累计（先一个
  0x00，再多个 0x00 各加 0xFF，最后非 0 字节终止）的字面量长度计算
  与 C# 行为一致。
- 单元测试 `truncated_input_returns_error` pass：opcode 期望第二字节但
  src 用尽时返回 `TruncatedInput`。
- 单元测试 `out_of_range_back_reference_returns_error` pass：
  compOffset > dst.len() 时返回 `OffsetOutOfRange`。
- 单元测试 `output_overflow_returns_error` pass：声明
  decompressed_size = 4 但实际生长超过时返回 `OutputOverflow`。
- workspace test 全 ok / 0 failed；
- `-Dwarnings cargo check workspace` 干净；
- 不修改 `DwgFileHeader::parse` 行为，不接通 read_dwg 顶层路径，无回归。

## 4. 任务

| T | 描述 | 状态 |
|---:|---|---|
| T1 | 落 R46-B 子 plan（本文件） | ✅ 完成 |
| T2 | 新增 `src/lz77_ac18.rs`：Lz77DecodeError + decompress_ac18_lz77 + 单元测试 | ✅ 完成（12 个单元测试 pass） |
| T3 | 在 `lib.rs` 接 `mod lz77_ac18` 并 pub 关键 API | ✅ 完成 |
| T4 | 双重门验收（cargo test workspace + RUSTFLAGS=-Dwarnings cargo check workspace） | ✅ 完成（h7cad-native-dwg lib 104→116; -Dwarnings cargo check 3.53s ok） |

## 5. 不纳入

- AC1018 page map 解析（R46-C）；
- AC1018 section descriptors map 解析（R46-D）；
- 修改 `DwgFileHeader::parse`（R46-D/E 之后）；
- 修改 `read_dwg` 顶层路径（R46-E）；
- AC1024 / AC1027 等更晚版本的 LZ77 变体（独立 R46 之外的事）。

## 6. 风险

- **算法分支正确性**：opcode1 的三段（`<0x10 || >=0x40`、`<0x20`、`>=0x20`）
  路径里隐藏 RLE / two-byte-offset 细节。R46-B 用合成 round-trip 测试覆盖
  每条分支至少一次。
- **整型截断**：C# 用 `int` / `byte` 混合，`(opcode1 >> 4) - 1` 在
  Rust 用 `i32` 中间计算后 `as usize`，避免 underflow（opcode1 < 0x10
  时 `(opcode1 >> 4) == 0`，但该分支被 `if opcode1 >= 0x10 || …` 同段
  保护，事实上不会到达 `compressedBytes = -1`）。Rust 端口必须验证
  这一点，必要时用 `i32::checked_sub`。
- **C# `byte.MaxValue` vs Rust `u8::MAX`**：`literalCount` 里 `lowbits +=
  byte.MaxValue` 是把 `int` 加 0xFF，不是字节加；Rust 用 `i32` 累加避免
  溢出。
- **`twoByteOffset` 的 `firstByte` 返回值**：C# `firstByte` 是 `int`
  类型从 `Stream.ReadByte`，可能是 `-1`（EOF）；Rust 端口要拒绝
  `TruncatedInput`，不能把 `-1` 当作合法 opcode 继续。

## 7. 状态

- [x] T1 R46-B plan（本文件）
- [x] T2 lz77_ac18.rs 实现 + 单元测试（12 个 pass：terminator-only、leading-literal preamble、short back-ref、RLE self-overlap、trailing literal via low-2-bits、trailing literal via extended chain、278-byte literal_count chain、truncated input reject、out-of-range back-ref reject、output overflow reject、negative compressed_bytes reject、Display strings 含诊断信息）
- [x] T3 lib.rs mod 接通 + pub（`decompress_ac18_lz77`、`Lz77DecodeError`）
- [x] T4 双重门验收（workspace test 全 ok / 0 failed; h7cad-native-dwg lib 104→116; RUSTFLAGS=-Dwarnings cargo check workspace 3.53s ok）

## 8. R46-C 衔接

R46-C 将用 `decompress_ac18_lz77` 解压 sample_AC1018.dwg 的 page map
压缩区域（位于 `R46-A` 实测的 `page_map_address(eff)=0x10BC20`），
解出 SectionPageMap records；R46-D 接续解 SectionDescriptorMap。
