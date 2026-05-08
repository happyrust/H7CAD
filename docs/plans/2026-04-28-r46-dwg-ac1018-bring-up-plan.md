# R46-DWG-AC1018: bring up native AC1018 reader to entity recovery

> 起稿：2026-04-28
> 前置：R47–R51 已收尾，AC1015 native DWG 端到端 read_dwg → entity recovery 完整可用。
> R46 是**多砖头工程**，拆成 R46-A → R46-F 六个独立可验收的 brick。本文件是
> R46 主 plan；每个 brick 单独维护 `2026-04-28-r46-{a|b|c|d|e|f}-*.md` 子计划。

## 1. 现状（事实摸清）

- 当前 `crates/h7cad-native-dwg/src/file_header.rs::section_count_offset` 对
  `DwgVersion::Ac1018` 直接返回
  `Err(DwgReadError::UnsupportedHeaderLayout { version })`（line 66）；
- AC1018 sample `d:/work/plant-code/cad/ACadSharp/samples/sample_AC1018.dwg`
  存在（约 1.05 MB），可作端到端验收 fixture；
- ACadSharp 的 C# 实现位于
  `src/ACadSharp/IO/DWG/{FileHeaders/DwgFileHeaderAC18.cs, DwgReader.cs::readFileHeaderAC18}`，
  作为 reference 实现；
- AC1018 与 AC1015 的核心差异是 **file header 层**（加密 0x6C 块 + LZ77
  压缩的 page map + section descriptor map）；entity body 解码层（
  `entity_line.rs / entity_circle.rs / …`）大概率可复用，因为 ODA
  spec 上 entity layout 在 AC1015 → AC1018 之间无破坏性变化。

## 2. AC1018 file header 结构（参考 ACadSharp + ODA spec）

```text
0x00  6     "AC1018" magic
0x06  ?     metadata header (类似 AC1015 前导)
0x80  0x6C  ENCRYPTED metadata block:
            * XOR 解密 with magic byte sequence
            * 0x00 12  fileId "AcFssFcAJMB\0"
            * 0x18  4  RootTreeNodeGap
            * 0x1C  4  LeftGap
            * 0x20  4  RightGap
            * 0x28  4  LastPageId
            * 0x2C  8  LastSectionAddr
            * 0x34  8  SecondHeaderAddr
            * 0x3C  4  GapAmount
            * 0x40  4  SectionAmount
            * 0x50  4  SectionPageMapId
            * 0x54  8  PageMapAddress (+ 0x100)
            * 0x5C  4  SectionMapId
            * 0x60  4  SectionArrayPageSize
            * 0x64  4  GapArraySize
            * 0x68  4  CRC32 seed
0x100+      Section pages (LZ77 compressed):
            * Page map at fileheader.PageMapAddress
            * Section descriptors map at fileheader.Records[SectionMapId].Seeker
```

Magic XOR sequence is generated programmatically by ACadSharp
(`DwgCheckSumCalculator::MagicSequence`, LCG with seed 1, multiplier
`0x343FD`, increment `0x269EC3`, take high 16 bits per byte → 256 bytes).
First 16 bytes match the ODA-spec magic `29 23 BE 84 E1 6C D6 AE 52 90
49 F1 F1 BB E9 EB`. Only the first 0x6C = 108 bytes are needed for the
encrypted-metadata block.

## 3. 砖头拆解（R46-A → R46-F）

| Brick | Scope | 阻塞? | 估计 | sample 验证 |
|---|---|---|---:|---|
| R46-A | Magic byte sequence 常量 + `parse_ac1018_encrypted_metadata` + sample test | 无 | 1 h | sample_AC1018.dwg fileId == "AcFssFcAJMB\0" |
| R46-B | DWG-LZ77 AC18 解压缩器（独立 module，纯算法） | 无 | 2 h | 单元测试 + ODA spec 例子 |
| R46-C | AC1018 page map 解析（用 R46-B） | R46-A,B | 1.5 h | sample 上 SectionPageMap 解出 N 条 record |
| R46-D | AC1018 section descriptors map 解析（用 R46-B） | R46-C | 1.5 h | sample 上 NumDescriptions ≥ 8 个核心 section |
| R46-E1 | 单 section payload 重组（XOR 解密 page header + LZ77 解压 + 多 page 拼装） | R46-D | 1.5 h | sample 上 AcDb:Header payload 解出且 sentinel match |
| R46-E2 | 接入 build_pending_document，复用 AC1015 entity decoding | R46-E1 | 2 h | sample 上 read_dwg 返回 Ok 且 entity_count > 0 |
| R46-F | AC1018 baseline_m3b 测试接通，ratchet baseline lower bound | R46-E2 | 0.5 h | baseline_m3b 不再 skip AC1018 |

总估计 10 h（R46-E 拆分后比原 8.5 h 多 1.5 h，反映"单 page 重组"
独立成砖的实际工作量）。每个 brick 是独立 commit / 可独立 PR，跨
多个对话轮次推进。

## 4. 本次会话目标

- ✅ R46 主 plan 文件落盘（本文件）
- ⏳ 执行 R46-A（最稳的入口砖：纯 byte-level 解码，零 LZ77 依赖，有 sample
  fileId 字符串作为 oracle）

