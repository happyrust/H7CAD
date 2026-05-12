# H7CAD F5.M5.E7 — SOLID + 3DFACE Entity Body Writer 子计划

> **起稿**:2026-05-12
> **父计划**:[`2026-05-09-dwg-m5-entity-body-writers-plan.md`](2026-05-09-dwg-m5-entity-body-writers-plan.md) §2.2 / §1 row 8
> **依赖**:F5.M4 全部落地;M5.E1…E6 已完成 (LINE / CIRCLE / ARC / POINT / LWPOLYLINE / TEXT / ATTRIB writer);`encode_entity` dispatch 框架及 `compose_entity_object_slice` helper 在位
> **里程碑**:F5.M5.E7 —— 在 native DWG writer 中加入 **SOLID (object_type=31)** 与 **3DFACE (object_type=28)** 两个 4-顶点面类 entity
> **目标**:打通 `EntityData::Solid` 和 `EntityData::Face3D` 两个路径的 `write_dwg → read_dwg` round-trip。该片下后,`encode_entity` 不再对这两类 entity 报 Unsupported。
> **预估**:1.5–2.5 h(★ 复杂度,与 M5.E1 CIRCLE 同级)

---

## 0. TL;DR

两个新 writer 文件镜像已有的 reader:

1. `writer/entity_solid.rs::write_solid_geometry` → 镜像 `read_solid_geometry` 的 9-field 位流(`BT thickness / BD elevation / 4 × 2RD corner_xy / BE extrusion`)。
2. `writer/entity_face3d.rs::write_face3d_geometry` → 镜像 `read_face3d_geometry` 的 6-field 位流(`B has_no_flags / [BS invisible_edges] / 4 × 3BD corner`)。
3. `writer/document.rs` 增加 `EntityData::Solid {…} → encode_solid_entity` 和 `EntityData::Face3D {…} → encode_face3d_entity` 两条 dispatch。
4. 两类 entity 各加 3 个 writer-internal roundtrip 单测 + 1 个 `tests/roundtrip_minimal.rs` 集成测试。

---

## 1. Reader 端字段对照表

### 1.1 SOLID (object_type = `SOLID_OBJECT_TYPE = 31`)

来源:`crates/h7cad-native-dwg/src/entity_solid.rs::read_solid_geometry`

| 序 | reader 调用 | M5.E7 writer 调用 | 备注 |
|---|---|---|---|
| 1 | `read_bit_thickness_r2000_plus()` → `thickness: f64` | `write_bit_thickness_r2000_plus(thickness)` | BT (紧凑 0/RD) |
| 2 | `read_bit_double()` → `elevation: f64` | `write_bit_double(elevation)` | BD:4 顶点共享 z |
| 3 | `read_2raw_double()` → `c1: [f64; 2]` | `write_raw_f64_le(c1[0]); write_raw_f64_le(c1[1])` | 2RD corner1 |
| 4 | 同上 c2 | 同上 c2 | |
| 5 | 同上 c3 | 同上 c3 | |
| 6 | 同上 c4 | 同上 c4 | |
| 7 | `read_bit_extrusion_r2000_plus()` → `extrusion: [f64; 3]` | `write_bit_extrusion_r2000_plus(extrusion)` | BE(默认 OCS Z+ 走紧凑路径) |

返回 `SolidGeometry { corners: [[x,y,elevation]; 4], thickness, extrusion }`。

### 1.2 3DFACE (object_type = `FACE3D_OBJECT_TYPE = 28`)

来源:`entity_solid.rs::read_face3d_geometry`

| 序 | reader 调用 | M5.E7 writer 调用 | 备注 |
|---|---|---|---|
| 1 | `read_bit()` → `has_no_flags: u8` | `write_bit(has_no_flags)` | 是否省略 flags 单比特 |
| 2 | `if has_no_flags == 0 { read_bit_short() }` → `invisible_edges: i16` | 同条件 `write_bit_short(invisible_edges)` | BS;否则跳过 |
| 3 | `read_3bit_double()` → `c1: [f64; 3]` | `write_3bit_double(c1)` | 3BD corner1 |
| 4 | 同上 c2 | 同上 c2 | |
| 5 | 同上 c3 | 同上 c3 | |
| 6 | 同上 c4 | 同上 c4 | |

