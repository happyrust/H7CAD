# H7CAD DWG Section Composer — 最小空 Payload 子计划（F5.M2.T3 + T4）

> **起稿**：2026-05-09
> **父计划**：[`2026-05-08-dwg-next-step-plan.md`](2026-05-08-dwg-next-step-plan.md) §F5.M2
> **里程碑**：F5.M2 收口（writer 第一阶段闭环到「空文档 → 字节流 → 读回」）
> **不在范围**：handle map 真实编码（F5.M3）、object stream（F5.M4）、entity body（F5.M5）、facade 切换（F5.M6）、AC1018 writer（F5.M7）

---

## 0. TL;DR

承接 2026-05-09 落地的 F5.M2.T1（writer 模块骨架）+ T2（AC1015 file header writer），本子计划把 native DWG writer 推进到 **「写出一个最小可被 native reader 读回的空 R2000 文档」** 这条 tracer bullet：

1. 给 6 个 AC1015 known section 各写一份「最小空 payload」composer。
2. 把 file header + section locator directory + 6 个 known section payload 串起来成顶层 `write_dwg(doc) -> Result<Vec<u8>, DwgWriteError>`。
3. 加 1 个集成测试：`write_dwg(empty CadDocument) → read_dwg → 6 个 PendingSection、零 entities、零 layers`。

完成后 facade `save(NativeFormat::Dwg, _)` **仍保持 placeholder**，由 F5.M6 在 entity body writer (M5) 落地后整体切换。

---

## 1. 现状盘点

| 模块 | 状态 |
|---|---|
| `crates/h7cad-native-dwg/src/bit_writer.rs` | F5.M1 完成；30 个对偶 round-trip 单测全绿 |
| `crates/h7cad-native-dwg/src/writer/mod.rs` | F5.M2.T1 完成（骨架 + re-export） |
| `crates/h7cad-native-dwg/src/writer/file_header.rs` | F5.M2.T2 完成；7 个 round-trip 单测全绿 |
| `crates/h7cad-native-dwg/src/writer/section_*.rs` | **本子计划交付** |
| `crates/h7cad-native-dwg/src/lib.rs::write_dwg` | **本子计划交付** |
| `crates/h7cad-native-dwg/tests/roundtrip_minimal.rs` | **本子计划交付** |
| `crates/h7cad-native-facade::save(Dwg, _)` | 维持 `Err("native DWG writer not implemented yet")` |

Reader 端关键事实（决定 writer 行为的最小契约）：

- `read_dwg(bytes)` 的成功路径 = `DwgFileHeader::parse → SectionMap::parse → read_section_payloads → build_pending_document → resolve_document → enrich_with_real_entities`。
- `build_pending_document` 对每个 section 调 `classify_section_records_for_section`；该函数 **`if payload.is_empty() { return Ok(Vec::new()); }`** —— 空 payload 是合法的，落到 `record_count = 0` 的 `PendingSection`。
- Handles section 走 `parse_handle_map`；对空 payload **`continue`**，不报错。
- `KnownSection::Header` 与 `KnownSection::Classes` 在 ACadSharp 中带 16 字节 start/end sentinel；H7CAD reader 当前 **不强制校验** sentinel（`classify_section_records_for_section` 不知道 sentinel 存在）。第一阶段 writer 选择 **不写 sentinel**，让 payload 保持完全空，证明 reader→writer→reader 完整对偶；sentinel 写入推到 M5/M6 acadrust 互通需求出现时再加（到时候单独 TDD slice）。

---

## 2. 目标与决策

### 2.1 目标

