# H7CAD F5.M5.E20 — HATCH Entity Body Writer 子计划

> **起稿**：2026-05-12
> **父计划**：[`2026-05-09-dwg-m5-entity-body-writers-plan.md`](2026-05-09-dwg-m5-entity-body-writers-plan.md) §2.3 / §1 row 15
> **依赖**：F5.M4 全部落地；M5.E1..E12 + M5.E21 已完成（CIRCLE / ARC / POINT / LWPOLYLINE / TEXT / ATTRIB / SOLID / 3DFACE / RAY / XLINE / ELLIPSE / SPLINE / MTEXT / INSERT / VIEWPORT writer）；`encode_entity` dispatch 框架及 `compose_entity_object_slice` helper 在位；`HatchBoundaryPath` 与 `HatchEdge` 模型稳定
> **里程碑**：F5.M5.E20 —— 在 native DWG writer 中加入 **HATCH (object_type=78)**，sample_AC1015.dwg 中第二高频复杂实体（6 个实例），是 facade 切换前最重要的实体覆盖缺口
> **目标**：打通 `EntityData::Hatch` 路径的 `write_dwg → read_dwg` round-trip。该片下后，sample_AC1015.dwg 中 HATCH family 可被 writer 处理而非 `Unsupported` 错误
> **预估**：2–3 天（★★★★★ 复杂度，与 DIMENSION 子类同级）

---

## 0. TL;DR

HATCH 是 M5 阶段最复杂的 entity writer。它有：

1. **嵌套变长结构**：`boundary_paths: Vec<HatchBoundaryPath>`，每个 path 有 `edges: Vec<HatchEdge>`，每条 edge 又是 4 种类型的 enum。
2. **可选 pattern 块**：`solid_fill == false` 时拖一大段 pattern 定义（angle / scale / lines / dashes）；模型不携带这部分，writer 必须发出 reader 能跑过去的最简形态。
3. **双流写入**：boundary_handle 引用通过 `handle_writer` 走，需要在主流 path 后回填 handle 数量。
4. **reader 丢弃字段多达 8 个**：`elevation` / `is_associative` / `style` / `pattern_type` / `pattern_angle` / `pattern_scale` / `is_double` / `pattern lines`，writer 一律写默认值。
5. **reader 不识别 Spline edge**：模型的 `HatchEdge` 不含 Spline 变体，writer 也无需发出 edge_type=4。

策略：**分段实现**。E20a 不带 pattern（solid_fill=true）打通核心 boundary path / edges 框架；E20b 加 pattern 块支持 solid_fill=false。两段共用 sub-plan，但分两个 PR 提交。

---

## 1. Reader 端字段对照表

### 1.1 主流（main_reader）

| 序 | reader 调用 | writer 调用 | model 来源 |
|---|---|---|---|
| 1 | `read_bit_double()` → `_elevation` | `write_bit_double(0.0)` | 丢弃；写 0 |
| 2 | `read_3bit_double()` → `extrusion` | `write_3bit_double(entity.extrusion)` | common header |
| 3 | `read_text_ascii()` → `pattern_name` | `write_text_ascii(&geom.pattern_name)` | EntityData::Hatch |
| 4 | `read_bit()` → `solid_fill` | `write_bit(if solid_fill { 1 } else { 0 })` | EntityData::Hatch |
| 5 | `read_bit()` → `_is_associative` | `write_bit(0)` | 丢弃；写 0 |
| 6 | `read_bit_long()` → `num_paths` | `write_bit_long(boundary_paths.len() as i32)` | EntityData::Hatch |
| 7 | per path: `read_boundary_path` | 对应 `write_boundary_path` | EntityData::Hatch |
| 8 | `read_bit_short()` → `_style` | `write_bit_short(0)` | 丢弃 |
| 9 | `read_bit_short()` → `_pattern_type` | `write_bit_short(0)` | 丢弃 |
| 10 | (if !solid_fill) pattern 块 | (if !solid_fill) 写最简 pattern：angle=0 / scale=1 / is_double=0 / num_lines=0 | 模型不携带 |
| 11 | (if has_derived) `read_bit_double()` | (if any path has derived flag) `write_bit_double(0.0)` | 模型无此字段，writer 不主动 set；除非 path.flags bit 2 设了，否则忽略 |
| 12 | `read_bit_long()` → `num_seeds` | `write_bit_long(0)` | 模型不携带，写 0 |

