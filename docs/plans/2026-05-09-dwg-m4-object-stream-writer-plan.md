# H7CAD F5.M4 — Object Stream Writer 子计划

> **起稿**：2026-05-09
> **父计划**：[`2026-05-08-dwg-next-step-plan.md`](2026-05-08-dwg-next-step-plan.md) §F5.M4
> **前置**：F5.M1 BitWriter / F5.M2 file header + section composer / F5.M3 handle map writer 全部已落地
> **目标**：写出能被 `read_dwg` 完整读回的 1 个 LINE entity 文档（reader→writer→reader 字节级 + 语义级 roundtrip）
> **不在范围**：M5 的其他 21 类 entity body writer；M6 的 facade 切换；M7 的 AC1018 writer

---

## 0. TL;DR

把 native DWG writer 从「写出空 R2000 文档」推进到「**写出含 1 条 LINE 的 R2000 文档并能被 reader 还原**」。这条 tracer bullet 一旦打通，剩余 21 类 entity（M5）就是按部就班的 entity body writer 拼接，每类一个 PR。

Reader 端的对偶链是：

```
read_dwg
 ├─ DwgFileHeader::parse                  (F5.M2.T2 ✓)
 ├─ SectionMap::parse                     (F5.M2.T2 ✓)
 ├─ read_section_payloads                 (F5.M2.T3 ✓)
 ├─ build_pending_document
 │   ├─ parse_handle_map                  (F5.M3 ✓)
 │   └─ classify_section_records_for_section
 ├─ resolve_document
 └─ enrich_with_real_entities
     ├─ ObjectStreamCursor::object_slice_by_handle    ← M4.A 物理布局
     ├─ split_ac1015_object_streams                   ← M4.B 拆 main/handle 流
     │   └─ read_ac1015_object_header                 ← M4.A 三字段
     └─ try_decode_entity_body
         ├─ parse_ac1015_entity_common                ← M4.C 公共字段
         └─ read_line_geometry                        ← M4.D LINE 专属
```

每条 reader 入口都有对应 writer 子里程碑。M4.A→D 完成后 M4.E 把全部串到 `write_dwg`，并把 doc.entities 推导成 `handle_offsets` 喂给 M3 的 handle map writer。

---

## 1. 现状盘点（写入侧）

| 模块 | 状态 |
|---|---|
| `writer/file_header.rs` | ✓ M2.T2 |
| `writer/section_*.rs` 6 个空 payload composer | ✓ M2.T3 |
| `writer/document.rs::write_dwg` | ✓ M2.T4（仅空 doc） |
| `writer/handle_map.rs` | ✓ M3 |
| `bit_writer.rs` 30 个 primitive | ✓ M1（含 `write_bit_short`、`write_raw_u32_le`、`write_handle`、`write_bit_double_with_default`、`write_bit_extrusion_r2000_plus`、`write_bit_thickness_r2000_plus`、`write_text_ascii` 等 LINE 与公共字段全部用得上的方法） |
| **`writer/object_header.rs`** | **本子计划 M4.A** |
| **`writer/object_slice.rs`**（compose MS + body + CRC，merge main/handle streams） | **本子计划 M4.B** |
| **`writer/entity_common.rs`** | **本子计划 M4.C** |
| **`writer/entity_line.rs`** | **本子计划 M4.D** |
| **`writer/document.rs::write_dwg` 接 entities** | **本子计划 M4.E** |
| `writer/section_handles.rs` 接 derived offsets | M4.E（当前对空 entries 走空路径） |

---

## 2. 子里程碑分解

每个里程碑是一个独立 PR-friendly 的 vertical slice（TDD red-green-refactor），上一步通过下一步才动手。

### 2.1 M4.A — Object header writer

**Scope**：把 `ObjectHeader { object_type, main_size_bits, handle, handle_code }` 三字段写到 `BitWriter`，与 `read_ac1015_object_header` 字节级对偶。

**接口**：

```rust
pub fn write_ac1015_object_header(
    header: ObjectHeader,
    writer: &mut BitWriter,
) -> Result<(), DwgWriteError>;
```

**测试**：

1. `object_header_round_trips_via_read_ac1015_object_header`：构造 `ObjectHeader { object_type: 17, main_size_bits: 256, handle: 0x2A, handle_code: HANDLE_CODE_HARD_OWNER }` → write → 加 MS prefix → `read_ac1015_object_header` → 等价。
2. `object_header_rejects_handle_code_above_nibble`：`handle_code > 0x0F` 触发 `BitWriter::write_handle` 的内部 `InvalidValue`，验证错误透传。
3. `object_header_writes_to_known_bit_count`：1 字节 handle 时刚好 58 bits（参考 reader 的 `reader_positioned_exactly_after_header` 对偶）。

**估时**：30 min。

### 2.2 M4.B — Object slice composer

**Scope**：拼接 main_stream + handle_stream 到一个 body byte buffer，加 MS size prefix 与 2 字节 CRC 占位，输出最终 object slice。

