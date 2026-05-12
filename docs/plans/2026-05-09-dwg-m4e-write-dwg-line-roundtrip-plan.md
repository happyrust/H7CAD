# H7CAD F5.M4.E — `write_dwg` LINE Entity Roundtrip 集成子计划

> **起稿**：2026-05-09
> **父计划**：[`2026-05-09-dwg-m4-object-stream-writer-plan.md`](2026-05-09-dwg-m4-object-stream-writer-plan.md) §2.5
> **依赖**：F5.M2 / M3 / M4.A / M4.B / M4.C / M4.D 全部落地
> **里程碑**：F5.M4.E —— `write_dwg(doc)` 接受含 LINE 的 doc，并经 `read_dwg` 字节级 + 语义级 roundtrip
> **目标**：完成 F5.M4 整体，让 native DWG writer 第一次写出可被 reader 读回的真实 entity 文档
> **预估**：3–5h

---

## 0. TL;DR

把 M4.A..D 拼起来：

1. 对 doc.entities 中每个 entity，写出 (main_writer, handle_writer) 两个 BitWriter（M4.C common header + M4.D entity body），调 M4.B compose 出 object slice bytes，累加 cursor 记 `(handle, file_offset)`。
2. 用累加的 `Vec<HandleMapEntry>` 调 M3 的 `write_ac1015_handle_map_payload` → handles section payload。
3. 重新排物理布局：`file_header + directory + object_slices + 6 个 section payloads`（object_slices 在 sections 之**前**，offsets 计算更直观）。
4. 顶层 `write_dwg` 删除 `if !doc.entities.is_empty() Unsupported` 早返回；改为对**每个 entity 的具体类型**逐一分发（LINE → 写；其他 → `Unsupported` 指向 F5.M5）。

---

## 1. 物理布局重排（关键设计决策）

### 1.1 现状（M2.T4 / M3）

```text
[file_header 0x19][directory 6×9 = 54][6 个 section payloads（全空）]
file_size = 0x19 + 54 = 79 bytes
```

### 1.2 M4.E 之后

```text
[file_header 0x19][directory 6×9 = 54][object_slices 总字节数 N][6 个 section payloads]
file_size = 0x19 + 54 + N + sum(section_payload_sizes)
```

object_slices 紧贴 directory 之后开始；每个 object slice 的绝对 file_offset 从 `0x19 + 54 = 79` 起累加。Section payloads 在 object_slices 之后；directory 中的 section descriptors 的 offset 字段反映这一点。

### 1.3 为什么 object_slices 在 sections 之前

**手指算 offset 的便利性**：handle_offsets 是 entity-by-entity 的累加表；如果先写 sections，handles section payload 又依赖 handle_offsets，会形成循环依赖。把 object_slices 放在最前消除循环。

**reader 端无要求**：reader 通过 `ObjectStreamCursor::object_slice_by_handle(handle)` 按 handle map 的 offset 索引 object slices —— 它只信任 offset，不要求 object_slices 物理位置在 sections 之前还是之后。

### 1.4 边界情形

- doc.entities 空 → object_slices_total = 0 → 物理布局退化到 M2.T4 时的形态；既存 4 个 roundtrip_minimal 测试不变。
- doc.entities 含 N 个 LINE → object_slices_total = sum(每个 LINE object slice 字节数) → handles section payload 含 N 条 entries。

---

## 2. 接口与算法

### 2.1 `write_dwg` 主流程伪码