### 1.2 boundary path（per path）

| 序 | 字段 | writer 实现 |
|---|---|---|
| 1 | `BL flags` | `write_bit_long(path.flags)` |
| 2 | if `flags & 2 != 0`（polyline）：`B has_bulge / B closed / BL num_vertices / per vertex 2RD + (BD bulge if has_bulge)` | 第一条 edge 必须是 `HatchEdge::Polyline`，否则 writer 错误。`has_bulge` 推导自 `vertices.iter().any(\|v\| v[2] != 0.0)` |
| 3 | else（非 polyline）：`BL num_edges / per edge (1 byte type + payload)` | 遍历 `path.edges` 写 |
| 4 | `BL boundary_handle_count` | 写 0（writer 不维护 boundary handle 链） |

### 1.3 boundary path 中 edge type 写入

| type | edge variant | writer payload |
|---|---|---|
| 1 | `Line { start, end }` | 2 × 2RD |
| 2 | `CircularArc { center, radius, start_angle, end_angle, is_ccw }` | 2RD center + BD radius + BD start + BD end + B is_ccw |
| 3 | `EllipticArc { center, major_endpoint, minor_ratio, start_angle, end_angle, is_ccw }` | 2RD center + 2RD major + BD minor_ratio + BD start + BD end + B is_ccw |
| 4 | Spline | **writer 不发出**：模型 `HatchEdge` 无 Spline 变体；遇到时是逻辑错误 |
| `Polyline` | 在 polyline path 路径下处理，不在 type 编号 | 见 §1.2 row 2 |

### 1.4 handle 流

| 序 | reader | writer |
|---|---|---|
| 1 | `for _ in 0..boundary_handle_total { read_handle_relative(object_handle.value()) }` | 写 0 个 handle（因为 boundary_handle_count 写 0） |

---

## 2. 接口

### 2.1 `crates/h7cad-native-dwg/src/writer/entity_hatch.rs`

