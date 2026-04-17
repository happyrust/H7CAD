# DWG Native Port Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 把 `H7CAD` 的 `h7cad-native-dwg` 从占位 stub 演进为可用的 Rust 原生 DWG 读取器，先稳定产出 `h7cad-native-model::CadDocument`，再按桥接覆盖度决定何时替换桌面端的 `acadrust` DWG 打开链路。

**Architecture:** 参考 `ACadSharp` 的 DWG 主干流程，但不要 1:1 移植其完整 Template 类体系。应移植的是四层能力：`file header/version dispatch`、`versioned section/page decode`、`handle/object traversal`、`late resolve into native model`。`H7CAD` 已有直接面向 `h7cad-native-model` 的 DXF 解析器和较轻量的 `DocumentBuilder`，因此 DWG 侧优先实现 Rust 风格的 `pending graph + resolver`，而不是把 C# 的 50+ 模板类机械照搬。

**Tech Stack:** Rust 2021, `h7cad-native-model`, `h7cad-native-builder`, `h7cad-native-facade`, `ACadSharp` DWG reader 作为格式参考，crate 级 fixture 测试，桌面 smoke 验证。

---

## Current Findings

- `crates/h7cad-native-dwg/src/lib.rs` 目前只有 `read_dwg()` stub，尚未进入任何二进制解析阶段。
- `crates/h7cad-native-facade/src/lib.rs` 对 `NativeFormat::Dwg` 仍直接返回 `not implemented`。
- `src/io/mod.rs` 当前 `.dwg` 打开路径仍走 `acadrust::io::dwg::DwgReader`；只有 `.dxf` 已切到 native reader。
- `src/io/native_bridge.rs` 当前只桥接了 6 类实体：`Line / Circle / Arc / Point / Text / MText`。这意味着“native-dwg 能解析”不等于“主程序已经能无回归打开 DWG”。
- `crates/h7cad-native-model/src/lib.rs` 已具备 DWG 所需的大部分目标结构：`Handle`、`BlockRecord`、`Layout`、`RootDictionary`、`Entity.owner_handle`、`ObjectData`、`repair_ownership()`、`next_handle` 同步等。
- `crates/h7cad-native-builder/src/lib.rs` 适合处理保底的 block/layout 注册与 ownership 修复，但还不足以承接 ACadSharp 那种重模板构建流程。
- `ACadSharp` 的 DWG 主流程已验证为：`DwgReader -> versioned file header -> section buffer/decompress -> handles -> DwgObjectReader -> DwgDocumentBuilder`。
- `ACadSharp.Tests` 有 DWG reader 测试代码，但当前检出的仓库里没有现成 `.dwg` fixture 文件；这意味着 `H7CAD` 必须自己建立最小 DWG 样本基线。
- `ACadSharp` 的测试基线对 `AC1014 / AC1015 / AC1018 / AC1024 / AC1027 / AC1032` 更稳定；`AC1021` 不应作为 native 第一阶段的承诺范围。

## Recommended Direction

1. **先做 parser milestone，不立刻切主程序默认链路。**
   `h7cad-native-dwg` 先保证能把 DWG 解析成 `h7cad-native-model::CadDocument`，并通过 fixture summary 验证；是否接入 `src/io/mod.rs`，要取决于 `native_bridge` 的实体覆盖面。

2. **移植 ACadSharp 的“知识”，不要移植它的全部“形状”。**
   应复用 `ACadSharp` 的版本判断、section/page 解码、bit/handle 读取规则、对象遍历顺序、late binding 思路；但 Rust 侧建议用更小的 `PendingTableEntry / PendingEntity / PendingObject / Resolver` 结构替代大规模 Template 继承层次。

3. **版本策略分两波。**
   第一波只承诺 `AC1015` 与 `AC1018`；第二波再扩到 `AC1024 / AC1027 / AC1032`；`AC1021` 与 failsafe parse 放到后续里程碑。

4. **集成策略必须分离“解析完成”和“UI 可替换”。**
   由于 `native_bridge` 目前只覆盖 6 类实体，native-dwg 集成到桌面前要么先扩 bridge，要么保留 feature flag / fallback。

---

## Milestone Breakdown

| Milestone | Scope | Exit Gate |
|---|---|---|
| M3-A | `h7cad-native-dwg` 能读取 `AC1015/AC1018` 最小 DWG，构建 `CadDocument` | 头信息、表、block/layout、基础实体 fixture 全绿 |
| M3-B | 扩到 `AC1024/AC1027/AC1032` 页解码与对象读取 | R2010+/R2013+/R2018+ 样本可读 |
| M3-C | facade 接通，保留 guarded rollout | `NativeFormat::Dwg` 可返回 native doc |
| M3-D | 桥接扩容并做桌面 smoke | 主程序可用 native 路径打开典型 DWG |
| M3-E | failsafe parse 与长尾对象 | 损坏/部分异常文件可带诊断跳过 |