```rust
pub fn write_dwg(doc: &CadDocument) -> Result<Vec<u8>, DwgWriteError> {
    // Step 1 (delete-existing): drop the M2 `if !doc.entities.is_empty()
    // Unsupported` early return. Replace with per-entity-type dispatch.
    let mut object_slices: Vec<Vec<u8>> = Vec::with_capacity(doc.entities.len());
    let mut handle_offsets: Vec<HandleMapEntry> = Vec::with_capacity(doc.entities.len());
    let object_slices_start: u32 = (AC1015_FILE_HEADER_PREFIX_LEN
        + (AC1015_KNOWN_SECTION_COUNT as usize) * AC1015_SECTION_LOCATOR_ENTRY_LEN) as u32;
    let mut cursor: u32 = object_slices_start;
    for entity in &doc.entities {
        let (main, handle, header) = encode_entity(entity)?;
        let slice = compose_ac1015_object_slice(header, &main, &handle)?;
        let size = u32::try_from(slice.len()).map_err(|_| /* SectionTooLarge */ )?;
        handle_offsets.push(HandleMapEntry {
            handle: entity.handle,
            offset: cursor as i64,
        });
        cursor = cursor.checked_add(size).ok_or(/* SectionTooLarge */)?;
        object_slices.push(slice);
    }

    // Step 2: build the six section payloads. The Handles payload
    // now consumes our derived `handle_offsets`.
    let header_payload = write_ac1015_header_section(doc)?;
    let classes_payload = write_ac1015_classes_section(doc)?;
    let handles_payload = if handle_offsets.is_empty() {
        Vec::new()
    } else {
        write_ac1015_handle_map_payload(&handle_offsets)?
    };
    // ... obj_free_space / template / aux_header same as M2.T4 ...

    // Step 3: build the descriptor table; offsets are after object_slices.
    let mut descriptors = Vec::with_capacity(6);
    for (record_number, payload) in [...]) {
        descriptors.push(SectionDescriptor {
            ...
            offset: cursor,
            size: payload.len() as u32,
        });
        cursor = cursor.checked_add(payload.len() as u32)?;
    }

    // Step 4: assemble final bytes.
    let prefix = write_ac1015_file_header_prefix(AC1015_KNOWN_SECTION_COUNT)?;
    let directory = write_ac1015_section_locator_directory(&descriptors)?;
    let mut bytes = Vec::with_capacity(cursor as usize);
    bytes.extend_from_slice(&prefix);
    bytes.extend_from_slice(&directory);
    for slice in object_slices { bytes.extend_from_slice(&slice); }
    for payload in [...] { bytes.extend_from_slice(&payload); }
    Ok(bytes)
}

fn encode_entity(entity: &Entity) -> Result<(BitWriter, BitWriter, ObjectHeader), DwgWriteError> {
    match &entity.data {
        EntityData::Line { start, end } => encode_line_entity(entity, *start, *end),
        other => Err(DwgWriteError::Unsupported(format!(
            "entity type {:?} pending F5.M5", other
        ))),
    }
}

fn encode_line_entity(entity: &Entity, start: [f64; 3], end: [f64; 3])
    -> Result<(BitWriter, BitWriter, ObjectHeader), DwgWriteError>
{
    let mut main = BitWriter::new();
    let mut handle = BitWriter::new();
    write_ac1015_entity_common_minimal(EntityCommonMinimal {
        owner_block_handle: entity.owner_handle,
        layer_handle: /* 通过 layer_name 反查 doc.layers — 见 §2.3 */,
        color_index: entity.color_index,
        linetype_scale: entity.linetype_scale,
        lineweight: entity.lineweight,
        invisible: entity.invisible,
    }, &mut main, &mut handle)?;
    write_line_geometry(LineGeometry {
        start, end,
        thickness: entity.thickness,
        extrusion: entity.extrusion,
    }, &mut main)?;
    let main_size_bits = HEADER_BIT_COUNT + main.position_in_bits();
    let header = ObjectHeader {
        object_type: 19, // LINE 的 AC1015 class number
        main_size_bits: u32::try_from(main_size_bits).map_err(/* InvalidValue */)?,
        handle: entity.handle,
        handle_code: HANDLE_CODE_HARD_OWNER,
    };
    Ok((main, handle, header))
}
```

### 2.2 LINE 的 object_type 编号

LINE 的 AC1015 class number 是 `19`（来自 reader 端 `entity_arc.rs` 等 entity 模块的注释或 ACadSharp 表）。需要在 M4.E 实施时验证（先用 19，跑 reader 端 `try_decode_entity_body` 看是否调对 LINE 路径）。

> **决策**：M4.E 首先验证 LINE 的 object_type；如果 19 错，回查 ACadSharp `DwgObjectReader.cs::readEntityType` 或 H7CAD 自身的 `try_decode_entity_body` 路径。

### 2.3 layer_name → layer_handle 反查

`Entity` 结构（`h7cad_native_model::Entity`）只有 `layer_name: String`，没有 `layer_handle`。Writer 需要从 `doc.layers[layer_name].handle` 反查：

```rust
let layer_handle = doc.layers.get(&entity.layer_name)
    .map(|props| props.handle)
    .ok_or_else(|| DwgWriteError::InvalidDocument(format!(
        "entity references unknown layer `{}`", entity.layer_name
    )))?;
