# H7CAD F5.M5 — Entity Body Writer 扩张子计划

> **起稿**：2026-05-09
> **父计划**：[`2026-05-08-dwg-next-step-plan.md`](2026-05-08-dwg-next-step-plan.md) §F5.M5
> **依赖**：F5.M4 全部落地（M4.A..E）；`encode_entity` 框架与 `compose_ac1015_object_slice` 已就位
> **里程碑**：F5.M5 —— 把 native DWG writer 从 LINE/CIRCLE/ARC/POINT/LWPOLYLINE/TEXT 扩展到 22 类 entity body 全覆盖
> **目标**：每个 reader 端的 `entity_*.rs` 模块都有对应 `writer/entity_*.rs`，roundtrip 锁在单测里
> **预估**：3–8 周（按 entity 数量与复杂度分批）

---

## 0. TL;DR

M4 把「LINE 单类 entity」打通后，M5.E1–E5 已把 CIRCLE/ARC/POINT/LWPOLYLINE/TEXT 接上同一套 `writer/entity_<kind>.rs` + roundtrip + `encode_entity` dispatch 模板。后续实体按同一模板迭代，每个 PR 解锁 `sample_AC1015.dwg` 中对应 family 的 baseline ratchet。

---

## 1. Reader 端 entity 模块清单

| 序 | reader 模块 | object_type | 字段数 | 复杂度 | sample_AC1015.dwg 频次 | 推荐顺序 |
|---|---|---|---|---|---|---|
| 1 | `entity_circle.rs` | 18 | 4 (3BD/BD/BT/BE) | ★ | 9 | **M5.E1**（已完成） |
| 2 | `entity_arc.rs` | 17 | 6 (3BD/BD/BT/BE/BD/BD) | ★ | 3 | M5.E2（已完成） |
| 3 | `entity_point.rs` | 27 | 5 (3BD/BT/BE/BD x_axis) | ★ | 34 | **M5.E3**（已完成） |
| 4 | `entity_line.rs` | 19* | 9 (z_are_zero shortcut) | ★★ | 82 | **已落地于 M4.D** |
| 5 | `entity_lwpolyline.rs` | 77 | 可变（顶点数组） | ★★★ | ≥ 17 | M5.E4（已完成） |
| 6 | `entity_text.rs` | 1 | 多（含字符串） | ★★★ | 26 | M5.E5（已完成） |
| 7 | `entity_attrib.rs` | 2 | TEXT 超集 + tag/prompt | ★★★ | 低 | M5.E6 |
| 8 | `entity_solid.rs` (含 Face3D) | 31, 22 | 4 顶点 | ★ | 低 | M5.E7 |
| 9 | `entity_ray.rs` (含 Xline) | 40, 41 | 6 (3BD point + 3BD direction) | ★ | 极低 | M5.E8 |
| 10 | `entity_ellipse.rs` | 35 | 8 (3BD center + 3BD major + 3BD ratio + start/end angles) | ★★ | 低 | M5.E9 |
| 11 | `entity_spline.rs` | 36 | 可变（控制点 + knots） | ★★★★ | 低 | M5.E10 |
| 12 | `entity_mtext.rs` | 44 | 可变（含格式化字符串） | ★★★★ | 低 | M5.E11 |
| 13 | `entity_insert.rs` | 7 | INSERT + Block ref + 可选 attrib 链表 | ★★★★ | 中 | M5.E12 |
| 14 | `entity_dimension.rs` | 21..28 | 7 个子类各自 ~10 字段 | ★★★★★ | 0–低 | M5.E13..E19 (7 PR) |
| 15 | `entity_hatch.rs` | 78 | 极复杂（boundary path 嵌套 + pattern） | ★★★★★ | 6 | **M5.E20**（高频但最复杂；计划独立） |
| 16 | `entity_viewport.rs` | 69 | 大量布局字段 | ★★★★ | 6 | M5.E21 |

\* LINE 在 H7CAD 实际采用 object_type 19；CIRCLE 采用 object_type 18；ARC 采用 object_type 17。

---

## 2. 优先级队列

按「value × ease」原则：

### 2.1 第一批（M5.E1..E5，~1 周）

