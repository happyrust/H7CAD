# H7CAD DWG 读写支持下一步开发计划

> **起稿**：2026-05-08
> **基线盘点**：见本文 §1。当前 DWG **读路径已基本闭环**（AC1015 强基线、AC1018 per-family 基线、其他版本 fail-closed），**产品写路径仍依赖 acadrust**，但 native writer 已完成 AC1015 空文档与 LINE/CIRCLE/ARC/POINT/LWPOLYLINE/TEXT tracer bullets。
> **目标**：把 DWG 读写从「能用但偏」推进到「读全谱 + 写有 R2000 起点」，并把对外承诺与代码事实对齐。
> **范围**：本计划覆盖 `crates/h7cad-native-dwg`、`crates/h7cad-native-facade`、`src/io/mod.rs::save_dwg/load_dwg_native_blocking`、`README.md` 的 DWG 状态描述，以及配套测试与 fixture。
> **不在范围**：DXF 写入器（已基本闭环）、PID 流水线、UI/CLI 主体功能、acadrust 移除（依赖本计划 F5 完成才能正式启动）。

---

## 1. 现状盘点（2026-05-08）

### 1.1 读取（Read）

| 维度 | 状态 |
|---|---|
| 集成层 | `src/io/load_dwg_native_blocking` 已实现「先 native 后 acadrust fallback」+ `OpenNotice::Warning` 提示，facade 也已直连 `read_dwg`。 |
| AC1012 / AC1014 | sniff OK，native reader 仍 fail-closed（当前 real-sample baseline 走 explicit `UnsupportedVersion`）。 |
| **AC1015 (R2000)** | **完整流水线**：DwgFileHeader → SectionMap → handle_offsets → build_pending_document → resolve_document → enrich_with_real_entities。 |
| **AC1018 (R2004)** | **完整流水线**（R46-A/C/D/E1/E2 已落地）：encrypted_metadata → page_map → section_descriptor_map → LZ77 section_payload → 合成 AC1015 风格 SectionMap → 复用下游。 |
| AC1021 / AC1024 / AC1027 / AC1032 | sniff OK，native reader 仍 fail-closed（当前 real-sample baseline 走 explicit `UnsupportedVersion`）。 |
| 实体覆盖 | ~22 类已分发：TEXT / ATTRIB / ATTDEF / INSERT / ARC / CIRCLE / LINE / DIMENSION（7 子类）/ POINT / FACE3D / SOLID / VIEWPORT / ELLIPSE / SPLINE / RAY / XLINE / MTEXT / LWPOLYLINE / HATCH。 |
| 诊断 | `Ac1015RecoveryDiagnostics`（family × failure_kind 双维度桶）+ `trace_ac1015_targeted_failure_before_fallback` + `collect_ac1015_preheader_object_type_hints` 已上线。 |

**实测基线**（`tests/real_samples.rs::real_dwg_samples_baseline_m3b`）：

- `sample_AC1015.dwg`：当前实测 238 entities（82 LINE / 9 CIRCLE / 3 ARC / 34 POINT / **=26 TEXT** / ≥17 LWPOLYLINE / **=6 HATCH**，另含 ELLIPSE / SPLINE / MTEXT / INSERT / DIMENSION / VIEWPORT）。
- `sample_AC1018.dwg`：当前实测 11 entities（1 CIRCLE / 2 HATCH / 2 INSERT / 6 VIEWPORT），2 blocks，2 layouts，281 objects；R46-F per-family ratchet 已落地。

### 1.2 写入（Write）