```rust
//! AC1015 HATCH entity body writer (F5.M5.E20).

use crate::bit_writer::BitWriter;
use crate::entity_hatch::HatchGeometry;
use crate::DwgWriteError;
use h7cad_native_model::{HatchBoundaryPath, HatchEdge};

pub fn write_hatch_geometry(
    geom: &HatchGeometry,
    main_writer: &mut BitWriter,
    _handle_writer: &mut BitWriter,
) -> Result<(), DwgWriteError> {
    main_writer.write_bit_double(0.0)?; // elevation discarded
    main_writer.write_3bit_double(geom.extrusion)?;
    main_writer.write_text_ascii(&geom.pattern_name)?;
    main_writer.write_bit(if geom.solid_fill { 1 } else { 0 })?;
    main_writer.write_bit(0)?; // is_associative discarded
    main_writer.write_bit_long(geom.boundary_paths.len() as i32)?;

    for path in &geom.boundary_paths {
        write_boundary_path(path, main_writer)?;
    }

    main_writer.write_bit_short(0)?; // style
    main_writer.write_bit_short(0)?; // pattern_type

    if !geom.solid_fill {
        // M5.E20 minimal pattern: angle=0, scale=1, is_double=0, num_lines=0
        main_writer.write_bit_double(0.0)?;
        main_writer.write_bit_double(1.0)?;
        main_writer.write_bit(0)?;
        main_writer.write_bit_short(0)?;
    }

    // M5.E20 known limitation: num_seeds = 0
    main_writer.write_bit_long(0)?;
    Ok(())
}

fn write_boundary_path(
    path: &HatchBoundaryPath,
    main_writer: &mut BitWriter,
) -> Result<(), DwgWriteError> {
    main_writer.write_bit_long(path.flags)?;
    let is_polyline = (path.flags & 2) != 0;
    if is_polyline {
        let polyline = path.edges.iter().find_map(|edge| match edge {
            HatchEdge::Polyline { closed, vertices } => Some((closed, vertices)),
            _ => None,
        });
        let (closed, vertices) = polyline.ok_or_else(|| DwgWriteError::InvalidValue(
            "HATCH polyline path must contain exactly one HatchEdge::Polyline".to_string()
        ))?;
        let has_bulge = vertices.iter().any(|v| v[2] != 0.0);
        main_writer.write_bit(if has_bulge { 1 } else { 0 })?;
        main_writer.write_bit(if *closed { 1 } else { 0 })?;
        main_writer.write_bit_long(vertices.len() as i32)?;
        for v in vertices {
            main_writer.write_raw_f64_le(v[0])?;
            main_writer.write_raw_f64_le(v[1])?;
            if has_bulge {
                main_writer.write_bit_double(v[2])?;
            }
        }
    } else {
        main_writer.write_bit_long(path.edges.len() as i32)?;
        for edge in &path.edges {
            write_hatch_edge(edge, main_writer)?;
        }
    }
    // boundary_handle_count = 0
    main_writer.write_bit_long(0)?;
    Ok(())
}

fn write_hatch_edge(
    edge: &HatchEdge,
    main_writer: &mut BitWriter,
) -> Result<(), DwgWriteError> {
    match edge {
        HatchEdge::Line { start, end } => {
            main_writer.write_raw_u8(1)?;
            main_writer.write_raw_f64_le(start[0])?;
            main_writer.write_raw_f64_le(start[1])?;
            main_writer.write_raw_f64_le(end[0])?;
            main_writer.write_raw_f64_le(end[1])?;
        }
        HatchEdge::CircularArc { center, radius, start_angle, end_angle, is_ccw } => {
            main_writer.write_raw_u8(2)?;
            main_writer.write_raw_f64_le(center[0])?;
            main_writer.write_raw_f64_le(center[1])?;
            main_writer.write_bit_double(*radius)?;
            main_writer.write_bit_double(*start_angle)?;
            main_writer.write_bit_double(*end_angle)?;
            main_writer.write_bit(if *is_ccw { 1 } else { 0 })?;
        }
        HatchEdge::EllipticArc { center, major_endpoint, minor_ratio, start_angle, end_angle, is_ccw } => {
            main_writer.write_raw_u8(3)?;
            main_writer.write_raw_f64_le(center[0])?;
            main_writer.write_raw_f64_le(center[1])?;
            main_writer.write_raw_f64_le(major_endpoint[0])?;
            main_writer.write_raw_f64_le(major_endpoint[1])?;
            main_writer.write_bit_double(*minor_ratio)?;
            main_writer.write_bit_double(*start_angle)?;
            main_writer.write_bit_double(*end_angle)?;
            main_writer.write_bit(if *is_ccw { 1 } else { 0 })?;
        }
        HatchEdge::Polyline { .. } => {
            return Err(DwgWriteError::InvalidValue(
                "HatchEdge::Polyline must appear only in polyline-flag boundary paths".to_string()
            ));
        }
    }
    Ok(())
}
```

### 2.2 `writer/document.rs` 接入

```rust
EntityData::Hatch { pattern_name, solid_fill, boundary_paths } => {
    encode_hatch_entity(doc, entity, pattern_name.clone(), *solid_fill, boundary_paths.clone())
}
```

`encode_hatch_entity` helper：

```rust
fn encode_hatch_entity(
    doc: &CadDocument,
    entity: &Entity,
    pattern_name: String,
    solid_fill: bool,
    boundary_paths: Vec<HatchBoundaryPath>,
) -> Result<Vec<u8>, DwgWriteError> {
    let mut main = BitWriter::new();
    let mut handle = BitWriter::new();
    write_common_entity_header(doc, entity, &mut main, &mut handle)?;
    write_hatch_geometry(
        &HatchGeometry { pattern_name, solid_fill, boundary_paths, extrusion: entity.extrusion },
        &mut main,
        &mut handle,
    )?;
    compose_entity_object_slice(entity, AC1015_OBJECT_TYPE_HATCH, "HATCH", &main, &handle)
}
```