返回 `Face3DGeometry { corners: [c1, c2, c3, c4], invisible_edges }`。

> **`has_no_flags` 决策**:writer 默认取 `has_no_flags = 0`(写出 flags),并始终写 `invisible_edges`(含 `0`)。这使 writer 路径可预测、与 M5.E1…E6 的风格一致。如未来 reader 依赖某些 sample 是 `has_no_flags = 1` 的 case,本子计划不处理(其源头路径仍由 reader 处理)。

---

## 2. 接口

### 2.1 `crates/h7cad-native-dwg/src/writer/entity_solid.rs`

```rust
//! AC1015 SOLID entity body writer (F5.M5.E7).
//!
//! Mirror of `entity_solid::read_solid_geometry`. The 4 corners are
//! encoded as XY pairs sharing a single BD elevation; callers MUST
//! provide `corners` with a consistent z (typically derived from
//! `EntityData::Solid` whose underlying 3D points already share their
//! z-component).

use crate::bit_writer::BitWriter;
use crate::entity_solid::SolidGeometry;
use crate::DwgWriteError;

pub fn write_solid_geometry(
    geom: SolidGeometry,
    writer: &mut BitWriter,
) -> Result<(), DwgWriteError> {
    writer.write_bit_thickness_r2000_plus(geom.thickness)?;
    let elevation = geom.corners[0][2];
    // Enforce shared elevation invariant: the SOLID wire format cannot
    // round-trip 4 distinct z values; mismatched input is a caller bug.
    for c in &geom.corners[1..] {
        if c[2] != elevation {
            return Err(DwgWriteError::InvalidValue(format!(
                "SOLID corners must share elevation; got z={} vs z={}",
                elevation, c[2]
            )));
        }
    }
    writer.write_bit_double(elevation)?;
    for c in &geom.corners {
        writer.write_raw_f64_le(c[0])?;
        writer.write_raw_f64_le(c[1])?;
    }
    writer.write_bit_extrusion_r2000_plus(geom.extrusion)?;
    Ok(())
}
```

### 2.2 `crates/h7cad-native-dwg/src/writer/entity_face3d.rs`

```rust
//! AC1015 3DFACE entity body writer (F5.M5.E7).

use crate::bit_writer::BitWriter;
use crate::entity_solid::Face3DGeometry;
use crate::DwgWriteError;

pub fn write_face3d_geometry(
    geom: Face3DGeometry,
    writer: &mut BitWriter,
) -> Result<(), DwgWriteError> {
    // Always emit `has_no_flags = 0` so the BS invisible_edges round-trips.
    writer.write_bit(0)?;
    writer.write_bit_short(geom.invisible_edges)?;
    for c in &geom.corners {
        writer.write_3bit_double(*c)?;
    }
    Ok(())
}
```

### 2.3 `writer/mod.rs` + `lib.rs`

加 `pub mod entity_solid; pub mod entity_face3d;` 与 re-export `write_solid_geometry / write_face3d_geometry`。不修改已有 const `SOLID_OBJECT_TYPE` / `FACE3D_OBJECT_TYPE`(它们是 i16 const,writer 直接复用)。

### 2.4 `writer/document.rs::encode_entity` dispatch 补 2 条

```rust
EntityData::Solid { corners, normal, thickness } => {
    encode_solid_entity(doc, entity, *corners, *normal, *thickness)
}
EntityData::Face3D { corners, invisible_edges } => {
    encode_face3d_entity(doc, entity, *corners, *invisible_edges)
}
```

增加两个 helper 函数:

```rust
fn encode_solid_entity(
    doc: &CadDocument,
    entity: &Entity,
    corners: [[f64; 3]; 4],
    normal: [f64; 3],
    thickness: f64,
) -> Result<Vec<u8>, DwgWriteError> {
    let mut main = BitWriter::new();
    let mut handle = BitWriter::new();
    write_common_entity_header(doc, entity, &mut main, &mut handle)?;
    write_solid_geometry(
        SolidGeometry { corners, thickness, extrusion: normal },
        &mut main,
    )?;
    compose_entity_object_slice(entity, AC1015_OBJECT_TYPE_SOLID, "SOLID", &main, &handle)
}

fn encode_face3d_entity(
    doc: &CadDocument,
    entity: &Entity,
    corners: [[f64; 3]; 4],
    invisible_edges: i16,
) -> Result<Vec<u8>, DwgWriteError> {
    let mut main = BitWriter::new();
    let mut handle = BitWriter::new();
    write_common_entity_header(doc, entity, &mut main, &mut handle)?;
    write_face3d_geometry(
        Face3DGeometry { corners, invisible_edges },
        &mut main,
    )?;
    compose_entity_object_slice(entity, AC1015_OBJECT_TYPE_FACE3D, "3DFACE", &main, &handle)
}
```

在 `document.rs` 顶部增加两个 `const AC1015_OBJECT_TYPE_SOLID: i16 = 31;` / `AC1015_OBJECT_TYPE_FACE3D: i16 = 28;`(与 reader 端 `lib.rs:296-307` 保持同步)。

---

## 3. 任务拆解

| ID | 描述 | 文件 |
|---|---|---|
| T1 | `writer/entity_solid.rs` 新建 + `write_solid_geometry` 实现 | 该文件 |
| T2 | `writer/entity_face3d.rs` 新建 + `write_face3d_geometry` 实现 | 该文件 |
| T3 | `writer/mod.rs` + `lib.rs` 加模块声明与 re-export | 2 文件 |
| T4 | `writer/document.rs` 加 2 条 dispatch + 2 个 helper + 2 个 object_type const | `document.rs` |
| T5 | 两个 writer 模块各加 3 个 roundtrip 单测(6 个) | 两文件 `#[cfg(test)] mod tests` |
| T6 | `tests/roundtrip_minimal.rs` 加 2 个集成测试 | 该文件 |
| T7 | `cargo test -p h7cad-native-dwg --all-targets` 全绿 | — |
| T8 | `RUSTFLAGS=-Dwarnings cargo check -p h7cad-native-dwg --all-targets` 干净 | — |
| T9 | `rustfmt --edition 2021` 两个新文件 + `ReadLints` 零错 | — |
| T10 | `CHANGELOG.md` 加 2026-05-12 F5.M5.E7 条目 + 父计划 §2.2 与 §12 执行记录补 | 3 文件 |

---

## 4. 测试矩阵

### 4.1 `writer/entity_solid.rs` 单测

| 测试名 | 验证 |
|---|---|
| `solid_geometry_round_trips_unit_quad` | 4 顶点 (0,0,0)/(1,0,0)/(1,1,0)/(0,1,0),thickness=0, extrusion=[0,0,1] |
| `solid_geometry_round_trips_translated_nonzero_elevation` | 均 z=5.0,thickness=1.0,extrusion=[1,0,0] |
| `solid_geometry_rejects_mismatched_corner_elevations` | 三个角 z=0, 一个 z=1 → `DwgWriteError::InvalidValue`,错误信息含两个 z |

### 4.2 `writer/entity_face3d.rs` 单测

| 测试名 | 验证 |
|---|---|
| `face3d_geometry_round_trips_planar_quad` | invisible_edges=0,4 个 3BD 角点 z=0 |
| `face3d_geometry_round_trips_nonplanar_with_invisible_edges` | invisible_edges=0b1010 (边 1、3 隐藏),4 个 3D 角点 z 不同 |
| `face3d_geometry_round_trips_zero_invisible_edges` | invisible_edges=0,确保 writer 仍写出 BS 位(`has_no_flags=0` 路径) |

两个单测都走 "write → BitReader → read → 断言等价",参考 `entity_attrib.rs::round_trip` 模型。

### 4.3 `tests/roundtrip_minimal.rs` 集成测试

| 测试名 | 验证 |
|---|---|
| `write_dwg_with_single_solid_entity_round_trips_through_read_dwg` | 1 个 SOLID entity:`doc → write_dwg → read_dwg → 名称="SOLID"`;corners / thickness / extrusion 等价;layer/handle 等价;color_index 等价 |
| `write_dwg_with_single_face3d_entity_round_trips_through_read_dwg` | 1 个 3DFACE entity:4 个 3D corner / invisible_edges 等价;类型名为 "3DFACE" |

### 4.4 既存测试不变