| 段 | entity | 理由 |
|---|---|---|
| M5.E1 | **CIRCLE** | ✅ 已完成：4 字段最简，验证 M4.E 框架可平移；解锁 9 个 sample entities |
| M5.E2 | **ARC** | ✅ 已完成：6 字段，CIRCLE 超集；解锁 3 个 sample entities |
| M5.E3 | **POINT** | ✅ 已完成：5 字段，sample 中 34 个，LINE+POINT+CIRCLE+ARC 已覆盖 sample 中 128 个高频实体 |
| M5.E4 | **LWPOLYLINE** | ✅ 已完成：第一个变长字段 entity，覆盖 vertex 数组、bulge、per-vertex width、closed、constant width |
| M5.E5 | **TEXT** | ✅ 已完成：第一个含字符串 entity，覆盖 Text ASCII、alignment point、rotation/oblique/width factor、alignment flags、style handle |

第一批结束后，sample_AC1015.dwg 中估计 ~158/170 ≈ 93% entities 可被 native writer roundtrip。

### 2.2 第二批（M5.E6..E11，~2 周）

ATTRIB / SOLID / RAY / ELLIPSE / SPLINE / MTEXT —— 中等复杂度，按需求平铺。

| 段 | entity | 状态 |
|---|---|---|
| M5.E6 | **ATTRIB** | ✅ 已完成 |
| M5.E7 | **SOLID + 3DFACE** | ✅ 已完成（2026-05-12，子计划 `2026-05-12-dwg-m5e7-solid-face3d-plan.md`）|
| M5.E8 | **RAY + XLINE** | ✅ 已完成（2026-05-12，共用 writer，wire body 一致仅 object_type 区分）|
| M5.E9 | **ELLIPSE** | ✅ 已完成（2026-05-12，3BD extrusion 非 BE 短路）|
| M5.E10 | **SPLINE** | ✅ 已完成（2026-05-12，scenario 自动选择 + rational weights 校验）|
| M5.E11 | **MTEXT** | ✅ 已完成（2026-05-12，main+handle 双流 + cos/sin/atan2 ε 容差 + STYLE 表 fallback）|

### 2.3 第三批（M5.E12..E21）已开始

| 段 | entity | 状态 |
|---|---|---|
| M5.E12 | **INSERT** | ✅ 已完成（2026-05-12，scale_flag 智能选择 + BLOCK_RECORD 名→handle 解析 + has_attribs force false 已知限制）|
| M5.E13..E19 | **DIMENSION 7 子类** | ⏳ 待执行 |
| M5.E20 | **HATCH** | ✅ 已完成（2026-05-12，3 级嵌套变长 + 4 种 HatchEdge + pattern 最简块；7 单测 + 2 集成全部首次过；子计划 [`2026-05-12-dwg-m5e20-hatch-plan.md`](2026-05-12-dwg-m5e20-hatch-plan.md)）|
| M5.E21 | **VIEWPORT** | ✅ 已完成（2026-05-12，minimal 3 字段；full spec 扩展属后续 reader/model 联合工作）|

### 2.3 第三批（M5.E12..E21，~3–4 周）

INSERT / DIMENSION (7 子类) / HATCH / VIEWPORT —— 高复杂度，每个独立 PR；HATCH 与 DIMENSION 各自独立子计划文件。

---

## 3. 共享流程模板（每个 entity PR 通用）

### 3.1 文件清单

| 文件 | 改动 |
|---|---|
| `crates/h7cad-native-dwg/src/writer/entity_<kind>.rs` | 新建：`pub fn write_<kind>_geometry(geom: <Kind>Geometry, writer: &mut BitWriter) -> Result<(), DwgWriteError>` |
| `crates/h7cad-native-dwg/src/writer/mod.rs` | 加 `pub mod entity_<kind>;` + `pub use entity_<kind>::write_<kind>_geometry;` |
| `crates/h7cad-native-dwg/src/lib.rs` | re-export `write_<kind>_geometry` |
| `crates/h7cad-native-dwg/src/writer/document.rs::encode_entity` | 加 `EntityData::<Kind> {...} => encode_<kind>_entity(...)` 分支 |
| `crates/h7cad-native-dwg/src/writer/document.rs` | 加 `fn encode_<kind>_entity(entity: &Entity, ...) -> ...` helper |
| `crates/h7cad-native-dwg/tests/roundtrip_minimal.rs` | 加 `write_dwg_with_<kind>_entity_round_trips_through_read_dwg` 集成测试 |