`const AC1015_OBJECT_TYPE_HATCH: i16 = 78;`

---

## 3. 任务拆解

### 3.1 第一段：E20a — solid fill 与边类型 framework

| ID | 描述 | 文件 |
|---|---|---|
| T1 | `writer/entity_hatch.rs` 新建 + `write_hatch_geometry`、`write_boundary_path`、`write_hatch_edge` 实现 | 该文件 |
| T2 | `writer/mod.rs` + `lib.rs` 加模块声明与 re-export | 2 文件 |
| T3 | `writer/document.rs` 加 dispatch + `encode_hatch_entity` + `AC1015_OBJECT_TYPE_HATCH` | `document.rs` |
| T4 | 6 个 writer-internal 单测（solid_fill=true / 空 boundary / Line edge / CircularArc / EllipticArc / Polyline path） | `entity_hatch.rs` 内 `mod tests` |
| T5 | 1 个 `tests/roundtrip_minimal.rs` 集成测试：单 HATCH（solid_fill=true，1 boundary path，2 Line edges） | 该文件 |

### 3.2 第二段：E20b — pattern 块（!solid_fill）

| ID | 描述 |
|---|---|
| T6 | `write_hatch_geometry` 的 !solid_fill 分支已在 E20a 写入最简 pattern；E20b 增加 1 个 writer-internal 单测覆盖 |
| T7 | `tests/roundtrip_minimal.rs` 加 1 个 solid_fill=false 集成测试 |

### 3.3 收尾

| ID | 描述 |
|---|---|
| T8 | `cargo test -p h7cad-native-dwg --all-targets` 全绿 |
| T9 | `RUSTFLAGS=-Dwarnings cargo check -p h7cad-native-dwg --all-targets` 干净 |
| T10 | `rustfmt --edition 2021` 通过；`ReadLints` 零错 |
| T11 | `CHANGELOG.md` 加 2026-MM-DD F5.M5.E20 条目；父计划 §2.3 / §10 / §12 同步 |

---

## 4. 测试矩阵

### 4.1 `writer/entity_hatch.rs` 单测

| 测试名 | 验证 |
|---|---|
| `hatch_round_trips_empty_solid` | solid_fill=true，0 boundary paths：与 reader `hatch_geometry_decodes_empty_solid_payload` 镜像 |
| `hatch_round_trips_single_line_boundary` | 1 boundary path（flags=0），1 Line edge |
| `hatch_round_trips_circular_arc_boundary` | 1 boundary path，1 CircularArc edge（is_ccw=true 与 false 两种） |
| `hatch_round_trips_elliptic_arc_boundary` | 1 boundary path，1 EllipticArc edge |
| `hatch_round_trips_polyline_path` | flags=2（polyline），1 HatchEdge::Polyline，含 bulge 与 closed |
| `hatch_round_trips_pattern_block` | solid_fill=false，验证 reader 能跑通 pattern 块 |

### 4.2 `tests/roundtrip_minimal.rs` 集成测试

| 测试名 | 验证 |
|---|---|
| `write_dwg_with_single_solid_hatch_round_trips_through_read_dwg` | solid_fill=true，1 boundary path with 2 Line edges；pattern_name / extrusion / boundary_paths / solid_fill 全部 round-trip |
| `write_dwg_with_single_pattern_hatch_round_trips_through_read_dwg` | solid_fill=false（pattern 块路径） |

### 4.3 既存测试不变

- M2 的 4 个 empty-doc 测试不动。
- 现有 19+ entity roundtrip 测试不动。
- `write_dwg_rejects_*_with_unsupported`：本轮后「下一个未支持」是 DIMENSION（7 子类），如果当前用 UNKNOWN 占位则不动；如已经是 HATCH 则改为下一个未实现的（如 POLYLINE）。

---

## 5. 验收门

- 6 个新 writer 单测 + 2 个新集成测试全绿；既存 native-dwg 测试零回归。
- `cargo check -p h7cad-native-dwg --all-targets`：通过，零新增 warning。
- `RUSTFLAGS=-Dwarnings cargo check -p h7cad-native-dwg --all-targets`：干净。
- 新文件 `rustfmt --edition 2021 --check` 通过；`ReadLints` 零错。
- facade `dwg_runtime_save_is_unavailable` **保持锁定**（不在本子计划范围内解锁）。
- 父计划 §2.3 标记 M5.E20 完成；§10 Week 4 行打勾；§12 执行记录追加 M5.E20 行。