| 层 | 目标 |
|---|---|
| 字节级 | `write_dwg(empty doc)` 输出可被 `DwgFileHeader::parse + SectionMap::parse + read_section_payloads` 全部成功消费的字节流。 |
| 语义级 | 输出经 `read_dwg` 读回的 `CadDocument` 与输入空文档语义等价：`entities.is_empty() && layers.is_empty()`。 |
| 接口级 | 顶层 `pub fn write_dwg(doc: &CadDocument) -> Result<Vec<u8>, DwgWriteError>` 暴露在 `lib.rs`；非空 doc 当前返回 `DwgWriteError::Unsupported`，由 M3..M5 逐步解锁。 |
| 错误级 | facade 测试 `dwg_runtime_save_is_unavailable` 保持锁定 placeholder（不接 native writer）。 |

### 2.2 关键决策

| 决策 | 选项 | 选择 | 理由 |
|---|---|---|---|
| Header/Classes 是否写 16 字节 sentinel | (a) 第一阶段就写 (b) 留到 M5/M6 | **(b)** | reader 不校验 sentinel；写它会引入「为什么是这串魔数」的额外不变量。先证「空 payload 对偶」再扩张。 |
| section 在文件中的物理布局 | (a) 紧密排列 (b) 留 padding | **(a) 紧密** | 读路径只信 directory 中的 `offset / size`，紧密排列字节最少；M5 时按需引入 padding 不影响 reader。 |
| section 顺序 | (a) `record_number` 升序 (b) AutoCAD 实际顺序 | **(a)** | 读路径不依赖物理顺序；升序便于 directory 与 payload 块在文件中位置一致，调试更容易。 |
| `write_dwg` 对非空 doc | (a) 报错 (b) 静默丢失 entity | **(a)** `DwgWriteError::Unsupported` | 诚实化原则；与 facade `save` 拒绝原 placeholder 一致。 |
| 是否在 M2 收口时切 facade | (a) 切 (b) 锁住 | **(b)** | facade 切换需要至少一个 entity 类型 roundtrip 才有意义；那是 M5 末尾的工作。 |

### 2.3 不变量（lock 在测试里）

1. **空文档 roundtrip**：`write_dwg(empty) → read_dwg → entities=0 && layers=0 && 6 个 PendingSection`。
2. **section_count 恒等**：写出文件的 `DwgFileHeader::section_count == 6`，directory 长度 == `6 * 9` 字节。
3. **directory 与物理布局对齐**：每个 directory entry 的 `offset` 落在 `[file_header_prefix_len + directory_len, file_size)` 区间内；`offset + size <= file_size`。
4. **non-empty doc 拒绝**：当 `doc.entities` 非空或 `doc.layers` 包含非默认条目时，`write_dwg` 返回 `DwgWriteError::Unsupported`，错误信息提及缺失的 milestone（"entity body writers (F5.M5)"）。

---

## 3. 任务拆解

### 3.1 F5.M2.T3 — 6 个 known section 的最小空 payload writer

每个子任务是一个独立的 vertical slice：**先写 1 个集成测试（RED）→ 实现到通过（GREEN）→ 不重构跳到下一片**。

| ID | 描述 | 文件 | 公共接口 |
|---|---|---|---|
| T3.1 | `Header` section composer | `writer/section_header.rs` | `pub fn write_ac1015_header_section(doc: &CadDocument) -> Result<Vec<u8>, DwgWriteError>` |
| T3.2 | `Classes` section composer | `writer/section_classes.rs` | `pub fn write_ac1015_classes_section(doc: &CadDocument) -> Result<Vec<u8>, DwgWriteError>` |
| T3.3 | `Handles` section composer | `writer/section_handles.rs` | `pub fn write_ac1015_handles_section(handle_offsets: &[HandleMapEntry]) -> Result<Vec<u8>, DwgWriteError>` |
| T3.4 | `ObjFreeSpace` section composer | `writer/section_obj_free_space.rs` | `pub fn write_ac1015_obj_free_space_section() -> Vec<u8>` |
| T3.5 | `Template` section composer | `writer/section_template.rs` | `pub fn write_ac1015_template_section() -> Vec<u8>` |
| T3.6 | `AuxHeader` section composer | `writer/section_aux_header.rs` | `pub fn write_ac1015_aux_header_section() -> Vec<u8>` |