| 维度 | 状态 |
|---|---|
| facade `save(Dwg, …)` | **显式拒绝**：`Err("native DWG writer not implemented yet")`，并被测试 `dwg_runtime_save_is_unavailable` 锁定。 |
| 主 bin `save_dwg` | 仍走 `acadrust::DwgWriter::write_to_file`，目标版本由 `doc.header.version` 决定（AC1015/AC1018/AC1021 三档）。 |
| native crate `write_dwg` | 已能写 AC1015 空文档和 LINE/CIRCLE/ARC/POINT/LWPOLYLINE/TEXT 文档，并通过 `write_dwg -> read_dwg` roundtrip；其他 entity 仍 `Unsupported(... pending F5.M5)`。 |
| 版本保真 | `native_bridge` 双向桥接 version 已落地（plan 2026-04-21）；读 R2000 → 写出仍是 R2000，无静默降级 bug。 |
| 保存对话框 | `pick_save_path` 已收敛为单一 `DWG File` filter；无伪装版本标签。 |

### 1.3 文档与对外承诺

- `docs/DEVELOPMENT-PLAN.md` 的 P2 读取三项（解析→model 映射、facade 真实文档、GUI/CLI 打开 UX）**实质均已完成**。
- `README.md` 已校准为：AC1015 / AC1018 native 读取可用；runtime 写入仍走 acadrust；native writer 仅 crate 内 AC1015 LINE/CIRCLE/ARC/POINT/LWPOLYLINE/TEXT tracer bullets。

---

## 2. 目标与决策矩阵

### 2.1 总目标（按价值排序）

1. **诚实化对外承诺**：README / facade 文档与代码事实一致，避免下游误判能力边界。
2. **AC1018 基线稳态**：保持 per-family ratchet，防止静默回归。
3. **写入闭环（最大缺口）**：把 native DWG writer 从 AC1015 LINE/CIRCLE/ARC/POINT/LWPOLYLINE/TEXT tracer bullets 扩到真实样本高价值实体集。
4. **读取版本扩面**：把 AC1014 / AC1021 / AC1024 / AC1027 / AC1032 从 fail-closed 推到至少 weak gate。
5. **EntityData 投影补缺**：Helix / Surface / Light / Camera / Section / ProxyEntity。

### 2.2 决策矩阵（影响 × 成本）

| 工作流 | 用户影响 | 实施成本 | 阻塞依赖 | 推荐节奏 |
|---|---|---|---|---|
| **F1 README 校准** | 中（消除误导） | 极低（≈ 30 min） | 无 | **已完成，后续随里程碑维护** |
| **F2 AC1018 baseline ratchet (R46-F)** | 中（防回归） | 低（0.5–1 天） | 需要 `sample_AC1018.dwg` 在仓库可达 | **已完成** |
| **F3 AC1014 / AC1012 reader bring-up** | 中（覆盖 R14 / R13 老图） | 中（3–5 天） | F2 框架可复用 | 第 2 周 |
| **F4 AC1021/AC1024/AC1027/AC1032 reader** | 高（R2007+ 是主力客户群） | 高（数周，加密 stream + R2007 string stream + R2010+ object class） | F2、F3 经验 | 第 3 周起 |
| **F5 native DWG writer (R2000 起点)** | **极高**（解锁 acadrust 移除） | **极高**（数月，需 bit writer / section composer / handle map / page map） | 无（与 reader 并行） | **M1–M4 已打通 LINE，M5.E1–E5 已打通 CIRCLE/ARC/POINT/LWPOLYLINE/TEXT；下一步 ATTRIB** |
| **F6 EntityData 投影补缺** | 中（覆盖长尾实体） | 中（每个 1–2 天） | 看具体类型 | 穿插，不阻塞主线 |

### 2.3 一句话推荐

**当前建议：保持 F1/F2 文档与基线同步 → 继续推进 F5.M5 writer 实体扩张（ATTRIB / SOLID 优先）→ F3/F4 高版本 reader 与 F6 长尾实体穿插推进。**

---

## 3. F1 — README 与 facade 文档校准

> **状态**：✅ 已完成（后续随代码事实维护）
> **预估**：30 分钟
> **目标**：把对外承诺改成与代码一致，避免误判能力边界。

### 3.1 范围

- `README.md`「Native DWG Parser Status」一节重写。
- `crates/h7cad-native-facade/src/lib.rs` 顶部 doc comment 顺手更新，去掉「today that crate covers AC1015 only and rejects everything else」这句已过时的描述。
- `docs/DEVELOPMENT-PLAN.md` P2 全部勾上「已完成」，并新增 P2.4（writer）作为后续锚点。