---

## 6. 风险与退路

| 风险 | 触发 | 退路 |
|---|---|---|
| **HatchEdge::Polyline 误放在非 polyline boundary** | 调用方 `path.flags & 2 == 0` 但 `edges` 包含 Polyline variant | writer 在 `write_hatch_edge` 抛 `DwgWriteError::InvalidValue`，提示约束 |
| **polyline boundary 没有 Polyline edge** | `path.flags & 2 != 0` 但 `edges` 不含 Polyline | writer 在 `write_boundary_path` 抛 `DwgWriteError::InvalidValue` |
| **多个 Polyline edge 在同一 boundary** | 调用方误把 polyline boundary 切成多个 vertex 组 | writer 用 `find_map` 取第一个；剩余忽略；不报错。后续 fidelity 增强可改为 reject |
| **boundary_handle_count 写 0 与真实 sample 不一致** | 真实 sample 可能在 boundary_handle 流中关联其他实体 | M5.E20 范围内不复刻关联 handle；reader 端 boundary_handle_total=0 时 handle 流不读，roundtrip 自洽。AutoCAD 端可能感知缺失但当前 H7CAD 不依赖 |
| **pattern 块字段全 0 在 AutoCAD 端被视为空 pattern** | solid_fill=false 但 num_lines=0 | M5.E20 已知限制：模型不携带 pattern 定义，writer 只能写最简形态。后续 fidelity 提升需先在 model 加 pattern 字段 |
| **spline edge 出现在调用方输入** | 上游解析其他格式（如 DXF）产生了 Spline edge | model 的 `HatchEdge` 没有 Spline 变体，编译期不允许；如未来加入，writer 要么 reject 要么 lossy approximation |
| **boundary_paths 与 sample bit-by-bit 不一致** | reader 端某些 sample 用紧凑变体 | M5.E20 范围只承诺 read→write→read 自闭等价，不承诺与 sample 字节对等 |

---

## 7. 时间表

| 段 | 工作 | 估时 |
|---|---|---|
| T1+T2+T3 | 三个文件 + dispatch + helper | 1.5–2.5 h |
| T4 | 6 个 writer-internal 单测 | 1.5–2 h |
| T5 | 2 个集成测试 | 0.5–1 h |
| T8+T9+T10 | cargo test / check / fmt / lint | 0.5–1 h |
| T11 | CHANGELOG + 父计划 §2.3/§10/§12 + 本计划 §11 执行记录 | 0.5 h |
| 缓冲 | edge_type 字段 endian / bit 偏移 debug、polyline boundary 边界情况 | 1–2 h |
| **合计** | | **5.5–9 h** |

预估调高至 **2 天**，给反复调通边界条件留余量。

---

## 8. 与父子计划的关系

| 计划 | 关系 |
|---|---|
| `2026-05-09-dwg-m5-entity-body-writers-plan.md` §2.3 / §1 row 15 | 本子计划是 M5.E20 的全展开 |
| `2026-05-12-104359-h7cad-dwg-2026q2-roadmap.md` §3.1 W5 | Q2 6 周路线图 Week 5 主线交付项 |
| `2026-05-08-dwg-next-step-plan.md` §F5.M5 | M5 总进度推进；M5.E20 后剩 DIMENSION 7 子类 |
| `2026-04-25-svg-hatch-pattern-lines-plan.md` 等 SVG/HATCH 相关计划 | 仅渲染相关，本子计划不耦合 |

---

## 9. M5.E20 落地后的 known limitation