**接口**：

```rust
pub fn compose_ac1015_object_slice(
    header: ObjectHeader,
    main_stream: &BitWriter,
    handle_stream: &BitWriter,
) -> Result<Vec<u8>, DwgWriteError>;
```

主要工作：
- 创建一个 `BitWriter` `body`：先 `write_ac1015_object_header(header, &mut body)`，然后追加 main_stream 的 bits（pad align 到 `header.main_size_bits` 绝对位置），再追加 handle_stream 的 bits。
- `header.main_size_bits` 由调用方提供，必须等于 "header bits + main_stream bits"。compose 内部断言这一点（防 off-by-one）。
- body bytes 长度 = ceil(total_bits / 8)；MS = body.len()。
- 输出 = `MS prefix + body + 0x00 0x00 (CRC stub)`。

**测试**：
1. `object_slice_round_trips_through_split_ac1015_object_streams`：构造 main_stream（写一个 bit_short）+ handle_stream（写一个 handle）→ compose → split → 三流字段恢复等价。
2. `object_slice_rejects_main_size_bits_inconsistent`：调用方传错的 `main_size_bits` 触发 `InvalidValue`。

**估时**：1.5h（涉及 bit-level concatenation 的 trick）。

### 2.3 M4.C — Common entity header writer

**Scope**：写 `parse_ac1015_entity_common` 读到的所有字段——EED、graphic flag、entity mode、reactor list、layer handle、linetype handle、color、lineweight、ltype scale、invisible、material、shadow flag、xdata、ownership 等 22 字段。

**Scope 削减**：先只支持「最小可能配置」——空 EED、no-graphic flag、entity_mode = 「by layer ref」、空 reactor list、layer handle 0x0、linetype = ByLayer、color 0、空 xdata。这些是 reader 端 `parse_ac1015_entity_common` 都允许的「全 default」配置。

**接口**：

```rust
pub fn write_ac1015_entity_common_minimal(
    layer_handle: Handle,
    main_writer: &mut BitWriter,
    handle_writer: &mut BitWriter,
) -> Result<(), DwgWriteError>;
```

主流写公共 bit 字段；handle 流写 layer 的 handle ref。两边交错写入是 reader 端 `split_ac1015_object_streams` 的强约定。

**测试**：
1. `common_minimal_round_trips_through_parse_ac1015_entity_common`：构造空 fixture → write → split → parse → 等价。
2. `common_round_trip_preserves_layer_handle_link`：layer_handle 写出的 `(code, value)` 与 reader 端 `Ac1015EntityCommonData::layer_handle` 读到的等价。

**估时**：3-5h（这是 M4 最复杂的一砖；`parse_ac1015_entity_common` 自身 600+ 行）。**风险**：reader 端某些 default 假设难以从代码反推；可能需要找 ACadSharp 源码交叉验证。

### 2.4 M4.D — LINE entity body writer

**Scope**：与 `read_line_geometry` 对偶。

**接口**：

```rust
pub fn write_line_geometry(
    geom: LineGeometry,
    writer: &mut BitWriter,
) -> Result<(), DwgWriteError>;
```

按 reader 端的 `[B z_are_zero][RD sx][DD ex (default = sx)][RD sy][DD ey][?RD sz?DD ez][BT thickness][BE extrusion]` 字节级 mirror。

**测试**：

1. `line_geometry_round_trips_2d_synthesis`：start=(1,2,0) end=(4,5,0) thickness=0 extrusion=default → write → `read_line_geometry` → 等价。
2. `line_geometry_round_trips_3d_with_z`：z_are_zero=0 路径 → 含 sz/ez。
3. `line_geometry_round_trips_nontrivial_thickness_extrusion`。
4. `line_geometry_uses_dd_default_when_end_equals_start`：end == start 时 DD prefix 应该选 `00`（payload-free，节省 64 bits）。

**估时**：1-2h。

### 2.5 M4.E — `write_dwg` 集成

**Scope**：把 doc.entities 推导成 object stream，更新 `write_dwg`，让单 LINE roundtrip。

**任务**：

1. 在 `write_dwg` 内部按 doc.entities 顺序：对每个 entity 用 M4.C/D 写出 main + handle stream → compose object slice (M4.B) → 累加 cursor 记录 `(handle, file_offset)`。
2. 用累加得的 `Vec<HandleMapEntry>` 调 `write_ac1015_handle_map_payload`（M3）替换 `write_ac1015_handles_section` 的空 payload。
3. 把所有 object slice + handle map payload 物理排列到 file 的合适 region；调整 directory descriptor 的 offset/size。
4. 删除 `write_dwg` 内 `if !doc.entities.is_empty() Unsupported` 早返回；改为只对**未实现的 entity 类型** Unsupported（M4 阶段只 LINE 通过；其他类型按 M5 解锁）。
5. `tests/roundtrip_minimal.rs` 加 `write_dwg_with_single_line_entity_round_trips_through_read_dwg`。
6. 既存 4 个 minimal 测试中：
   - `write_dwg_rejects_non_empty_entities_with_unsupported`：升级为「未支持的 entity 类型」拒绝（如 CIRCLE / ARC），LINE 不再触发它。
   - 其他 3 个不变。