R46-B/C/D/E/F 留作后续会话推进，每砖独立 plan + TDD red→green + 双重门验收。

## 5. 不在 R46 scope

- AC1019、AC1021、AC1024、AC1027、AC1032 仍 fail-closed（`UnsupportedVersion`）；
- AC1018 写出（`writer` 路径，对应 ACadSharp `DwgFileHeaderWriterAC18`）不做；
- AC1018 的 SummaryInfo / VbaProject / AppInfo / Preview 等可选 section 不做；
- AC1018 加密 section（`Encrypted=1`）的 password 解密不做（real-world AC1018
  几乎都是 `Encrypted=0`）；
- AC1018 的 SecondHeader 校验不做（read 路径不需要）。

## 6. 进入条件 / 退出条件

| 入口 | 退出 |
|---|---|
| R46-A: 无（独立 module） | sample_AC1018.dwg 解密后 fileId match |
| R46-B: 无（纯算法） | DWG-LZ77 单元测试 pass + ODA spec 例子 pass |
| R46-C: A,B 完 | sample SectionPageMap 解出 records (records.len ≥ 1 且 PageMapAddress 命中) |
| R46-D: C 完 | sample SectionDescriptors map 解出 N ≥ 8 个 descriptors（包含 AcDb:Handles / AcDb:AcDbObjects 等核心 section） |
| R46-E: D 完 | `read_dwg(sample_AC1018_bytes)` 返回 `Ok(doc)` 且 `doc.entities.len() > 0` |
| R46-F: E 完 | baseline_m3b 在 AC1018 路径下不再 skip / explicit unsupported；ratchet baseline 反映 sample_AC1018 实测 entity 数 |

## 7. 风险

- **LZ77 算法分歧**：DWG-LZ77 是 ODA 自定义变体，与 standard LZ77 不一致。
  R46-B 必须严格按 ACadSharp `DwgLZ77AC18Decompressor` 端口，不能用通用 lz4
  / lzma 库。
- **Magic sequence 端口差异**：Rust LCG 实现要严格匹配 C# 行为
  （`(byte)(randSeed >> 0x10)` 在 C# 是 `(byte)(int >> 16)`，要注意 signed
  vs unsigned 截断）。R46-A 单元测试要 hardcode 前 16 字节 oracle
  （`29 23 BE 84 E1 6C D6 AE 52 90 49 F1 F1 BB E9 EB`）来 catch 实现 bug。
- **0x6C 块字段顺序**：ODA spec 与 ACadSharp 注释一致，但 spec 多年来有
  小版本差异，必须以 sample_AC1018.dwg 真实数据为 ground truth。
- **CRC32 校验 unwrapper**：CRC seed 计算可以先跳过（read path 仅用，不影响
  正确性），如果未来要写 AC1018 再补。

## 8. 状态

- [x] R46 主 plan（本文件）
- [x] R46-A: encrypted metadata block parse（详见 `2026-04-28-r46-a-encrypted-metadata-plan.md`）
- [x] R46-B: LZ77 解压缩器（详见 `2026-04-28-r46-b-lz77-decompressor-plan.md`）
- [x] R46-C: page map 解析（详见 `2026-04-28-r46-c-page-map-plan.md`；sample 实测 total/valid_records=56，page_map_self.seeker=0x10BC20，section_descriptor_map.seeker=0x10B880）
- [x] R46-D: section descriptors map 解析（详见 `2026-04-29-r46-d-section-descriptors-plan.md`；sample 实测 descriptor_count=13，7 个 ODA 核心 section 全部命中）
- [x] R46-E1: 单 section payload 重组（详见 `2026-04-29-r46-e1-section-data-plan.md`；sample AcDb:Header 解出 29696 bytes 且前 16 字节匹配 ODA start sentinel）
- [ ] R46-E2: build_pending_document 接通 + entity recovery（依赖 R46-E1，需把 13 个 AC1018 descriptors 桥接为 AC1015 风格的 SectionMap + Vec<Vec<u8>>）
- [ ] R46-F: baseline_m3b AC1018 接通（依赖 R46-E2）

> **R46-E 拆分**：原计划 R46-E 是单一砖头"build_pending_document
> 接通"，但实测发现需要先单独解决"单 page XOR 解密 + LZ77 解压"
> 这一前置工序，遂拆为 R46-E1（数据层）+ R46-E2（接通层）。

## 9. 子计划索引

| Brick | 子 plan |
|---|---|
| R46-A | `docs/plans/2026-04-28-r46-a-encrypted-metadata-plan.md` |
| R46-B | `docs/plans/2026-04-28-r46-b-lz77-decompressor-plan.md` |
| R46-C | `docs/plans/2026-04-28-r46-c-page-map-plan.md` |
| R46-D | `docs/plans/2026-04-29-r46-d-section-descriptors-plan.md` |
| R46-E1 | `docs/plans/2026-04-29-r46-e1-section-data-plan.md` |
| R46-E2,F | 待落盘（R46-E2 用 R46-E1 的 `read_ac1018_section_payload` 给每个 R46-D descriptor 生成 payload，按 name → record_number 映射合成 AC1015 风格 SectionMap，调用 build_pending_document → resolve_document → enrich_with_real_entities） |