### 3.2 非目标

- 不动 facade 的代码契约（`save(Dwg, …)` 仍返回 `Err`）。
- 不写「即将支持」式 marketing 文案，只描述可验证的事实。

### 3.3 任务

| T | 描述 |
|---|---|
| F1.T1 | 重写 `README.md` Native DWG 段：明确 AC1015 完整、AC1018 已接通、其他 fail-closed；写仍走 acadrust。 |
| F1.T2 | 更新 facade `lib.rs` 模块级 doc comment 中关于「AC1015 only」的过时描述。 |
| F1.T3 | `docs/DEVELOPMENT-PLAN.md` 添加 P2.4 native DWG writer 占位条目，引用 §6 (F5)。 |
| F1.T4 | `CHANGELOG.md` 添加「docs: align DWG read/write status with code reality」条目。 |

### 3.4 验收门

- `cargo test --workspace` 不受影响（无代码改动）。
- README 中提到的所有版本号与 `crates/h7cad-native-dwg/src/version.rs` 的 enum 严格对应。
- 不引入新的「即将」「计划中」等模糊表述（marketing 词汇）；只用事实陈述。

### 3.5 风险与退路

- 风险：用户误以为「能写 native DWG」。退路：README 显式写「写仍走 acadrust，native writer 计划见 §F5」。

---

## 4. F2 — AC1018 baseline ratchet（R46-F）

> **状态**：✅ 已完成
> **预估**：0.5–1 天
> **目标**：把 `sample_AC1018.dwg` 的 entity recovery 从 weak gate（≥1）升级为 per-family ratchet，避免静默回归。

### 4.1 范围

- `crates/h7cad-native-dwg/tests/real_samples.rs::real_dwg_samples_baseline_m3b` 增加 AC1018 分支。
- 复用 `Ac1015RecoveryDiagnostics`（diagnostics 已 version-agnostic）。
- 必要时为 AC1018 增加 sample-缺失 soft-skip。

### 4.2 非目标

- 不引入新的实体类型解码（沿用 ~22 类）。
- 不动 AC1015 baseline 数字（保持 ≥ 170 总量等门槛）。
- 不验证字段级保真（只验证 family count）。

### 4.3 任务

| T | 描述 |
|---|---|
| F2.T1 | 跑 `cargo test ac1018_read_dwg_real_sample_recovers_some_entities -- --nocapture`，记录当前实测的每个 family 数量。 |
| F2.T2 | 在 `real_dwg_samples_baseline_m3b` 的 `version == DwgVersion::Ac1018` 分支添加 per-family lower bound（每个 family 实测值 - 1，保留漂移缓冲）。 |
| F2.T3 | 添加 `recovered_total >= measured - 5` 总量下界。 |
| F2.T4 | 跑 `cargo test --locked --workspace --all-targets`：全绿。 |
| F2.T5 | `RUSTFLAGS=-Dwarnings cargo check --locked --workspace --all-targets`：零新 warning。 |
| F2.T6 | CHANGELOG 加条「test: ratchet AC1018 entity recovery baseline」。 |

**执行结果（2026-05-09 复核）**：`real_dwg_samples_baseline_m3b` 已包含 AC1018 分支，当前锁定 `sample_AC1018.dwg` 至少 10 entities、1 CIRCLE、2 HATCH、2 INSERT、5 VIEWPORT、2 block records、2 layouts；实测输出为 11 entities、2 blocks、2 layouts、281 objects。

### 4.4 验收门

- 新基线断言 pass（包含 sample 缺失的 soft-skip 路径）。
- AC1015 基线无回归。
- 非样本机器（CI 上若不带 sample）行为：测试 print「skip」并 return，不 fail。

### 4.5 风险与退路

- **风险**：AC1018 上某些 family 数量本来就为 0，断言会反而失败。
  **退路**：只对实测 > 0 的 family 加 ratchet；其它 family 加 `>= 0` 占位（同时 print 实测值便于后续观察）。