**第一阶段（M2.T3）每个 composer 都返回 `Vec::new()`**（最小空 payload）。

设计 trade-off：

- T3.1 / T3.2：接受 `&CadDocument` 入参（即使现在不用），是为了 M5 阶段无需改公共签名就能加真实序列化逻辑。
- T3.3：接受 `&[HandleMapEntry]` 入参，预留给 M3 真正的 handle map writer；M2 阶段只对空切片返回空 payload。
- T3.4 / T3.5 / T3.6：无入参，因为 reader 当前对它们零依赖。

**单测 / 集成测试不写在子模块里**（避免 horizontal slicing），统一放在 T4.1 的 `tests/roundtrip_minimal.rs` 里通过 `write_dwg → read_dwg` 间接验证。

### 3.2 F5.M2.T4 — 顶层 `write_dwg(doc)` 接面

| ID | 描述 | 文件 |
|---|---|---|
| T4.1 | 实现 `pub fn write_dwg(doc: &CadDocument) -> Result<Vec<u8>, DwgWriteError>`：组装 file header + directory + 6 个 section payload；非空 doc → `Unsupported`。 | `lib.rs`（或新增 `writer/document.rs` 后从 `lib.rs` re-export） |
| T4.2 | 创建 `tests/roundtrip_minimal.rs`：测试 1（空 doc roundtrip）+ 测试 2（non-empty doc 被 `Unsupported` 拒绝）+ 测试 3（writer→reader 字节级不变量：section_count = 6、directory 偏移合法）。 | `crates/h7cad-native-dwg/tests/roundtrip_minimal.rs` |
| T4.3 | 在 `writer/mod.rs` re-export `write_ac1015_<section>_section` 系列；在 `lib.rs` re-export `write_dwg`、`DwgWriteError`（DwgWriteError 已 re-exported，复核即可）。 | `writer/mod.rs`、`lib.rs` |
| T4.4 | 更新 `CHANGELOG.md`、`progress.md`、`findings.md`：记录 M2 收口与下一步 M3 / M5。 | 三份文档 |

### 3.3 完成标志

- [ ] 6 个 `writer/section_*.rs` 文件落地，每个文件一个 vertical slice TDD 周期。
- [ ] `write_dwg` 暴露在 `lib.rs` 公共表面。
- [ ] `tests/roundtrip_minimal.rs` 至少 3 个 case 全绿。
- [ ] `cargo test -p h7cad-native-dwg --all-targets` 全绿，无回归。
- [ ] `cargo check --workspace --all-targets` 编译通过，零新警告。
- [ ] CHANGELOG / progress / findings 三份文档同步。
- [ ] facade 测试 `dwg_runtime_save_is_unavailable` 保持锁定 placeholder。

---

## 4. TDD 流程（vertical slice）

> **铁律**：禁止 horizontal slicing（先写 6 个红测再写 6 个绿实现）。每片是独立的 RED → GREEN → REFACTOR 周期，互不阻塞。

### 4.1 推荐顺序

| 步 | 测试焦点 | 实现焦点 | 解锁价值 |
|---|---|---|---|
| 1 | `tests/roundtrip_minimal.rs::write_dwg_empty_doc_round_trips_through_read_dwg` | `write_dwg(doc)` + 6 个空 composer + section directory 物理布局 | tracer bullet：整条流水线打通一次 |
| 2 | `tests/roundtrip_minimal.rs::write_dwg_rejects_non_empty_entities_with_unsupported` | `write_dwg` 在 `doc.entities` 非空时早返回 `Unsupported`；错误信息包含 "F5.M5" 指引 | 诚实化承诺；防止后续误调用 |
| 3 | `tests/roundtrip_minimal.rs::write_dwg_directory_offsets_are_within_file_bounds` | 加 `assert!(entry.offset + entry.size <= bytes.len())` for all 6 entries | 字节级不变量自检 |
| 4（可选） | `tests/roundtrip_minimal.rs::write_dwg_section_payloads_are_byte_for_byte_empty` | 在 reader 读出的 6 个 `PendingSection` 上 `assert!(section.payload.is_empty())` | 确认「空」语义而不是「空+占位字节」 |