**估时**：3-5h（含调试物理布局 off-by-one 的预期成本）。

### 2.6 退路与风险

| 风险 | 触发 | 退路 |
|---|---|---|
| `parse_ac1015_entity_common` 的某字段 default 难反推 | M4.C 实现卡住 | 用 ACadSharp `DwgObjectReader.cs` 交叉验证；必要时把 M4.C 拆成 M4.C.1（写最简单字段）+ M4.C.2（增量加字段）多 PR |
| object slice 物理布局在 reader 端某 corner 上不对偶 | M4.B 的 roundtrip 测试红 | 加 trace 工具 dump bit-by-bit 对比 reader 期望与 writer 输出；先把分歧点限制在 1 个 entity 上 |
| `main_size_bits` 与 reader 端 absolute body-bit 计算 mismatch | M4.B compose 失败 | 写一个调试 fixture：手动 hex-dump 一个真 LINE object slice，writer 输出与之 byte-by-byte 比较 |
| handle_map 的 `(handle, offset)` 在 file 中的绝对位置算错 | M4.E roundtrip 失败 | M3 的 `write_ac1015_handle_map_payload` 已稳；问题一定在 cursor 累加；用 `print` 打印每个 entity 的 file offset 调试 |

---

## 3. 验收门（每个子里程碑共享）

- 新单测全绿；既存测试零回归。
- `cargo test -p h7cad-native-dwg --all-targets` 全绿。
- `cargo check --workspace --all-targets` 通过；零新警告。
- 新文件 `rustfmt --edition 2021 --check` 通过；`ReadLints` 零错误。
- facade `dwg_runtime_save_is_unavailable` 测试**仅在 M4.E 完成且 LINE roundtrip 通过后**才允许变化（按主计划 §10 DoD）。当前 M4.A..D 阶段保持锁定。

---

## 4. 时间表（按 vertical slice 顺序）

| 段 | 子里程碑 | 预估 |
|---|---|---|
| 本轮 | **M4.A 启动** | 30 min |
| 下一轮 | M4.B object slice composer | 1.5h |
| +1 | M4.C 公共字段 minimal | 3-5h |
| +1 | M4.D LINE body writer | 1-2h |
| +1 | M4.E `write_dwg` 集成 + LINE roundtrip 集成测试 | 3-5h |
| **合计** | | **9-14h**（约 1-2 个工作日） |

每段独立成 PR，按顺序合入 main。

---

## 5. 与父计划的关系

| 父计划条目 | 关系 |
|---|---|
| §7.2 F5.M4「object stream writer」 | 本子计划是其展开；M4.E 完成时父计划 §7.2 标记完成 |
| §7.4 M3.T4「集成进 write_dwg：从 doc.entities 推导 handle_offsets」 | 移到本子计划 M4.E（语义上属于 M4 的 doc.entities 推导链路） |
| §7.5 验收门 | 复用 |

---

## 6. 执行记录（落地后填充）

| 日期 | 子里程碑 | 简述 | 验证 |
|---|---|---|---|
| 2026-05-09 | M4.A | `writer/object_header.rs::write_ac1015_object_header` 写入 BS/RL/H 三字段；与 `read_ac1015_object_header` 对偶 | 4 个 unit test 全绿；`cargo test -p h7cad-native-dwg --all-targets` 319 全绿，零回归 |
| 2026-05-09 | M4.B | `modular.rs::write_modular_short` + `writer/object_slice.rs::compose_ac1015_object_slice`：拼 header + main 流 + handle 流 + MS prefix + CRC 占位，与 `split_ac1015_object_streams` 对偶 | 6 个 unit test 全绿（含 main_size_bits 不一致拒绝、空流 edge case）；`cargo test -p h7cad-native-dwg --all-targets` 325 全绿，零回归 |
| 2026-05-09 | M4.C | `writer/entity_common.rs`: `EntityCommonMinimal` + `write_ac1015_entity_common_minimal` 镜像 `parse_ac1015_entity_common_after_extended_data`（minimal 配置） | 5 个 unit test 首次全绿；`cargo test -p h7cad-native-dwg --all-targets` 330 全绿，零回归。详见 `2026-05-09-dwg-m4c-entity-common-minimal-plan.md` §9 |
| 2026-05-09 | M4.D | `writer/entity_line.rs::write_line_geometry` 镜像 `read_line_geometry`：z_are_zero 自动检测、紧凑 thickness/extrusion、`-0.0` z 边界处理 | 5 个 unit test 首次全绿（含 135 bits 精确预测）；`cargo test -p h7cad-native-dwg --all-targets` 335 全绿。详见 `2026-05-09-dwg-m4d-line-body-writer-plan.md` §10 |