- **风险**：sample_AC1018.dwg 不在仓库可达路径。
  **退路**：保持 `try_read_sample` 的 soft-skip；CI 中显式标注「baseline 仅在含样本环境生效」。

---

## 5. F3 — AC1014 / AC1012 reader bring-up

> **状态**：⏳ 待执行
> **预估**：3–5 天
> **目标**：把 R13 (AC1012) / R14 (AC1014) 从 fail-closed 推到至少能读 header + section map，理想状态产出非空 `CadDocument`。

### 5.1 范围

- `crates/h7cad-native-dwg/src/file_header.rs::section_count_offset(Ac1014)` 等查询表补充 R13/R14 偏移。
- 复用现有 `SectionMap` / `build_pending_document` / `resolve_document` 流水线。
- 实体 body decoder 与 AC1015 共享（R13/R14 与 R2000 的 entity 二进制基本兼容；差异主要在 header 偏移和 section count）。

### 5.2 非目标

- AC1012（R13）当前优先级低于 AC1014，但 enum 已存在；本工作流只承诺 sniff + UnsupportedHeaderLayout 改为「至少能读 header」。
- 不为 AC1014 引入新的 entity body decoder。

### 5.3 任务

| T | 描述 |
|---|---|
| F3.T1 | 调研 ACadSharp `DwgReader.cs` / `DwgFileHeaderReader` 中 R14 / R13 的 header layout 差异（section_count_offset、locator record size、checksum 等）。 |
| F3.T2 | 在 `file_header.rs` 加 R14 / R13 分支；保持 R2000 路径不变。 |
| F3.T3 | 添加 `tests/real_samples.rs::real_dwg_samples_baseline_m3b` 中 AC1014 的弱基线（仅断言 `entities >= 1`）。 |
| F3.T4 | 若 `sample_AC1012.dwg` 可读 header，将其纳入 sniff baseline；否则显式标注 `UnsupportedHeaderLayout` 期望。 |
| F3.T5 | 跑 workspace test + check warnings。 |
| F3.T6 | CHANGELOG。 |

### 5.4 验收门

- `read_dwg(sample_AC1014_bytes)` 返回 `Ok(doc)`，`doc.entities.len() >= 1`。
- AC1015/AC1018 基线无回归。
- 若 R13 不在本工作流闭环，文档明确说明。

### 5.5 风险与退路

- **风险**：R14 与 R2000 的 section locator 排列差异导致 `SectionMap::parse` 越界读。
  **退路**：先实现 R14 专用 `SectionMap::parse_ac1014`，确认稳定后再考虑融合。
- **风险**：R14 的 entity body bit-stream 与 R2000 不完全一致。
  **退路**：发现差异时先在 `enrich_with_real_entities` 内按版本分支，差异点局部化，不污染 AC1015 路径。

---

## 6. F4 — AC1021 / AC1024 / AC1027 / AC1032 reader

> **状态**：⏳ 待执行
> **预估**：4–8 周（按版本分子里程碑）
> **目标**：把 R2007 起的高版本从 fail-closed 推到能读，先 AC1024 (R2010)，再 AC1021 (R2007 加密)，最后 AC1027/AC1032。

### 6.1 范围与里程碑

| 子里程碑 | 版本 | 关键技术点 |
|---|---|---|
| **F4.M1** | AC1024 (R2010) | 基本沿用 AC1018 (R2004) page/section 框架；header 结构变化但 LZ77 / section descriptor 流程相近。 |
| **F4.M2** | AC1021 (R2007) | **string stream 分离**：所有字符串移到独立 stream，需要 `BitReader` 双流模式。**page header 加密增强**。 |
| **F4.M3** | AC1027 (R2013) | object class 列表新增；OBJECT_TYPE 范围扩大。 |
| **F4.M4** | AC1032 (R2018) | 新增 sub-entity 类型；header 字段布局调整。 |

### 6.2 非目标