### 3.2 单 PR 任务模板

| ID | 描述 |
|---|---|
| Tx.1 | 镜像 reader 端 `read_<kind>_geometry` 字段顺序，写 `write_<kind>_geometry` |
| Tx.2 | 在 `writer/entity_<kind>.rs` 内 `#[cfg(test)] mod tests` 加 3–5 个 roundtrip 单测 |
| Tx.3 | `encode_entity` dispatch arm |
| Tx.4 | `tests/roundtrip_minimal.rs` 集成测试：构造单 `<Kind>` entity → write_dwg → read_dwg → 字段等价 |
| Tx.5 | re-export |
| Tx.6 | `cargo test -p h7cad-native-dwg --all-targets` 全绿 |
| Tx.7 | `RUSTFLAGS=-Dwarnings cargo check` 干净 |
| Tx.8 | fmt + lint |
| Tx.9 | CHANGELOG / progress / findings 同步 |

每 PR 估时：

| 复杂度 | 估时 |
|---|---|
| ★ (CIRCLE / POINT / SOLID / RAY) | 1–1.5h |
| ★★ (ARC / ELLIPSE) | 2h |
| ★★★ (LWPOLYLINE / TEXT / ATTRIB) | 3–4h |
| ★★★★ (SPLINE / MTEXT / INSERT) | 5–8h |
| ★★★★★ (DIMENSION / HATCH / VIEWPORT) | 1–3 天每个，独立子计划 |

### 3.3 已知字段限制（M5 阶段不解锁）

每个 entity PR 携带 M4.C minimal common header 的 known limitation：
- `owner_handle = NULL`（reader 解码值与 caller 输入无关）
- `lineweight = -3` ByDefault（与 caller 输入无关）

这些限制在 M6 切换 facade 时由 M5 末尾增加 `write_ac1015_entity_common_full` 一并解决。

---

## 4. M5.E1 — CIRCLE writer 详细计划（已完成）

### 4.1 reader 端字段时序

来源：`crates/h7cad-native-dwg/src/entity_circle.rs::read_circle_geometry`：

| 序 | reader 调用 | M5.E1 writer 调用 |
|---|---|---|
| 1 | `read_3bit_double()` → `center: [f64; 3]` | `write_3bit_double(center)` |
| 2 | `read_bit_double()` → `radius: f64` | `write_bit_double(radius)` |
| 3 | `read_bit_thickness_r2000_plus()` → `thickness` | `write_bit_thickness_r2000_plus(thickness)` |
| 4 | `read_bit_extrusion_r2000_plus()` → `extrusion` | `write_bit_extrusion_r2000_plus(extrusion)` |

BitWriter 已有所有 4 个 primitive（M1 30 单测覆盖）；直接调用即可。

### 4.2 接口

```rust
// crates/h7cad-native-dwg/src/writer/entity_circle.rs

use crate::entity_circle::CircleGeometry;
use crate::bit_writer::BitWriter;
use crate::DwgWriteError;

pub fn write_circle_geometry(
    geom: CircleGeometry,
    writer: &mut BitWriter,
) -> Result<(), DwgWriteError> {
    writer.write_3bit_double(geom.center)?;
    writer.write_bit_double(geom.radius)?;
    writer.write_bit_thickness_r2000_plus(geom.thickness)?;
    writer.write_bit_extrusion_r2000_plus(geom.extrusion)?;
    Ok(())
}
```

### 4.3 测试矩阵（写在 `writer/entity_circle.rs` 内部）