### 4.2 每个红绿循环的微步骤

```
RED   → 写测试（用 write_dwg + read_dwg 调用对，断言行为）
        cargo test -p h7cad-native-dwg --test roundtrip_minimal <name>
        → 确认红
GREEN → 实现到能通过；不超出当前测试需要
        cargo test -p h7cad-native-dwg --test roundtrip_minimal <name>
        → 确认绿
REFACTOR
      → 仅在绿态下；不改变 public 接口
        cargo test -p h7cad-native-dwg --all-targets
        → 全绿确认
```

---

## 5. 测试矩阵

| 测试 | 类型 | 文件 | 验证内容 |
|---|---|---|---|
| `write_dwg_empty_doc_round_trips_through_read_dwg` | 集成 | `tests/roundtrip_minimal.rs` | 空 doc 写出 → 读回 → entities=0 / layers=0 / 6 个 PendingSection |
| `write_dwg_rejects_non_empty_entities_with_unsupported` | 集成 | `tests/roundtrip_minimal.rs` | 非空 doc → `Err(Unsupported(_))`，错误信息含 "F5.M5" |
| `write_dwg_directory_offsets_are_within_file_bounds` | 集成 | `tests/roundtrip_minimal.rs` | 每个 directory entry 的 `[offset, offset+size)` ⊂ `[0, bytes.len())` |
| `write_dwg_section_payloads_are_byte_for_byte_empty` | 集成 | `tests/roundtrip_minimal.rs` | 6 个 `PendingSection.payload` 都是 0 字节 |
| 既存：`prefix_*` / `directory_*`（M2.T2） | 单元 | `writer/file_header.rs` | 7 个 file header writer 单测保持全绿 |
| 既存：`bit_writer::tests::*`（M1） | 单元 | `bit_writer.rs` | 30 个 BitWriter 单测保持全绿 |
| 既存：`real_dwg_samples_baseline_m3b` | 集成 | `tests/real_samples.rs` | AC1015 / AC1018 真实样本 reader 基线无回归 |

---

## 6. 验收门

| 项 | 要求 |
|---|---|
| 测试 | 上述 4 个新增 case + 全部既存 native-dwg 测试 = 全绿。`cargo test -p h7cad-native-dwg --all-targets` 无回归。 |
| 编译 | `cargo check --workspace --all-targets`：零新增 warning（既存 H7CAD bin dead-code warning 不计）。 |
| Lint | 新文件 `ReadLints` 零错误。 |
| 格式 | 新文件 `rustfmt --edition 2021 --check` 通过。 |
| 文档 | `CHANGELOG.md [未发布]` 段加 2026-05-09 F5.M2 收口条目；`progress.md` / `findings.md` 同步更新；本计划文件加「执行记录」附录。 |
| 接口诚实 | facade `dwg_runtime_save_is_unavailable` 测试无变化；`write_dwg` 对非空 doc 显式 `Unsupported`，错误文案指向 F5.M5。 |
| 提交粒度 | 一次提交可包含 T3.1..T3.6 + T4 全部，因为它们共享同一 tracer bullet 测试；不要拆成 6 个 PR。M3 起才单独成 PR。 |

---

## 7. 风险与退路