- 不在本工作流支持加密文件密码解密（`encrypted == 1` 路径）。
- 不在本工作流为新版本支持 ProxyEntity / 新增 entity body decoder（移到 F6）。

### 6.3 通用任务模板（每个子里程碑）

| T | 描述 |
|---|---|
| `*.T1` | 阅读 ACadSharp 对应版本的 `DwgFileHeaderReader` / `Dwg<version>StreamHandler`。 |
| `*.T2` | 在 `crates/h7cad-native-dwg/src/file_header_<version>.rs` 实现版本专属 header 解码。 |
| `*.T3` | 在 `read_dwg` 顶层 dispatch 加版本分支。 |
| `*.T4` | 在 `tests/real_samples.rs` 添加 `<version>_read_dwg_decodes_real_sample_to_non_empty_doc` 弱基线。 |
| `*.T5` | 全部 fail-closed 路径替换为 weak gate。 |
| `*.T6` | 跑 workspace test + check warnings + CHANGELOG。 |

### 6.4 验收门（每个子里程碑）

- `read_dwg(sample_<version>_bytes)` 返回 `Ok(doc)` 且 `entities >= 1`。
- AC1015 / AC1018 基线无回归。
- `Ac1018Decode` 风格的结构化错误（每个 sub-brick 独立 stage 字符串）。

### 6.5 风险与退路

- **风险**：R2007 的 string stream 重构会侵入 `BitReader`，AC1015/AC1018 路径出现回归。
  **退路**：在 `BitReader` 上加 `string_stream: Option<&mut BitReader>` 可选字段；旧路径不传 → 行为不变。
- **风险**：高版本 entity 结构变化使 ~22 类 body decoder 失效。
  **退路**：在 `enrich_with_real_entities` 顶层按版本拒绝某些 family 进入 decoder，确保 fallback 到 `UnsupportedType` 而不是 panic / 假数据。

---

## 7. F5 — native DWG writer（最大缺口）

> **状态**：🚧 已启动；F5.M1–M4 已打通 AC1015 空文档与单 LINE roundtrip，F5.M5.E1–E5 已打通 CIRCLE/ARC/POINT/LWPOLYLINE/TEXT，下一步是 ATTRIB
> **预估**：3–6 个月（按子里程碑分批交付）
> **目标**：把 facade 的 `Err("native DWG writer not implemented yet")` 替换为真实写入。第一阶段只承诺 R2000 (AC1015)，与现有 reader 形成读写对偶；runtime 默认保存路径在 M6 前继续走 acadrust。

### 7.1 总体设计原则

1. **从最简单的版本开始**：R2000 (AC1015) 字段最稳定，是 reader 已经吃透的同一版本，read-write roundtrip 测试最容易构造。
2. **复用 reader 的"知识"，不复用其"形状"**：reader 是 `BitReader → pending → resolve`，writer 是 `compose → BitWriter → bytes`，两侧的 entity body 结构定义可以共享 `entity_*.rs`，但写入流程独立。
3. **Roundtrip 是终极验收门**：写出的 DWG 经 native reader 读回，与源文档 entity 级等价。
4. **不引入并行版本**：先 R2000 单一版本闭环，再扩 R2004。承诺前 facade 仍返回 `Err` 与原 placeholder 兼容。

### 7.2 子里程碑