- `write_dwg_with_single_<line/circle/arc/point/lwpolyline/text/attrib>_*` 都不动。
- M2 的 4 个 empty-doc 测试不动。
- `write_dwg_rejects_*_with_unsupported`:原本用某个"下一个未支持"作为触发。本轮后"下一个未支持"倘若仍然是 SOLID/FACE3D,需改为更后类型(推荐 `RAY` 或 `XLine`,为 M5.E8 占位)。如该测试现在已经是 RAY,则不动。

---

## 5. 验收门

- 6 个新 writer 单测 + 2 个新集成测试全绿;既存 native-dwg 测试零回归。
- `cargo check --workspace --all-targets`:通过,零新增 warning。
- `RUSTFLAGS=-Dwarnings cargo check -p h7cad-native-dwg --all-targets`:干净。
- 新文件 `rustfmt --edition 2021 --check` 通过;`ReadLints` 零错。
- facade `dwg_runtime_save_is_unavailable` **保持锁定**(不在本子计划范围内解锁)。
- `CHANGELOG.md` / `progress.md` / `findings.md` 各增一条 2026-05-12 条目;父计划 `2026-05-09-dwg-m5-entity-body-writers-plan.md` §2.2 标记 M5.E7 完成,§12 补执行记录行。

---

## 6. 风险与退路

| 风险 | 触发条件 | 退路 |
|---|---|---|
| **3DFACE `has_no_flags` 选 0 与 sample 不一致** | reader 能读但不代表与 ACadSharp / sample_AC1015 实际布局等价 | M5.E7 范围内只负责 "write → 本项目 reader read → 等价";sample 反跳留给 M5 末 sample_AC1015 roundtrip 套件 |
| **SOLID elevation 不一致成为真实场景** | 上游调用者计算出不同 z 的 4 角 | T1 中 `InvalidValue` 报错是设计内含捕获点;错误信息里需明确提示 "SOLID corners must share elevation" |
| **`EntityData::Solid` 的 normal 与 SOLID 的 BE extrusion 语义是否同一** | reader 中 `extrusion` 字段在 entity_common 上下文被语义化为 OCS normal | 检查 reader 路径 + `try_decode_entity_body` 是否将 `extrusion` 赋值给 `EntityData::Solid.normal`;本子计划 T4 helper 拼 `extrusion = normal`,如 reader 那边是另外一个字段赋值错位,退到 "在 reader 侧补映射 commit" 后再接 M5.E7 writer |
| **invisible_edges 负数** | DWG BS 是 i16,reader 原生返回 i16 | writer 直接调 `write_bit_short(i16)`,无额外处理 |
| **与 M4.C minimal common header 的交互** | `owner_handle`/`lineweight` 由 common minimal 控制,roundtrip 后该二者不能期望跟 caller 一致 | 集成测试只断言 geometry / layer / color / handle / type,不断言 owner_handle / lineweight;与 M5.E1…E6 一致 |

---

## 7. 时间表

| 段 | 工作 | 估时 |
|---|---|---|
| T1+T2+T3 | 两个 writer module + mod.rs/lib.rs re-export | 30–40 min |
| T4 | document.rs dispatch + 2 helper + 2 const | 20–30 min |
| T5 | 6 个 writer-internal 单测 | 20–30 min |
| T6 | 2 个 roundtrip_minimal 集成测试 | 20–30 min |
| T7+T8+T9 | cargo test / check / fmt / lint | 10–15 min |
| T10 | CHANGELOG / progress / findings / 父计划 §2.2 + §12 | 15–20 min |
| 缓冲 | SOLID extrusion 语义反推与 debug | 15–30 min |
| **合计** | | **1.75–2.7 h** |

---

## 8. 与父子计划的关系

| 计划 | 关系 |
|---|---|
| `2026-05-09-dwg-m5-entity-body-writers-plan.md` §2.2 / §1 row 8 | 本子计划是 M5.E7 的全展开;落地后§2.2 第二批 ×2/6 完成 |
| `2026-05-08-dwg-next-step-plan.md` §F5.M5 | M5 总进度由 6/22 跟进为 8/22(SOLID + 3DFACE) |
| `2026-04-09-dwg-native-port-plan.md` | F5 里程碑进度同步更新 |