- **boundary_handle_count 永远为 0**：writer 不保留 hatch boundary 关联的源实体 handles。AutoCAD 端对 hatch 关联性的依赖（如 hover 时高亮 source entity）不支持。
- **pattern 定义不可 round-trip**：solid_fill=false 时 writer 写最简 pattern（angle=0 / scale=1 / num_lines=0），AutoCAD 端可能渲染为空 pattern。完整 pattern fidelity 需 model 先扩字段。
- **num_seeds 永远为 0**：模型不携带 seed points；writer 写 0。
- **Spline edge 不支持**：模型 enum 无此 variant。
- **elevation 永远写 0**：reader 不读回该字段；模型不携带。
- entity common minimal 差别（`owner_handle`/`lineweight`）与 M5.E1…E19 / E21 同质。

---

## 10. 立即可执行的命令（实现完成后跑）

```powershell
# T4 跑 HATCH writer 单测
cargo test -p h7cad-native-dwg --lib writer::entity_hatch -- --nocapture

# T5 跑集成测试
cargo test -p h7cad-native-dwg --test roundtrip_minimal write_dwg_with_single_solid_hatch -- --nocapture
cargo test -p h7cad-native-dwg --test roundtrip_minimal write_dwg_with_single_pattern_hatch -- --nocapture

# T8 全绿
cargo test -p h7cad-native-dwg --all-targets

# T9 warning gate
$env:RUSTFLAGS='-Dwarnings'; cargo check -p h7cad-native-dwg --all-targets; Remove-Item Env:RUSTFLAGS
```

---

## 11. 执行记录（落地后填充）

| 日期 | 任务 | 简述 | 验证 |
|---|---|---|---|
| 2026-05-12 | T1+T2+T3 | 新建 `writer/entity_hatch.rs::write_hatch_geometry` + `write_boundary_path` + `write_hatch_edge`（含 polyline-flag 强约束 + 4 种 HatchEdge 编码 + !solid_fill 最简 pattern 块）；`writer/mod.rs` + `lib.rs` re-export `write_hatch_geometry`；`writer/document.rs` 加 `EntityData::Hatch` dispatch + `encode_hatch_entity` helper + `AC1015_OBJECT_TYPE_HATCH=78` | `ReadLints` 零错；`cargo check -p h7cad-native-dwg --all-targets` 干净 |
| 2026-05-12 | T4 | 7 个 writer-internal 单测：empty solid / Line edge / CircularArc + is_ccw 双向 / EllipticArc / polyline+bulge / !solid_fill pattern 块 / polyline-flag 缺 Polyline edge 拒绝 | `cargo test -p h7cad-native-dwg --lib writer::entity_hatch -- --nocapture` 7/7 首次运行就全过 |
| 2026-05-12 | T5 | 2 个集成测试 in `tests/roundtrip_minimal.rs`：solid_fill=true 含 4-Line 闭合矩形 boundary / solid_fill=false 含 CircularArc edge | `cargo test -p h7cad-native-dwg --test roundtrip_minimal -- --nocapture` 23/23 绿 |
| 2026-05-12 | T8+T9+T10 | 全 crate cargo test + warning gate + rustfmt + lint | `cargo test -p h7cad-native-dwg --all-targets`：lib 292 + read_headers 53 + real_samples 38 + roundtrip_minimal 23 = **406 全绿**；`$env:RUSTFLAGS='-Dwarnings'; cargo check -p h7cad-native-dwg --all-targets` 零警告 |
| 2026-05-12 | T11 | `CHANGELOG.md` 顶部加 2026-05-12 M5.E20 条目；父计划 `2026-05-09-dwg-m5-entity-body-writers-plan.md` §2.3 表标 M5.E20 ✅、§10 Week 4 ✅、§12 执行记录补 M5.E20 行；本子计划 §11 执行记录填齐 | grep 验证 |

**亮点**：本里程碑所有 9 个测试（7 writer-internal + 2 集成）**首次运行就全过零调试**，证明 §1 字段对照表 + §2 接口示例代码两段的准确性。实际工时 ≈ 30 分钟，远低于 §7 中预估的 5.5–9h；这与同会话内 M5.E7\u2013E12 + E21 + E20 共 8 个 entity writer 复用同一套接线模板的累积效应一致。

---

*起草者：H7CAD agent；2026-05-12。M5.E20 是 M5 阶段最复杂的 entity writer 之一，建议独立会话推进。*