| 里程碑 | 范围 | 出口门 |
|---|---|---|
| **F5.M1 — bit writer 与 fixture 框架** | `BitWriter`（与 `BitReader` 镜像）+ 最小 fixture 测试 | ✅ 已完成 |
| **F5.M2 — section composer** | AC1015 的 6 个 known section 组装：Header / Classes / Handles / ObjFreeSpace / Template / AuxHeader | ✅ 已完成；empty doc roundtrip 通过 |
| **F5.M3 — handle map writer** | `parse_handle_map` 的反函数：modular char + 7-bit chunks 编码 | ✅ 已完成；writer handles section 可被 reader 解回 |
| **F5.M4 — object stream writer** | `enrich_with_real_entities` 的反向：DecodedEntity → bit-stream | ✅ 已完成；单 LINE 文档完整 roundtrip 通过 |
| **F5.M5 — entity body 写入扩张** | 复用 reader 的 22 类 → 各自的 `write_<entity>_geometry` | 🚧 CIRCLE/ARC/POINT/LWPOLYLINE/TEXT 已完成；下一步 ATTRIB / SOLID 等高价值 family |
| **F5.M6 — facade & 主 bin 切换** | facade::save(Dwg, _) 返回 native 结果；主 bin 加 feature flag 在 native 与 acadrust 间切换 | 真实工程 DWG 经 native writer 写出后 acadrust 可读 |
| **F5.M7 — AC1018 写入扩张** | 复用 reader 的 AC1015→AC1018 桥接逻辑反向 | sample_AC1018.dwg 读→写→读 等价 |

### 7.3 关键技术点

#### 7.3.1 BitWriter

`crates/h7cad-native-dwg/src/bit_writer.rs`（新增）。需要镜像 `BitReader` 的所有读法：
- `write_bit / write_bits`
- `write_byte / write_bit_short / write_bit_long / write_bit_double`
- `write_handle / write_text_ascii / write_text_unicode`
- `write_modular_char / write_signed_modular_char`

测试矩阵：所有 reader 单测都对应一个 writer→reader roundtrip 单测，断言读回值等于写入值。

#### 7.3.2 Section composer

每个 known section 的 writer 函数：

```rust
// crates/h7cad-native-dwg/src/writer/section_header.rs
pub fn write_header_section(doc: &CadDocument, writer: &mut BitWriter) -> Result<(), DwgWriteError>;

// crates/h7cad-native-dwg/src/writer/section_handles.rs
pub fn write_handles_section(handle_offsets: &[HandleMapEntry], writer: &mut BitWriter) -> Result<(), DwgWriteError>;
// ...
```

顶层 `write_dwg(doc, &mut bytes)` 按 section_directory_offset 顺序填充每个 section，并最终回填 directory entries。

#### 7.3.3 错误模型

```rust
pub enum DwgWriteError {
    InvalidDocument(String),
    UnsupportedEntity { entity_type: &'static str },
    UnsupportedVersion(DwgVersion),
    SectionTooLarge { section: &'static str, bytes: usize, limit: usize },
    Io(String),
}
```

显式 `From<DwgWriteError> for String` 便于 facade 沿用 `Result<Vec<u8>, String>` 契约。

### 7.4 任务清单（M1–M4 已落地，M5 起继续）

#### F5.M1 — BitWriter 与 fixture 框架

| T | 描述 | 文件 |
|---|---|---|
| F5.M1.T1 | 创建 `crates/h7cad-native-dwg/src/bit_writer.rs`，实现与 `BitReader` 对偶的写入方法。 | `bit_writer.rs` |
| F5.M1.T2 | 为每个 reader 单测复制一份 writer→reader roundtrip 单测，TDD 风格红→绿。 | `bit_writer.rs` 单测块 |
| F5.M1.T3 | 在 `lib.rs` 暴露 `BitWriter` + `pub fn write_dwg(doc: &CadDocument) -> Result<Vec<u8>, DwgWriteError>`（先返回 Unsupported error）。 | `lib.rs` |
| F5.M1.T4 | 创建 `crates/h7cad-native-dwg/src/error.rs::DwgWriteError`。 | `error.rs` |

#### F5.M2 — Section composer

| T | 描述 | 文件 |
|---|---|---|
| F5.M2.T1 | 创建 `crates/h7cad-native-dwg/src/writer/mod.rs` 与子模块。 | `writer/mod.rs` |
| F5.M2.T2 | 实现 `write_file_header(doc, version, &mut bytes)`，与 `DwgFileHeader::parse` 对偶。 | `writer/file_header.rs` |
| F5.M2.T3 | 实现 6 个 known section 的最小空 payload writer（先用占位 zero-bytes，确保 reader 能 sniff 出 6 个 section）。 | `writer/section_*.rs` |
| F5.M2.T4 | 添加测试：`write_dwg(empty CadDocument) → read_dwg → 6 sections`。 | `tests/roundtrip_minimal.rs` |