```

caller 必须保证 entity.layer_name 在 doc.layers 里有条目；典型 LINE entity 用 `"0"` layer，由 `CadDocument::new()` 默认插入。

---

## 3. 任务拆解

### 3.1 子任务

| ID | 描述 | 文件 |
|---|---|---|
| T1 | 在 `writer/document.rs` 引入 `encode_entity` / `encode_line_entity` helpers | 同文件 |
| T2 | 重写 `write_dwg(doc)` 主体：去掉 `Unsupported` 早返回；按 §2.1 算法装配 | `writer/document.rs` |
| T3 | 把现有 3 个 helper（`ac1015_section_name` / `ac1015_section_record_number`）保留；增加 LINE object_type 常量 | 同上 |
| T4 | `tests/roundtrip_minimal.rs` 增加 `write_dwg_with_single_line_entity_round_trips_through_read_dwg` 集成测试 | 测试文件 |
| T5 | 升级 `write_dwg_rejects_non_empty_entities_with_unsupported` 测试：触发 case 改为 CIRCLE（M5 待办）；LINE 不再触发它 | 同上 |
| T6 | 运行 `cargo test -p h7cad-native-dwg --all-targets` 全绿 | — |
| T7 | `RUSTFLAGS=-Dwarnings cargo check` 干净 | — |
| T8 | `rustfmt --check` + `ReadLints` 通过 | — |
| T9 | facade `dwg_runtime_save_is_unavailable` 测试是否能解锁？**不解锁**——facade 需要等 M4.E 稳定一段后由 M5 / M6 做正式切换。本计划只在 native 层打通 LINE roundtrip。 | — |
| T10 | 更新 CHANGELOG / progress / findings / 父子计划 §6 执行记录 | 四处 |

### 3.2 测试矩阵

| 测试 | 路径 | 验证 |
|---|---|---|
| `write_dwg_with_single_line_entity_round_trips_through_read_dwg` | 1 个 LINE entity | doc → write → read → 回头的 doc.entities[0] 与 input entity 几何字段（start/end/thickness/extrusion）等价；layer_name 经 layer_handle round-trip 后等价；color/linetype_scale/invisible 等 caller-supplied common 字段等价 |
| `write_dwg_with_two_line_entities_uses_distinct_handles` | 2 个 LINE | reader 还原两条 entity；handle_offsets 严格按 handle 升序；offset 严格递增 |
| `write_dwg_rejects_circle_entity_with_unsupported` | 单 CIRCLE | `Unsupported` 错误指向 F5.M5；LINE 不会触发 |
| `write_dwg_with_line_using_unknown_layer_returns_invalid_document` | LINE 引用不在 doc.layers 的 layer_name | `DwgWriteError::InvalidDocument` 错误信息含 layer 名 |
| **既存 4 个 minimal 测试** | 空 doc | 不变；continue green |

### 3.3 既存测试改动

- `write_dwg_rejects_non_empty_entities_with_unsupported`：**重命名为** `write_dwg_rejects_non_line_entity_with_unsupported`（更精确反映 M4.E 后行为），触发 case 改为 CIRCLE。
- 其他 3 个保持不变。

---

## 4. 验收门

- 5 个新增 + 改良集成测试全绿；既存 native-dwg 测试零回归 → 期望 lib 244 + read_headers 53 + real_samples 38 + roundtrip_minimal 8 = **343 全绿**（数量级估计；M4.C/D 落地后再校准）。
- `cargo check --workspace --all-targets`：通过；零新增 warning。
- `RUSTFLAGS=-Dwarnings cargo check -p h7cad-native-dwg --all-targets`：干净。
- 新文件 `rustfmt --edition 2021 --check` 通过；`ReadLints` 零错误。
- facade `dwg_runtime_save_is_unavailable` 测试**保持锁定**（不在本子计划范围解锁）。
- M4.E 收口后，本子计划 §6 与父计划 §2.5 同步标完成；更新主父计划 `2026-05-08-dwg-next-step-plan.md` §F5.M4 状态。

---

## 5. 风险与退路

| 风险 | 触发条件 | 退路 |
|---|---|---|
| **LINE object_type 编号不对** | reader 端 `try_decode_entity_body` 收到 19 但走错 dispatch 分支 | 加 print 看 reader 实际拒绝；查 ACadSharp `DwgObjectReader.cs::readEntityType` 表；如必要从 entity_line.rs 单测里 reverse-engineer |
| **`encode_line_entity` 中 main_size_bits 算错** | `compose_ac1015_object_slice` 的 `main_size_bits == header_bits + main_stream.bits()` 校验红 | 这是 M4.B 设计的捕获点；按错误信息列出的三方 bit count 调试 caller 端累加 |
| **handle_offsets 中 offset 与 reader 期望不一致** | reader 拿 offset 索引 → ObjectStreamCursor 拿不到 slice | 对比 file 的 hex dump 与 reader 在 offset 处期望读到的 MS 字节；多半是 file_header 或 directory 长度算错 |
| **layer_handle 反查失败** | T4 测试中 entity.layer_name 是 "0" 但 `CadDocument::new()` 的 layer "0" 的 handle 是 NULL | 现实情况：`CadDocument::new()` 设置 `LayerProperties::new("0").handle = Handle::NULL`。需要确保测试构造 layer 时手动给 handle 一个非零值，或在 writer 内对 NULL layer_handle 走特殊路径 |
| **物理布局换序导致 reader 端 bug** | 既存样本 reader 走对，但 writer 写出后 reader 不走通 | 对比真 sample_AC1015.dwg 的 directory descriptor offset 是否在 sections 之前/之后；reader 应该不依赖物理顺序，但 M4.E 的实测会 confirm |
| **M4.C `lineweight = -3` 限制** | LINE roundtrip 测试期望 lineweight = -1，实际读到 -3 | 测试断言放宽为「lineweight 字段是 ByDefault」，记入已知 limitation；M5 解锁 |
| **写入 entity 时遗漏 EED skip 终止** | reader 在 `skip_extended_entity_data` 死循环 | M4.C 已在 main 流写 `bit_short(0)` 终结；如复发查 M4.C 的执行记录 |

---

## 6. 时间表

| 段 | 工作 | 估时 |
|---|---|---|
| T1+T2+T3 | encode_entity 链路 + write_dwg 重写 | 90 min |
| T4+T5 | 5 个集成测试 + 调试物理布局 off-by-one | 60–120 min |
| T6+T7+T8 | 验证 + fmt + lint | 15 min |
| T9 | facade 不动（仅文档说明） | — |
| T10 | 文档同步 | 30 min |
| 缓冲 | object_type 反推 / handle_offsets 调试 | 60 min |
| **合计** | | **3.5–5 h** |

---

## 7. 与父子计划的关系

| 计划 | 关系 |
|---|---|
| `2026-05-09-dwg-m4-object-stream-writer-plan.md` §2.5 | 本子计划是其展开；本计划落地后父计划 §2.5 标完成；F5.M4 整体 5/5 完成 |
| `2026-05-08-dwg-next-step-plan.md` §F5.M4 | 父父计划 §F5.M4 整体可标完成；下一步 §F5.M5（21 类 entity body writer）按照 M4.E 已建立的 `encode_entity` 框架平移即可，每类一个 PR |
| `2026-05-09-dwg-m4c-entity-common-minimal-plan.md` §5.1 拆 PR 退路 | 如 M4.C 拆 C.1 + C.2，M4.E 仍然能基于 C.1 起步；未支持的 entity 字段属于 known limitation |

---

## 8. M4.E 收口后的 known limitation 清单

落地后 README / facade 文档需要承诺以下事实：

- native writer 仅支持 **LINE** 一类 entity；其他类型 → `DwgWriteError::Unsupported(... pending F5.M5)`
- entity common header 用 minimal 配置：`owner_handle` / `lineweight` 在 reader 端解码值与 caller 输入无关（known limitation；M5 解锁）
- handles section CRC 是 `[0x00, 0x00]` 占位；reader 不验证（与 handle_map.rs advisory 注释保持一致）
- Header / Classes section 的 16 字节 sentinel 不写出（reader 不强制；ACadSharp 互通留 M5/M6）

facade `save(NativeFormat::Dwg, _)` 仍然返回 `Err("native DWG writer not implemented yet")`；切换由 M5 完成 ≥ 5 个 entity 类型 roundtrip 后再做。

---

## 9. 立即可执行的第一批命令

```powershell
# 起步：先看红
cargo test -p h7cad-native-dwg --test roundtrip_minimal -- --nocapture

# T6 收尾
cargo test -p h7cad-native-dwg --all-targets
$env:RUSTFLAGS='-Dwarnings'; cargo check -p h7cad-native-dwg --all-targets; Remove-Item Env:RUSTFLAGS

# 物理布局调试（如 T4 红）
cargo test -p h7cad-native-dwg --test roundtrip_minimal write_dwg_with_single_line_entity_round_trips_through_read_dwg -- --nocapture
```

---

## 10. 执行记录（落地后填充）

| 日期 | 任务 | 简述 | 验证 |
|---|---|---|---|
| 待填 | T1+T2+T3 | encode_entity + write_dwg 重写 | 待填 |
| 待填 | T4+T5 | 集成测试 + 物理布局 | 待填 |

---

*起草者：H7CAD agent；2026-05-09。*