---

## Task 1: 建立 DWG Fixture 与 Summary 基线

**Files:**
- Create: `crates/h7cad-native-dwg/tests/fixtures/README.md`
- Create: `crates/h7cad-native-dwg/tests/fixture_manifest.rs`
- Create: `crates/h7cad-native-dwg/tests/read_headers.rs`
- Create: `crates/h7cad-native-dwg/tests/read_minimal_docs.rs`
- Create: `crates/h7cad-native-testkit/src/dwg_summary.rs`
- Modify: `crates/h7cad-native-testkit/src/lib.rs`

**Step 1: 列出第一批必须拥有的 DWG 样本**

样本至少包含：
- `minimal_ac1015.dwg`
- `minimal_ac1018.dwg`
- `minimal_ac1032.dwg`
- `blocks_insert.dwg`
- `layout_paperspace.dwg`
- `text_hatch_dim.dwg`

**Step 2: 写 `DwgSummary` 归一化摘要结构**

摘要只比较 parser 关键产物，不比较完整二进制：
- 版本
- handle seed
- layer/style/block/layout 名称集合
- model space / paper space entity 计数
- 关键实体签名（类型、layer、owner、基础几何）
- 关键对象签名（Dictionary/Layout/Group/XRecord 等）

**Step 3: 明确 fixture 来源**

优先级：
1. 由当前 `acadrust` 写出最小 DWG
2. 由外部 CAD/ODA 生成
3. 人工保留少量真实工程 DWG 做 smoke，不做严格 snapshot

**Step 4: 把“缺 fixture”显式暴露成测试失败或 ignored 测试**

避免 parser 开发阶段没有样本就继续堆实现。

**Step 5: 运行最小基线测试**

Run: `cargo test -p h7cad-native-dwg read_headers -- --nocapture`  
Expected: 至少 fixture manifest 能加载；缺样本时失败信息明确指向缺失文件。

---

## Task 2: 实现 DWG 入口、版本识别与文件头层

**Files:**
- Modify: `crates/h7cad-native-dwg/src/lib.rs`
- Create: `crates/h7cad-native-dwg/src/error.rs`
- Create: `crates/h7cad-native-dwg/src/version.rs`
- Create: `crates/h7cad-native-dwg/src/file_header.rs`
- Create: `crates/h7cad-native-dwg/src/section_map.rs`

**Step 1: 定义 `DwgReadError`**

错误类型至少覆盖：
- unsupported version
- invalid magic/version string
- truncated file header
- page decode failure
- object decode failure
- unresolved handle / owner / layout

**Step 2: 从 `lib.rs` 中拆出 `sniff_version(bytes)`**

读取前 6 字节 `ACXXXX` 并映射到内部版本枚举。

**Step 3: 先移植 AC15/AC18 文件头**

从 `ACadSharp.IO.DWG.DwgReader` 中提取以下逻辑：
- `readFileHeaderAC15`
- `readFileHeaderAC18`
- `readFileMetaData`

目标不是一次读完整 DWG，而是先能得到：
- code page
- section locator / descriptor
- page map / section map 元数据

**Step 4: 预留 AC21/AC24 结构，不立即做全量支持**

先把 AC21/AC24 所需的数据结构与 dispatch 留出来，真正实现放在后续任务。

**Step 5: 写头部层单测**

Run: `cargo test -p h7cad-native-dwg read_header_`  
Expected: `AC1015`/`AC1018` fixture 能正确识别版本与 section 描述。

---

## Task 3: 实现 bit reader、handle reader 与 section/page 解码

**Files:**
- Create: `crates/h7cad-native-dwg/src/reader/mod.rs`
- Create: `crates/h7cad-native-dwg/src/reader/bit_reader.rs`
- Create: `crates/h7cad-native-dwg/src/reader/ac15.rs`
- Create: `crates/h7cad-native-dwg/src/reader/ac18.rs`
- Create: `crates/h7cad-native-dwg/src/reader/ac21.rs`
- Create: `crates/h7cad-native-dwg/src/reader/ac24.rs`
- Create: `crates/h7cad-native-dwg/src/lz77.rs`
- Create: `crates/h7cad-native-dwg/src/reed_solomon.rs`

**Step 1: 先移植 `DwgStreamReaderBase` 的核心 bit 语义**

必须先稳定这些原语：
- `read_bit`
- `read_2bits`
- `read_bit_short`
- `read_bit_long`
- `read_bit_double`
- `read_modular_char`
- `read_signed_modular_char`
- `handle_reference(reference_handle)`

**Step 2: 为 Rust API 设计统一 reader trait**