#### F5.M3 — Handle map writer

| T | 描述 | 文件 |
|---|---|---|
| F5.M3.T1 | 在 `bit_writer.rs` 加 `write_modular_char` 与 `write_signed_modular_char`。 | `bit_writer.rs` |
| F5.M3.T2 | 创建 `writer/handle_map.rs::write_handle_map(entries, &mut bytes)`，与 `parse_handle_map` 对偶。 | `writer/handle_map.rs` |
| F5.M3.T3 | 测试：构造 5 个 handle_offsets → 写 → 读 → 等价。 | 同上单测块 |
| F5.M3.T4 | 集成进 `write_dwg`：从 `doc.entities` 推导 handle_offsets 并写入 Handles section。 | `lib.rs::write_dwg` |

#### F5.M4 / M5 / M6 / M7 任务

M4 已通过 `writer/document.rs::write_dwg` 接通 LINE；M5.E1–E5 已接通 CIRCLE/ARC/POINT/LWPOLYLINE/TEXT。后续按反向 reader 顺序继续细化；每完成一个 entity body writer（如 ATTRIB / SOLID），就解锁对应 family 的 roundtrip 断言，并逐步建立 `sample_AC1015.dwg` 的 read → write → read family-count 守恒基线。

### 7.5 验收门（每个里程碑）

- 新增的 roundtrip 测试全绿。
- AC1015 / AC1018 reader baseline_m3b 无回归。
- `cargo check --workspace -- -Dwarnings` 干净。
- facade 测试 `dwg_runtime_save_is_unavailable` 在 M5 之前保持锁定 placeholder；M6 切换时同步替换为 roundtrip 断言。

### 7.6 风险与退路

| 风险 | 退路 |
|---|---|
| BitWriter 与 BitReader 在边界条件（最后一个字节剩余 bit、modular char 终止位）出现轻微不对偶。 | TDD：每个单测都 reader.value == writer.value 双向断言。 |
| AC1015 entity body 的某些字段是 reader-only 派生（如 dxf 注释、proxy 数据），写出时找不到来源。 | M5 阶段每写一类 entity 都先做 `read → CadDocument → write → read` roundtrip；丢失字段先标记为已知 gap，不阻塞主线。 |
| Section payload 实测大小超出 R2000 单 section 上限。 | DwgWriteError 显式区分 `SectionTooLarge`，调用方可以选择降级 / 拒绝。 |
| facade 切换到 native writer 后，下游用户在 acadrust 写出与 native 写出的细微差异中陷入。 | M6 加 feature flag `native_dwg_writer = false` 默认关闭，保持 acadrust 为生产链路；启用 flag 由用户主动选择。 |

---

## 8. F6 — EntityData 投影补缺

> **状态**：⏳ 持续穿插
> **目标**：把 `crates/h7cad-native-model::EntityData` 中暂未实现的实体类型投影补全。

### 8.1 优先级队列

| 实体 | 用户出现频率 | reader 实现成本 | 推荐顺序 |
|---|---|---|---|
| **Helix** | 中 | 中 | F6.M1 |
| **Surface**（Plane/Cylinder/Cone/Sphere/Torus）| 中 | 高 | F6.M2 |
| **Light** | 低 | 低 | F6.M3 |
| **Camera** | 低 | 低 | F6.M3 |
| **Section** | 低 | 中 | F6.M4 |
| **ProxyEntity**（preserve only） | 高（兼容性兜底） | 低（不解析，记录原 bytes） | F6.M0 / 提前做 |

### 8.2 通用模板

每个实体一个 PR：
- `crates/h7cad-native-model::EntityData::<NewKind>` 加 variant。
- `crates/h7cad-native-dwg/src/entity_<kind>.rs` 加 reader。
- `lib.rs::try_decode_entity_body` 加 dispatch。
- `tests/real_samples.rs` 加单实体 fixture 测试。
- F5 完成后：同步加 writer。