| 风险 | 触发条件 | 退路 |
|---|---|---|
| 空 payload 让 reader 在 `resolve_document` 报错 | `resolve_document` 期望至少 1 个 BLOCK_RECORD 或 LAYER 表 | 在 `roundtrip_minimal.rs` 调试时观察 `read_dwg` 返回的 Err；如确实需要最小占位记录，则在 `write_ac1015_handles_section` 的对应位置补一个 NULL handle 条目。**不要**为此引入 sentinel；保持「为什么这是最小」可追溯。 |
| `enrich_with_real_entities` 对空文件 panic | 实体段读取的 cursor 越界 | 同上：在 `tests/roundtrip_minimal.rs` 上 reproduce → 在 reader 端加 length-guard（reader 修，不污染 writer）。这是发现 reader 隐藏 bug 的契机，按 fix 路径处理。 |
| 测试 1 立刻失败但根因在 reader | `read_dwg` 在某处对最小文件不健壮 | 把 reader 修复作为前置 PR，单独走 reader 路径的 TDD；写 reader 路径单测锁定行为，再回到 writer。 |
| section directory 的 `offset` 计算 off-by-one | 物理 payload 与 directory `offset` 不一致 | 测试 3 立即捕获；在 `write_dwg` 内用一个**单一 `cursor: u32`** 累加器代替手动算偏移，结构上避免重复计算。 |
| `write_dwg` 接口签名后期需要扩展 | M3 引入 handle map 时 `write_dwg` 需要传更多信息 | 接口签名仅承诺 `(&CadDocument) -> Result<Vec<u8>, DwgWriteError>`；内部用 builder 模式（后续可加 `WriteOptions`），不破坏外部调用。 |

---

## 8. 时间表

| 阶段 | 估时 | 备注 |
|---|---|---|
| T3.1..T3.6（6 个空 composer） | 30 min | 每个 5 min，纯样板代码 |
| T4.1 `write_dwg` + directory 物理布局 | 1.5 h | tracer bullet 测试驱动；要解决 cursor 累加 + non-empty 拒绝 |
| T4.2 集成测试 4 个 case | 1 h | 每个 case ~15 min |
| T4.3 + T4.4 文档与 re-export | 30 min | 模板照抄本子计划末尾 |
| 缓冲（reader 隐藏 bug、CI 偶发） | 1 h | 见 §7 |
| **合计** | **4 h** | 半天工作量 |

如果 §7 的"reader 在最小空文件 panic"风险触发，再加 0.5–1 day。

---

## 9. 与现有计划的关系

| 计划 | 关系 |
|---|---|
| `2026-05-08-dwg-next-step-plan.md` | 本子计划是其 §F5.M2 的展开，不替换；M2 收口后回到主计划 §F5.M3 (handle map)。 |
| `2026-04-21-dwg-save-version-honesty-plan.md` | 已落地；本计划继承「写入版本由 `doc.header.version` 决定」的诚实承诺，但只承诺 R2000；非 R2000 doc 当前直接 `Unsupported`。 |
| `2026-04-25-native-dwg-fallback-notice-plan.md` | 仍是 reader 端 fallback 机制；writer 路径目前没有 fallback（facade 直接报错），不引入新通知。 |

---

## 10. 立即可执行的第一批命令

```powershell
# 起步：先看红
cargo test -p h7cad-native-dwg --test roundtrip_minimal -- --nocapture

# T4.1 实现循环
cargo test -p h7cad-native-dwg --test roundtrip_minimal write_dwg_empty_doc_round_trips_through_read_dwg -- --nocapture

# 全绿确认
cargo test -p h7cad-native-dwg --all-targets
$env:RUSTFLAGS='-Dwarnings'; cargo check -p h7cad-native-dwg --all-targets; Remove-Item Env:RUSTFLAGS

# workspace 收尾（避免主 bin OOM 用 check 替代 test）
cargo check --workspace --all-targets
```

---

## 11. 完成后的下一步路标