| 测试 | 验证 |
|---|---|
| `circle_geometry_round_trips_origin_unit_circle` | center=(0,0,0), radius=1.0, thickness=0, extrusion=[0,0,1] (全紧凑路径) |
| `circle_geometry_round_trips_translated_circle` | center=(10, 20, 5), radius=2.5 |
| `circle_geometry_round_trips_nontrivial_thickness_extrusion` | thickness=1.0, extrusion=[1,1,1] |
| `circle_geometry_round_trips_negative_radius` | radius=-3.0（DWG 允许；reader 不验证） |
| `circle_geometry_round_trips_via_compose_and_split` | 完整 LINE-style：write → compose with M4 framework → split → read，verify all 4 fields |

### 4.4 `encode_entity` dispatch arm

```rust
EntityData::Circle { center, radius } => {
    let mut main = BitWriter::new();
    let mut handle = BitWriter::new();
    write_ac1015_entity_common_minimal(/* same as LINE */, &mut main, &mut handle)?;
    write_circle_geometry(CircleGeometry {
        center: *center,
        radius: *radius,
        thickness: entity.thickness,
        extrusion: entity.extrusion,
    }, &mut main)?;
    let main_size_bits = HEADER_BIT_COUNT + main.position_in_bits();
    let header = ObjectHeader {
        object_type: 18, // CIRCLE
        main_size_bits: u32::try_from(main_size_bits).map_err(...)?,
        handle: entity.handle,
        handle_code: HANDLE_CODE_HARD_OWNER,
    };
    Ok((main, handle, header))
}
```

### 4.5 估时与风险

- T1+T2: 1h
- T3+T4: 30 min
- T5+T6+T7+T8: 30 min
- T9: 30 min
- 缓冲：30 min（验证 CIRCLE 的 object_type 是 18 而不是 19）
- **合计：2.5–3h**

风险：
- CIRCLE object_type 反推：见 §4.4 placeholder 18；M4.E 时已验证 LINE = 19，CIRCLE 应该 = 18。如错改回正确值。
- 其他风险与 M4.D 共享。

---

## 5. M5 后续 entity（M5.E6..E21）入口指针

每个 entity PR 落地后，下一个 PR 直接复制已完成模板：简单实体参考 M5.E1/M5.E3，变长实体参考 M5.E4，字符串实体参考 M5.E5。

| entity | reader 文件 | 字段对照来源 |
|---|---|---|
| ATTRIB | `entity_attrib.rs::read_attrib_geometry` | TEXT 超集，含 tag/prompt 与 field length |
| ... | ... | ... |

每个 PR 的具体字段对照表在那个 PR 落地时就地填入；不预先全部铺开（避免计划过早老化）。

---

## 6. 验收门（每个 entity PR 共享）

- 新单测全绿；既存测试零回归。
- `cargo test -p h7cad-native-dwg --all-targets` 全绿。
- `cargo check --workspace --all-targets` 通过；零新增 warning。
- 新文件 `rustfmt --edition 2021 --check` + `ReadLints` 零错误。
- facade `dwg_runtime_save_is_unavailable` 测试**保持锁定**直到 M6。
- `tests/roundtrip_minimal.rs` 至少有 1 个集成测试覆盖该 entity 的 write_dwg roundtrip。

---

## 7. M5 完成的退出条件

- 22 类 entity（含 DIMENSION 7 子类与 RAY/XLINE 共用 module）每类至少 1 个 roundtrip 集成测试。
- `sample_AC1015.dwg` 经 reader → CadDocument → write_dwg → read_dwg → 与原始 doc 在所有 entity types 上语义等价（这是 M5 → M6 的承接前提）。
- `crates/h7cad-native-dwg/tests/real_samples.rs` 新增 `sample_AC1015_round_trips_via_native_writer` 测试，对 sample_AC1015 跑完整 read→write→read 循环并断言 entity_count + family_count 守恒。

---

## 8. 风险与退路