### 8.3 验收门

- 新 variant 不破坏 `acad_to_truck` / `tessellate` 现有渲染（默认走 fallback）。
- 主 bin `cargo test` 不引入未使用 variant 警告。

---

## 9. 执行节奏

```
Now      F1/F2 状态维护 ───────────────▶ F5.M5.E6 ATTRIB writer
Next     F5.M5.E7 SOLID/Face3D writer  ▶ F3 启动 (AC1014)
Then     F5.M5 高价值实体集             ▶ F4.M1 启动 (AC1024)
Later    F5.M6 facade feature gate      ▶ F4.M2/M3 (AC1021/AC1027)
Later    F5.M7 AC1018 writer            ▶ F6 长尾实体穿插
```

**并行度**：F1 / F2 / F3 / F4 是 reader 端，可与 F5 (writer) 完全并行；F6 的实体扩张可跨二者复用。

**节奏控制**：每周末跑全量 `cargo test --workspace --all-targets` + `cargo check -- -Dwarnings`，确保无静默回归。

---

## 10. Definition of Done

| 项 | 要求 |
|---|---|
| 测试 | 每条新基线断言都有 sample-缺失 soft-skip。AC1015 baseline_m3b 永远不动。 |
| 文档 | 每个完成的子里程碑同步更新 `README.md` Native DWG 段、`docs/DEVELOPMENT-PLAN.md` P2.x、`CHANGELOG.md`。 |
| 错误诚实 | facade `save(Dwg, _)` 在 F5.M6 之前**保持** `Err("native DWG writer not implemented yet")`；不允许提前替换为「实验性」中间产物。 |
| 兼容性 | F4 / F5 任何提交都不允许使 acadrust fallback 路径在错误信息上消失：`OpenNotice` 必须仍能区分「native 走通」「fallback 兜底」「都失败」三态。 |
| Lint | 所有新增文件 `ReadLints` 零错误；workspace `-Dwarnings` 零新增。 |
| 提交粒度 | 一个 PR 只解决一个子里程碑（例如 F5.M1 单独成 PR），便于回滚。 |

---

## 11. 立即可执行的基线命令

```powershell
# Reader baseline：确认 AC1015 / AC1018 当前实测数量
cargo test -p h7cad-native-dwg --test real_samples real_dwg_samples_baseline_m3b -- --nocapture

# Writer tracer baseline：确认 AC1015 empty + LINE/CIRCLE/ARC/POINT/LWPOLYLINE/TEXT roundtrip
cargo test -p h7cad-native-dwg --test roundtrip_minimal -- --nocapture

# 全量基线检查
cargo test --locked --workspace --all-targets
$env:RUSTFLAGS='-Dwarnings'; cargo check --locked --workspace --all-targets; Remove-Item Env:RUSTFLAGS
```

---

## 12. 与现有计划的关系

| 现有计划 | 与本计划的关系 |
|---|---|
| `2026-04-09-dwg-native-port-plan.md` | 本计划是其 M3-C/D 之后的延伸，原计划聚焦 reader，本计划补上 writer 与文档校准。 |
| `2026-04-17-acadrust-removal-plan.md` | 本计划 F5 是其前置：writer 不闭环，acadrust 移除无法启动。 |
| `2026-04-21-dwg-save-version-honesty-plan.md` | 已落地（version 桥接），本计划继承其结果并补上写入器本身。 |
| `2026-04-25-native-dwg-fallback-notice-plan.md` | fallback notice 已上线；本计划 §10 DoD 沿用其错误诚实原则。 |
| `2026-04-28-r46-*-plan.md` 系列 | AC1018 reader 已落地，本计划 F2 是其 R46-F ratchet 收口。 |
| `2026-04-30-h7cad-next-development-plan.md` | 上一份综合路线图，本计划仅聚焦 DWG 子集；OCS / Hatch 等仍按原计划推进。 |

---

*起草者：H7CAD agent；起稿日期 2026-05-08。修订记录追加在文件末尾。*