| 下一片 | 入口 |
|---|---|
| F5.M3 — handle map writer | `writer/section_handles.rs::write_ac1015_handles_section` 升级为 `parse_handle_map` 反函数；roundtrip 5 个合成 entry。 |
| F5.M4 — object stream writer | `writer/object_stream.rs::write_object_stream`；与 `enrich_with_real_entities` 反向。 |
| F5.M5 — entity body writers | 22 类 `write_<entity>_geometry`，与 reader 端的 `entity_*.rs` 对偶。每类一个 PR，每类解锁 baseline_m3b 中对应 family 的 roundtrip。 |
| F5.M6 — facade 切换 | `crates/h7cad-native-facade::save(Dwg, _)` 接 `write_dwg`；删 placeholder 字符串；改 `dwg_runtime_save_is_unavailable` 为 roundtrip 断言。 |
| F5.M7 — AC1018 writer | 复用 reader 端 AC1015→AC1018 桥接的反向；引入 LZ77 encoder + page header XOR + section descriptor map writer。 |

---

## 12. 执行记录

| 日期 | Task | 简述 | 验证 |
|---|---|---|---|
| 2026-05-09 | T3.1 | `write_ac1015_header_section(&CadDocument) -> Result<Vec<u8>, DwgWriteError>`，返回 `Ok(Vec::new())` | 由 T4 集成测试覆盖 |
| 2026-05-09 | T3.2 | `write_ac1015_classes_section`，返回空 | 同上 |
| 2026-05-09 | T3.3 | `write_ac1015_handles_section(&[HandleMapEntry])`：空 → 空 payload；非空 → `Unsupported`（指向 F5.M3） | 同上 |
| 2026-05-09 | T3.4 | `write_ac1015_obj_free_space_section() -> Vec<u8>`，返回空 | 同上 |
| 2026-05-09 | T3.5 | `write_ac1015_template_section()`，返回空 | 同上 |
| 2026-05-09 | T3.6 | `write_ac1015_aux_header_section()`，返回空 | 同上 |
| 2026-05-09 | T4.1 | `writer/document.rs::write_dwg(&CadDocument) -> Result<Vec<u8>, DwgWriteError>` 实现：file header + 6×9 directory + 6 个空 payload；非空 entities → `Unsupported`；offset 用 `cursor: u32 + checked_add` 防 off-by-one；溢出 → `SectionTooLarge` | 4 个集成测试全绿 |
| 2026-05-09 | T4.2 | `tests/roundtrip_minimal.rs` 4 个 case 落地。RED：测试 1 草稿断言 `parsed.layers.is_empty()` 与事实不符（`CadDocument::new()` 预置 `0` layer 等默认表）；GREEN：改为 `parsed.layers == fresh.layers && parsed.linetypes == fresh.linetypes`，证明 writer 没新增/丢失任何默认表条目 | `cargo test -p h7cad-native-dwg --test roundtrip_minimal`：4/4 |
| 2026-05-09 | T4.3 | `writer/mod.rs` 与 `lib.rs` re-export `write_dwg`、6 个 section composer、`AC1015_EMPTY_DWG_MIN_LEN`、`AC1015_KNOWN_SECTION_COUNT` | `cargo check -p h7cad-native-dwg --all-targets` 通过 |
| 2026-05-09 | T4.4 | `CHANGELOG.md` / `progress.md` / `findings.md` 同步；本计划文件填执行记录 | — |

**收口验收**

- `cargo test -p h7cad-native-dwg --all-targets`：lib 208 + read_headers 53 + real_samples 38 + roundtrip_minimal 4 = **303 全绿**。
- `cargo check --workspace --all-targets`：通过；零新增 warning（10 个 dead-code 全在 `H7CAD` 主 bin，先前已存在）。
- `RUSTFLAGS=-Dwarnings cargo check -p h7cad-native-dwg --all-targets`：零警告。
- 新文件 `rustfmt --edition 2021 --check` 通过；`ReadLints` 零错误。
- facade `dwg_runtime_save_is_unavailable` 测试无变化（按计划锁定 placeholder 至 F5.M6）。

**M2 收口；下一片为 F5.M3（handle map writer）。**

---

*起草者：H7CAD agent；2026-05-09。本文件落地后置入 `docs/plans/`；如范围调整请先改本文件再改代码。*