建议暴露：
- `position_bits()`
- `set_position_bits()`
- `read_section_bytes()`
- `read_handle_ref()`
- `read_text()`

**Step 3: 实现 AC15 直接 section stream**

AC15 先不做复杂分页，先把 section locator → raw section stream 跑通。

**Step 4: 实现 AC18 页装配与 LZ77 解压**

对齐 `ACadSharp` 的 `getSectionBuffer18()` 与 `decryptDataSection()` 逻辑。

**Step 5: AC21/AC24 只在 AC15/AC18 绿灯后推进**

避免多个版本同时 debug，导致定位困难。

**Step 6: 用人工 buffer 写 reader 单元测试**

Run: `cargo test -p h7cad-native-dwg reader_`  
Expected: bit/handle 语义测试全部通过，再进入对象层。

---

## Task 4: 设计 Rust 风格的 Pending Graph 与 Resolver

**Files:**
- Create: `crates/h7cad-native-dwg/src/pending.rs`
- Create: `crates/h7cad-native-dwg/src/resolver.rs`
- Modify: `crates/h7cad-native-dwg/src/lib.rs`

**Step 1: 定义最小 pending 层**

至少定义：
- `PendingHeader`
- `PendingTableEntry`
- `PendingBlockRecord`
- `PendingLayout`
- `PendingEntity`
- `PendingObject`

这些结构保留 raw handle 引用，而不是立即要求拿到名字或最终 owner。

**Step 2: 建立 `HandleMap` 与 late-resolve 流程**

解析阶段只做：
- 按 handle 收集对象
- 保存 owner/layer/linetype/block/layout 等引用

resolve 阶段再做：
- handle -> table name
- handle -> block record
- handle -> layout
- handle -> root dictionary entry

**Step 3: 直接落到 `h7cad-native-model::CadDocument`**

不要先造出 ACadSharp 等价 Template 类树，再二次映射。  
目标是：
- 填充 `doc.layers / linetypes / text_styles / dim_styles / vports`
- 填充 `doc.block_records / layouts / root_dictionary`
- 填充 `doc.entities / objects`
- 最后调用 `repair_ownership()` 与 `set_next_handle()`

**Step 4: 复用 `h7cad-native-builder` 仅做保底修复**

只在默认 model/paper space 或缺 layout/block 时使用 `DocumentBuilder` 的模板注册能力，不让它变成主解析流程的中心。

**Step 5: 写 resolver 测试**

Run: `cargo test -p h7cad-native-dwg resolver_`  
Expected: 给定 pending graph 后能稳定产出包含 block/layout/root dictionary 的 `CadDocument`。

---

## Task 5: 先移植 MVP 对象/实体读取器

**Files:**
- Create: `crates/h7cad-native-dwg/src/object_reader/mod.rs`
- Create: `crates/h7cad-native-dwg/src/object_reader/common.rs`
- Create: `crates/h7cad-native-dwg/src/object_reader/tables.rs`
- Create: `crates/h7cad-native-dwg/src/object_reader/entities_basic.rs`
- Create: `crates/h7cad-native-dwg/src/object_reader/entities_complex.rs`
- Create: `crates/h7cad-native-dwg/src/object_reader/non_graphical.rs`
- Modify: `crates/h7cad-native-dwg/src/lib.rs`

**Step 1: 先移植公共读取逻辑**

对齐 `ACadSharp` 中以下概念：
- common entity data
- common non-entity data
- owner handle
- layer handle / linetype handle
- entity mode (model/paper)
- reactors / dictionary handle

**Step 2: MVP 表/对象优先级**

先做这些，能支撑绝大多数最小文档：
- `LAYER`
- `LTYPE`
- `STYLE`
- `DIMSTYLE`
- `VPORT`
- `BLOCK_RECORD`
- `LAYOUT`
- `DICTIONARY`
- `XRECORD`
- `GROUP`

**Step 3: MVP 实体优先级**

第一波只做：
- `LINE`
- `CIRCLE`
- `ARC`
- `POINT`
- `TEXT`
- `MTEXT`
- `LWPOLYLINE`
- `INSERT`
- `HATCH`
- `DIMENSION`
- `VIEWPORT`

**Step 4: 长尾对象先落 `Unknown`**

在 `native-model::ObjectData::Unknown` 已存在的前提下，
对下面这些高复杂对象先做“识别但不细解”：
- `MLEADERSTYLE`
- `MLINESTYLE`
- `TABLESTYLE`
- `SORTENTSTABLE`
- `DIMASSOC`
- AEC / Proxy 类对象

**Step 5: 明确 defer 列表**

以下内容不进入 native 第一阶段：
- `ACIS / 3DSOLID / REGION / BODY` 深解析
- `AC1021` 全版本保证
- `failsafe parse`
- preview / summary info 的完整对外 API