| 风险 | 触发 | 退路 |
|---|---|---|
| 某 entity 的 reader 端 default 行为难反推 | 单测 RED | 跳过该 entity 进下一个；该 entity 拉单独子计划文件深挖 |
| LWPOLYLINE / SPLINE / HATCH 的可变长度字段写入复杂 | 编码 vertex 数组困难 | 拆 PR：先实现「空 vertex 数组」case，再增量加 vertex 写入 |
| DIMENSION 7 子类共享 reader 不易拆分 | encode_entity dispatch 太长 | DIMENSION 单独写一份 sub-plan，按子类拆 PR |
| HATCH 复杂度爆表 | 实施超出 5 天 | 留作 M5 末尾收口；如不可行降级为 `Unsupported` 直到 M6 后做专项 |
| `lineweight` known limitation 让用户感知 | 真实工程文件圆角丢失 | 先解锁 `write_ac1015_entity_common_full` 反查表（在 M5 中段插一个独立 PR） |

---

## 9. 与父计划的关系

| 父计划条目 | 关系 |
|---|---|
| `2026-05-08-dwg-next-step-plan.md` §F5.M5 | 本子计划是其全展开 |
| `2026-05-09-dwg-m4-object-stream-writer-plan.md` §2.5 M4.E | M4.E 完成后 M5.E1 直接接续；`encode_entity` 框架是 M5 入口 |
| 主父计划 `2026-04-30-h7cad-next-development-plan.md` | M5 完成意味着 native DWG writer 可承担生产 LINE/CIRCLE/ARC/etc. 写入；与 acadrust 移除路线对接 |

---

## 10. 执行节奏

| 周 | 工作 |
|---|---|
| Week 1 | M5.E1 (CIRCLE) ✅ → M5.E2 (ARC) ✅ → M5.E3 (POINT) ✅ → M5.E4 (LWPOLYLINE) ✅ → M5.E5 (TEXT) ✅ |
| Week 2 | M5.E6 (ATTRIB) ✅ → M5.E7 (SOLID/Face3D) ✅ → M5.E8 (RAY/XLINE) ✅ → M5.E9 (ELLIPSE) ✅ → M5.E10 (SPLINE) ✅ → M5.E11 (MTEXT) ✅ |
| Week 3 | M5.E12 (INSERT) ✅ → M5.E13..E19 (DIMENSION 7 子类) |
| Week 4 | M5.E20 (HATCH，独立子计划) ✅ |
| Week 5 | M5.E21 (VIEWPORT) ✅ → M5 收口；准备 M6 facade 切换 |

整体 M5 估时：**3–8 周**（视 HATCH / DIMENSION 实际复杂度）。

---

## 11. M5.E1..E5 基线命令

```powershell
# CIRCLE writer 单测
cargo test -p h7cad-native-dwg --lib writer::entity_circle -- --nocapture

# CIRCLE 集成测试
cargo test -p h7cad-native-dwg --test roundtrip_minimal write_dwg_with_single_circle_entity_round_trips_through_read_dwg -- --nocapture

# ARC writer 单测
cargo test -p h7cad-native-dwg --lib writer::entity_arc -- --nocapture

# ARC 集成测试
cargo test -p h7cad-native-dwg --test roundtrip_minimal write_dwg_with_single_arc_entity_round_trips_through_read_dwg -- --nocapture

# 收尾
cargo test -p h7cad-native-dwg --all-targets
$env:RUSTFLAGS='-Dwarnings'; cargo check -p h7cad-native-dwg --all-targets; Remove-Item Env:RUSTFLAGS
```

---

## 12. 执行记录（落地后填充）