---

## 9. M5.E7 落地后的 known limitation

- 3DFACE writer 始终走 `has_no_flags = 0` 路径:与部分 sample 物理字节不同,但 reader 可读。
- SOLID writer 要求 4 角 z 均一;调用者传入不一致 z 会遇到 `DwgWriteError::InvalidValue`。
- entity common minimal 差别(`owner_handle`/`lineweight`)与 M5.E1…E6 同质,不在本子计划范围内解锁。

---

## 10. 立即可执行的第一批命令

```powershell
# T7 起步全验
cargo test -p h7cad-native-dwg --all-targets

# T5 单独跑 SOLID/3DFACE writer 单测
cargo test -p h7cad-native-dwg --lib writer::entity_solid -- --nocapture
cargo test -p h7cad-native-dwg --lib writer::entity_face3d -- --nocapture

# T6 集成测试
cargo test -p h7cad-native-dwg --test roundtrip_minimal write_dwg_with_single_solid_entity -- --nocapture
cargo test -p h7cad-native-dwg --test roundtrip_minimal write_dwg_with_single_face3d_entity -- --nocapture

# T8 warning gate
$env:RUSTFLAGS='-Dwarnings'; cargo check -p h7cad-native-dwg --all-targets; Remove-Item Env:RUSTFLAGS
```

---

## 11. 执行记录(落地后填充)

| 日期 | 任务 | 简述 | 验证 |
|---|---|---|---|
| 2026-05-12 | T1+T2 | 新建 `writer/entity_solid.rs::write_solid_geometry`（含共享 elevation 强约束 + `DwgWriteError::InvalidValue` 显式错误信息）与 `writer/entity_face3d.rs::write_face3d_geometry`（`has_no_flags=0` 恒定路径，BS invisible_edges 始终落盘） | `ReadLints` 零错；`cargo check -p h7cad-native-dwg` 干净 |
| 2026-05-12 | T3+T4 | `writer/mod.rs` 加 `entity_solid` / `entity_face3d` 模块声明与 re-export；`lib.rs` 顶层 re-export `write_solid_geometry` / `write_face3d_geometry`；`writer/document.rs` 加 `EntityData::Solid` / `Face3D` 两条 dispatch + `encode_solid_entity` / `encode_face3d_entity` helper + `AC1015_OBJECT_TYPE_SOLID = 31` / `AC1015_OBJECT_TYPE_FACE3D = 28` 两个 const | `cargo check -p h7cad-native-dwg --all-targets` 干净 |
| 2026-05-12 | T5+T6 | 两个 writer 模块各 3 个 `#[cfg(test)] mod tests` 单测（共 6/6）+ `tests/roundtrip_minimal.rs` 加 `write_dwg_with_single_solid_entity_round_trips_through_read_dwg` 与 `write_dwg_with_single_face3d_entity_round_trips_through_read_dwg`（共 2/2） | 单测：`cargo test -p h7cad-native-dwg --lib writer::entity_solid -- --nocapture` 3/3；`cargo test -p h7cad-native-dwg --lib writer::entity_face3d -- --nocapture` 3/3；集成：`cargo test -p h7cad-native-dwg --test roundtrip_minimal -- --nocapture` 14/14 |
| 2026-05-12 | T7+T8+T9 | 全 crate 测试 + warning gate + format/lint | `cargo test -p h7cad-native-dwg --all-targets`：lib 263 + read_headers 53 + real_samples 38 + roundtrip_minimal 14 = **368 全绿**；`$env:RUSTFLAGS='-Dwarnings'; cargo check -p h7cad-native-dwg --all-targets`：零错误零警告；`ReadLints` 零错 |
| 2026-05-12 | T10 | `CHANGELOG.md` 加「2026-05-12：AC1015 SOLID + 3DFACE entity body writers (F5.M5.E7)」条目；父计划 `2026-05-09-dwg-m5-entity-body-writers-plan.md` §2.2 第二批表标记 M5.E6 ✅ M5.E7 ✅；§10 Week 2 行标记 M5.E7 ✅；§12 执行记录追加 M5.E7 行 | grep 验证三文件均出现「M5.E7」字样 |

---

*起草者:H7CAD agent;2026-05-12。*