**Step 6: 运行 fixture 读取测试**

Run: `cargo test -p h7cad-native-dwg read_minimal_docs -- --nocapture`  
Expected: `AC1015`/`AC1018` 最小 fixture 能构建非空 `CadDocument`，block/layout/owner 关系正确。

---

## Task 6: 做解析正确性验证，而不是只验证“不 panic”

**Files:**
- Create: `crates/h7cad-native-dwg/tests/read_blocks.rs`
- Create: `crates/h7cad-native-dwg/tests/read_layouts.rs`
- Create: `crates/h7cad-native-dwg/tests/read_entities.rs`
- Create: `crates/h7cad-native-dwg/tests/read_objects.rs`
- Modify: `crates/h7cad-native-dwg/src/error.rs`

**Step 1: 每类 fixture 都做 summary 断言**

不要只断言 `read_dwg().is_ok()`，至少要比较：
- 实体数量
- block/layout 数量
- model/paper space 分类
- owner handle 是否可回溯到 block record
- root dictionary 是否补齐 layout 条目

**Step 2: 建立 `strict` 与 `diagnostic` 两种模式**

- `strict`: 任何未知关键对象直接报错
- `diagnostic`: 保留错误上下文并尽可能继续

第一阶段默认用 `strict` 开发，避免 silent corruption。

**Step 3: 为每个错误带上足够定位信息**

至少包含：
- DWG version
- object type / handle
- section name
- bit/byte offset

**Step 4: 跑 crate 级全量测试**

Run: `cargo test -p h7cad-native-dwg`  
Expected: header/reader/resolver/fixture 测试全部通过。

---

## Task 7: Facade 接通与受控集成

**Files:**
- Modify: `crates/h7cad-native-facade/src/lib.rs`
- Modify: `src/io/mod.rs`
- Modify: `src/io/native_bridge.rs`

**Step 1: 先接通 `NativeFormat::Dwg`**

让 facade 能返回 native `CadDocument`，但不要立刻替换桌面 `.dwg` 默认路径。

**Step 2: 增加 guarded rollout**

推荐用 feature flag 或环境变量控制：
- `H7CAD_NATIVE_DWG=0` 继续走 `acadrust`
- `H7CAD_NATIVE_DWG=1` 走 native parser

**Step 3: 先扩 `native_bridge` 到 MVP 实体集**

至少补齐：
- `LwPolyline`
- `Insert`
- `Hatch`
- `Dimension`
- `Viewport`

否则桌面端切换后会出现“parser 解析出来了，但 UI 只显示 6 类实体”的假成功。

**Step 4: 做桌面 smoke**

Run: `cargo run`  
Manual check:
- 打开 `minimal_ac1015.dwg`
- 打开 `blocks_insert.dwg`
- 打开 `layout_paperspace.dwg`
- 验证打开后图元数量、图层、布局切换、基础渲染无明显回退

**Step 5: 决定默认切换时机**

只有在以下条件全部满足后，才考虑把 `.dwg` 默认打开链路从 `acadrust` 切到 native：
- crate 测试稳定
- MVP fixture 全绿
- bridge 覆盖面足够
- 桌面 smoke 连续通过

---

## Risks And Mitigations

**Risk 1: ACadSharp 的模板体系过重，Rust 原样照搬会拖慢开发。**

Mitigation: 只移植格式知识与遍历顺序；对象关系在 Rust 侧用 `pending + resolver` 轻量实现。

**Risk 2: 没有 DWG fixture，开发会退化成“读一份文件碰运气”。**

Mitigation: 把 fixture 建设放到 Task 1，先于 parser 主体。

**Risk 3: native parser 完成后，桌面端仍因 bridge 覆盖不足而不可替换。**

Mitigation: 把 parser milestone 与 integration milestone 明确拆开；默认保留 fallback。

**Risk 4: 多版本页解码同时推进，debug 成本爆炸。**

Mitigation: 严格按 `AC1015 -> AC1018 -> AC1024/1027/1032` 顺序推进。

**Risk 5: AC1021 支持不稳定，影响里程碑预期。**

Mitigation: 第一阶段文档中显式不承诺 AC1021，把它列为单独后续任务。

---

## Definition Of Done

- `h7cad-native-dwg::read_dwg()` 不再返回 stub 错误。
- `AC1015` 与 `AC1018` 最小 DWG fixture 能稳定产出 `CadDocument`。
- `CadDocument` 中的 table/block/layout/root dictionary/owner 关系可通过 summary 验证。
- `NativeFormat::Dwg` 已接通 facade。
- 是否切主程序默认链路，有明确 feature flag 与 smoke 结论，而不是隐式替换。

---

## Suggested Execution Order

1. Task 1
2. Task 2
3. Task 3
4. Task 4
5. Task 5
6. Task 6
7. Task 7