| 日期 | 子段 | 简述 | 验证 |
|---|---|---|---|
| 2026-05-09 | M5.E1 (CIRCLE) | `writer/entity_circle.rs::write_circle_geometry` + `encode_entity` CIRCLE dispatch + single CIRCLE `write_dwg -> read_dwg` roundtrip；unsupported case 改用 ARC | `cargo test -p h7cad-native-dwg --lib writer::entity_circle -- --nocapture`；`cargo test -p h7cad-native-dwg --test roundtrip_minimal -- --nocapture`；`cargo test -p h7cad-native-dwg --all-targets` |
| 2026-05-09 | M5.E2 (ARC) | `writer/entity_arc.rs::write_arc_geometry` + `encode_entity` ARC dispatch + single ARC `write_dwg -> read_dwg` roundtrip；unsupported case 改用 UNKNOWN | `cargo test -p h7cad-native-dwg --lib writer::entity_arc -- --nocapture`；`cargo test -p h7cad-native-dwg --test roundtrip_minimal -- --nocapture`；`cargo test -p h7cad-native-dwg --all-targets` |
| 2026-05-09 | M5.E3 (POINT) | `writer/entity_point.rs::write_point_geometry` + `encode_entity` POINT dispatch + single POINT `write_dwg -> read_dwg` roundtrip；native model 暂以 `x_axis_angle = 0.0` 写出 | `cargo test -p h7cad-native-dwg --lib writer::entity_point -- --nocapture`；`cargo test -p h7cad-native-dwg --test roundtrip_minimal -- --nocapture`；`cargo test -p h7cad-native-dwg --all-targets` |
| 2026-05-09 | M5.E4 (LWPOLYLINE) | `writer/entity_lwpolyline.rs::write_lwpolyline_geometry` + `encode_entity` LWPOLYLINE dispatch + single LWPOLYLINE `write_dwg -> read_dwg` roundtrip；native model 暂以 `elevation = 0.0` 写出 | `cargo test -p h7cad-native-dwg --lib writer::entity_lwpolyline -- --nocapture`；`cargo test -p h7cad-native-dwg --test roundtrip_minimal -- --nocapture`；`cargo test -p h7cad-native-dwg --all-targets` |
| 2026-05-09 | M5.E5 (TEXT) | `writer/entity_text.rs::write_text_geometry` + `encode_entity` TEXT dispatch + single TEXT `write_dwg -> read_dwg` roundtrip；STYLE table 仍未写出，读回样式名按 style handle fallback 表示 | `cargo test -p h7cad-native-dwg --lib writer::entity_text -- --nocapture`；`cargo test -p h7cad-native-dwg --test roundtrip_minimal -- --nocapture`；`cargo test -p h7cad-native-dwg --all-targets` |
| 2026-05-12 | M5.E7 (SOLID + 3DFACE) | `writer/entity_solid.rs::write_solid_geometry`（共享 elevation 强约束）+ `writer/entity_face3d.rs::write_face3d_geometry`（`has_no_flags=0` 恒定路径）+ `encode_entity` 两条 dispatch + 两个 `encode_*_entity` helper + `AC1015_OBJECT_TYPE_SOLID=31` / `AC1015_OBJECT_TYPE_FACE3D=28`；6 writer-internal 单测 + 2 roundtrip_minimal 集成测试；子计划 `2026-05-12-dwg-m5e7-solid-face3d-plan.md` | `cargo test -p h7cad-native-dwg --lib writer::entity_solid -- --nocapture`（3/3 绿）；`cargo test -p h7cad-native-dwg --lib writer::entity_face3d -- --nocapture`（3/3 绿）；`cargo test -p h7cad-native-dwg --test roundtrip_minimal -- --nocapture`（14/14 绿）；`cargo test -p h7cad-native-dwg --all-targets`（lib 263 + read_headers 53 + real_samples 38 + roundtrip_minimal 14 = 368 全绿）；`$env:RUSTFLAGS='-Dwarnings'; cargo check -p h7cad-native-dwg --all-targets` 干净 |
| 2026-05-12 | M5.E8 (RAY + XLINE) | `writer/entity_ray.rs::write_ray_geometry`（单一函数覆盖两个 object_type，因 wire body 完全一致）+ `encode_entity` 两条 dispatch + `encode_ray_like_entity` helper（`EntityKind::{Ray,XLine}` 内部枚举区分 object_type 与 type_name）+ `AC1015_OBJECT_TYPE_RAY=38` / `AC1015_OBJECT_TYPE_XLINE=40`；3 writer-internal 单测 + 2 roundtrip_minimal 集成测试（RAY 与 XLINE 各一） | `cargo test -p h7cad-native-dwg --lib writer::entity_ray -- --nocapture`（3/3 绿）；`cargo test -p h7cad-native-dwg --test roundtrip_minimal -- --nocapture`（16/16 绿）；`cargo test -p h7cad-native-dwg --all-targets`（lib 266 + read_headers 53 + real_samples 38 + roundtrip_minimal 16 = 373 全绿）；`$env:RUSTFLAGS='-Dwarnings'; cargo check -p h7cad-native-dwg --all-targets` 干净 |
| 2026-05-12 | M5.E9 (ELLIPSE) | `writer/entity_ellipse.rs::write_ellipse_geometry`（6 字段：3 × 3BD + 3 × BD；extrusion 走 3BD 而非 BE 短路）+ `encode_entity` dispatch + `encode_ellipse_entity` helper（extrusion 从 `entity.extrusion` 共享 common header）+ `AC1015_OBJECT_TYPE_ELLIPSE=35`；3 writer-internal 单测 + 1 roundtrip_minimal 集成测试 | `cargo test -p h7cad-native-dwg --lib writer::entity_ellipse -- --nocapture`（3/3 绿）；`cargo test -p h7cad-native-dwg --test roundtrip_minimal -- --nocapture`（17/17 绿）；`cargo test -p h7cad-native-dwg --all-targets`（lib 269 + read_headers 53 + real_samples 38 + roundtrip_minimal 17 = 377 全绿）；`$env:RUSTFLAGS='-Dwarnings'; cargo check -p h7cad-native-dwg --all-targets` 干净 |
| 2026-05-12 | M5.E10 (SPLINE) | `writer/entity_spline.rs::write_spline_geometry`（变长 + scenario 分支：scenario=2 当 fit-point 数据非默认，否则 scenario=1；rational 推自 weights 非空；reader 丢弃字段 fit/knot/control tolerance + periodic 一律写 0）+ `encode_entity` dispatch + `encode_spline_entity` helper + `AC1015_OBJECT_TYPE_SPLINE=36`；5 writer-internal 单测（4 round-trip 路径 + 1 weights/control_points 长度不一致 reject）+ 1 roundtrip_minimal 集成测试 | `cargo test -p h7cad-native-dwg --lib writer::entity_spline -- --nocapture`（5/5 绿）；`cargo test -p h7cad-native-dwg --test roundtrip_minimal -- --nocapture`（18/18 绿）；`cargo test -p h7cad-native-dwg --all-targets`（lib 274 + read_headers 53 + real_samples 38 + roundtrip_minimal 18 = 383 全绿）；`$env:RUSTFLAGS='-Dwarnings'; cargo check -p h7cad-native-dwg --all-targets` 干净 |
| 2026-05-12 | M5.E11 (MTEXT) | `writer/entity_mtext.rs::write_mtext_geometry`（13 字段，首个双流 main+handle 写入；reader 丢弃字段 ext_height/ext_width/line_spacing_style/unknown_bit 一律写 0；`x_direction = [cos(rotation), sin(rotation), 0]` 反推 → reader `atan2` 还原 rotation，cos/sin/atan2 链路非 bit-exact，约 ε ≤ 1e-12；rectangle_height: Option<f64> ↔ wire BD rect_height 用 `unwrap_or(0.0)` + reader `> 0.0 ? Some : None`）+ `encode_entity` dispatch + `encode_mtext_entity` helper（style 解析链：text_styles.get(style_name) → fallback Standard → Handle::NULL）+ `AC1015_OBJECT_TYPE_MTEXT=44`；3 writer-internal 单测（canonical 0 + 非零 rotation ε 容差 + 空字符串边界）+ 1 roundtrip_minimal 集成测试（rotation=0，style_name 走 `$STYLE_<HEX>` fallback 与 M5.E5 TEXT 已知限制同质） | `cargo test -p h7cad-native-dwg --lib writer::entity_mtext -- --nocapture`（3/3 绿）；`cargo test -p h7cad-native-dwg --test roundtrip_minimal -- --nocapture`（19/19 绿）；`cargo test -p h7cad-native-dwg --all-targets`（lib 277 + read_headers 53 + real_samples 38 + roundtrip_minimal 19 = 387 全绿）；`$env:RUSTFLAGS='-Dwarnings'; cargo check -p h7cad-native-dwg --all-targets` 干净 |
| 2026-05-12 | M5.E12 (INSERT) | `writer/entity_insert.rs::write_insert_geometry`（双流 main+handle；scale_flag 智能选择：unit→`01` / 等比→`10` / 异质→`00` 走 RD+DD+DD；reader 永不读 first/last/seqend，故 writer force has_attribs=false）+ `encode_entity` dispatch（解构 block_name/insertion/scale/rotation，忽略 has_attribs/attribs）+ `encode_insert_entity` helper（block_records.values().find by name → handle）+ `AC1015_OBJECT_TYPE_INSERT=7`；5 writer-internal 单测（unit scale + single + DD + DD-mid-row default + writer-forced has_attribs=false 限制断言）+ 1 roundtrip_minimal 集成测试（DD scale 路径，block_name `$BLOCK_<HEX>` fallback） | `cargo test -p h7cad-native-dwg --lib writer::entity_insert -- --nocapture`（5/5 绿）；`cargo test -p h7cad-native-dwg --test roundtrip_minimal -- --nocapture`（20/20 绿）；`cargo test -p h7cad-native-dwg --all-targets`（lib 282 + read_headers 53 + real_samples 38 + roundtrip_minimal 20 = 393 全绿）；`$env:RUSTFLAGS='-Dwarnings'; cargo check -p h7cad-native-dwg --all-targets` 干净 |
| 2026-05-12 | M5.E21 (VIEWPORT) | `writer/entity_viewport.rs::write_viewport_geometry`（minimal 3 字段镜像 reader：3BD center + BD width + BD height；full VIEWPORT spec 的 view direction / twist / lens length / frozen layers 等字段 reader 一律 skip，writer 同步）+ `encode_entity` dispatch + `encode_viewport_entity` helper + `AC1015_OBJECT_TYPE_VIEWPORT=34`；3 writer-internal 单测（原点+单位 / 偏移矩形 / 退化零尺寸）+ 1 roundtrip_minimal 集成测试 | `cargo test -p h7cad-native-dwg --lib writer::entity_viewport -- --nocapture`（3/3 绿）；`cargo test -p h7cad-native-dwg --test roundtrip_minimal -- --nocapture`（21/21 绿）；`cargo test -p h7cad-native-dwg --all-targets`（lib 285 + read_headers 53 + real_samples 38 + roundtrip_minimal 21 = 397 全绿）；`$env:RUSTFLAGS='-Dwarnings'; cargo check -p h7cad-native-dwg --all-targets` 干净 |
| 2026-05-12 | M5.E20 (HATCH) | `writer/entity_hatch.rs::write_hatch_geometry`（M5 最复杂 entity；3 级嵌套变长：boundary_paths → edges → 4 种 HatchEdge variant；polyline path flag-2 强约束：必须含 HatchEdge::Polyline；!solid_fill 写最简 pattern 块 angle=0 / scale=1 / num_lines=0；reader 丢弃字段 elevation/is_associative/style/pattern_type/pattern_*/seeds 一律写 0；boundary_handle_count 永远写 0，HATCH 与源实体关联不保留）+ `encode_entity` dispatch + `encode_hatch_entity` helper（extrusion 从 entity.extrusion 共享 common header）+ `AC1015_OBJECT_TYPE_HATCH=78`；7 writer-internal 单测（empty solid / Line / CircularArc + is_ccw 双向 / EllipticArc / polyline+bulge / pattern 块 / polyline-flag 缺 Polyline edge 拒绝）+ 2 roundtrip_minimal 集成测试（solid_fill=true 含 4-Line 矩形边界 / solid_fill=false 含 CircularArc 边界）；子计划 `2026-05-12-dwg-m5e20-hatch-plan.md` 全展开；**所有 9 个测试首次运行就全过，零调试** | `cargo test -p h7cad-native-dwg --lib writer::entity_hatch -- --nocapture`（7/7 绿）；`cargo test -p h7cad-native-dwg --test roundtrip_minimal -- --nocapture`（23/23 绿）；`cargo test -p h7cad-native-dwg --all-targets`（lib 292 + read_headers 53 + real_samples 38 + roundtrip_minimal 23 = 406 全绿）；`$env:RUSTFLAGS='-Dwarnings'; cargo check -p h7cad-native-dwg --all-targets` 干净 |
| ... | ... | ... | ... |

---

*起草者：H7CAD agent；2026-05-09。*
