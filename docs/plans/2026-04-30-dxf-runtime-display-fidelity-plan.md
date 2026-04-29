# DXF 运行时显示保真收口计划

> **日期**: 2026-04-30
> **范围**: H7CAD 运行时显示 / 编辑层关于 DXF 真源的收口；不再涉及 DXF 文件层（read/write 已基本闭环），不涉及 DWG 红灯（见 `2026-04-22-post-dimalt-roadmap.md` Path C）。
> **前置**: `2026-04-24-dxf-2d-display-closure-plan.md` 已闭合（Task 1–6 全绿）；`write_dxf_strict` 已落地；`crates/h7cad-native-dxf` 174+ roundtrip 全绿。
> **驱动**: `INTEGRATION_GAPS.md` 列出的 acadrust 集成 gap + `ROADMAP.md` 命令缺口 + `2026-04-17-acadrust-removal-plan.md` 的 B5 残留。

---

## 1. 立题

DXF **文件层** 已经闭环：
- `h7cad-native-dxf::read_dxf_bytes` / `write_dxf_strict` 双向 roundtrip 锁定 30+ 实体
- `h7cad-native-model::CadDocument` 是单一真源
- `native_bridge::native_doc_to_acadrust` 默认显示投影路径已通过 7 个 `fixture_*` + 4 个 `dualstore_*` + 2 个 `e2e_*` 锁定

但 **运行时层** 还有显著的"打开任意真实 DXF 都能正确显示+编辑"缺口。本计划把这些缺口分 4 个 Phase 落地，每个 Phase 自成一个可独立合并的 PR 集合。

---

## 2. 现状盘点（2026-04-30）

| 维度 | 已完成 | 待完成（本计划范围） |
|---|---|---|
| Reader 实体类型覆盖 | 40+ EntityData 变体 | OLE2FRAME / MPOLYGON / GEOPOSITIONMARKER / MATERIAL / FIELD 落入 `EntityData::Unknown` |
| Writer | `write_dxf_strict` + 兼容 `write_dxf_string` | Binary DXF 未实现（Phase 4 长尾，本计划不做） |
| Native → compat bridge 投影 | 35 类实体投影完整 | Helix / 6 种 Surface / Light / Camera / Section / ProxyEntity 返回 `None` |
| 运行时编辑（acadrust 移除） | B1 / B2 / B3 / B5a 已完成 | B5b–B5g 未做（66 处 `cargo check --no-default-features` trait bound 错） |
| 显示保真 — systemic | 单层 INSERT ByBlock 链 ✓ | OCS→WCS 任意轴变换缺；嵌套块 ByBlock 链不递归 |
| 显示保真 — 字段 | layer/color/lineweight common 字段 ✓ | LWPolyline width / Polyline width / Spline weights+closed / MINSERT 阵列 / Dimension DIMSCALE 系列 / Hatch spline 边 / RasterImage clip_boundary / Polyline3D vertex flags |
| 显示保真 — 文本 | TEXT/MTEXT 基础渲染 ✓ | TextStyle is_backward / is_upside_down 不应用；CXF 外 Unicode 静默丢弃；复杂线型嵌入文本未渲染 |
| Snap | 36/41 实体有 snap_pts | INSERT Insertion / 嵌套块内 / Hatch 边界 / Dimension defpoint / Spline 控制点 / MultiLeader & MLine 顶点 |
| Reader bug 修复 | Dimension rotation/text_rotation ✓ | AttributeEntity / AttributeDefinition rotation 未修（仍以度被当弧度用）|

---

## 3. Phase 划分

| Phase | 主题 | 周期 | PR 数 | 用户感知 | 风险 |
|---|---|---:|---:|---|---|
| **P0** | systemic 显示偏差（OCS→WCS / 嵌套 ByBlock 链）| 3-4 天 | 2 | 高（3D-DXF 立刻不再错位）| 中（涉及所有 entity tessellation） |
| **P1** | 字段宽度 + 高频显示字段（width / weights / DIMSCALE / clip / spline-flag）| 1.5 周 | 6 | 高（用户最常吐槽的"粗多段线变零宽"等）| 低（每条独立） |
| **P2** | bridge 补 6 类未投影实体 + 修 reader bug 长尾 | 1 周 | 7 | 中（少见但打开就消失）| 低 |
| **P3** | acadrust 移除 B5b–B5g | 2-3 周 | 6 子批 | 低（架构）| 中-高（命令面广）|

P0 & P1 可串行；P2 可与 P1 后半段并行；P3 在 P0/P1/P2 完成后启动。

---

## 4. Phase 0 — Systemic 显示偏差（最高优先）

### 4.1 OCS→WCS 任意轴变换补全

**问题**：17 种实体（Arc/Circle/Ellipse/Point/Line/Spline/LwPolyline/Polyline/AttributeDefinition/AttributeEntity/Dimension/Hatch/MLine/Leader/Insert/Shape）携带 `normal` 字段定义其 Object Coordinate System，但 tessellation 时全部按 `(0,0,1)` 平铺到 XY 平面。`normal ≠ (0,0,1)` 的实体（典型：3D 工程模型上的标注）显示位置全错。

**算法**（`INTEGRATION_GAPS.md` 已记录）：

```rust
// arbitrary_axis(N) -> (Ax, Ay, N)
fn arbitrary_axis(n: [f64;3]) -> ([f64;3], [f64;3], [f64;3]) {
    let n = normalize(n);
    let ax = if n[0].abs() < 1.0/64.0 && n[1].abs() < 1.0/64.0 {
        cross([0.0, 0.0, 1.0], n)   // 注意：DXF spec 的 Y-world × N
    } else {
        cross([0.0, 1.0, 0.0], n)   // X-world × N
    };
    let ax = normalize(ax);
    let ay = cross(n, ax);
    (ax, ay, n)
}

fn ocs_to_wcs(p_ocs: [f64;3], origin: [f64;3], n: [f64;3]) -> [f64;3] {
    let (ax, ay, n) = arbitrary_axis(n);
    [
        origin[0] + p_ocs[0]*ax[0] + p_ocs[1]*ay[0] + p_ocs[2]*n[0],
        origin[1] + p_ocs[0]*ax[1] + p_ocs[1]*ay[1] + p_ocs[2]*n[1],
        origin[2] + p_ocs[0]*ax[2] + p_ocs[1]*ay[2] + p_ocs[2]*n[2],
    ]
}
```

**改动清单**

| 文件 | 改动 |
|---|---|
| `crates/h7cad-native-model/src/lib.rs`（或新 `geom_ocs.rs`） | 公开 `arbitrary_axis` / `ocs_to_wcs` 工具函数 |
| `src/scene/tessellate.rs` | 在每个受影响 entity 的 tessellator 入口处用 `ocs_to_wcs` 把 OCS 顶点转成 WCS；保留 origin=(0,0,0) 当 `normal=(0,0,1)` 时为 no-op |
| `src/entities/{arc,circle,ellipse,line,point,spline,lwpolyline,polyline,attdef,attrib,dimension,hatch,mline,leader,insert,shape}.rs` | 各 entity 的 `to_truck` / `wires` / `snap_pts` 改用 OCS→WCS 工具 |

**约束**：`Arc::tessellate` 当前用 `normal.z < 0` 反 sweep 方向作为唯一 mitigation，本轮把它替换为完整 OCS→WCS 后，反向 sweep 自然由 axis 翻转吸收。需要回归测试锁定原有 2D normal=(0,0,-1) 弧的视觉效果不变。

**新增测试**（`crates/h7cad-native-dxf/tests/ocs_arbitrary_axis.rs`，新文件）：

1. `arbitrary_axis_z_axis_returns_identity` — `normal=(0,0,1)` 时 ax=(1,0,0), ay=(0,1,0)
2. `arbitrary_axis_below_threshold_uses_z_world_cross` — `normal=(1/128,0,1/128)` 命中 `<1/64` 分支
3. `arbitrary_axis_above_threshold_uses_y_world_cross` — `normal=(1,0,0)` 命中默认分支
4. `arbitrary_axis_unit_vectors_orthogonal` — `proptest`：1000 个随机单位 normal，断言 ax⊥ay⊥n 且全为单位
5. `circle_with_tilted_normal_renders_in_correct_plane` — Read 一个 normal=(0,1,0) 的 CIRCLE，断言 wire 顶点在 XZ 平面而非 XY
6. `arc_with_negative_z_normal_keeps_legacy_sweep_visual` — 锁定 `normal=(0,0,-1)` 的弧反向 sweep 行为不变（防回归）

**新增 fixture**（`src/scene/mod.rs#[cfg(test)] mod tests`）：

- `fixture_3d_dxf_with_tilted_circle_renders_in_oblique_plane` — `display_scene()` 后 wire 顶点 z 维度有变化

**验收**

```bash
cargo test -p h7cad-native-dxf --quiet
cargo test --lib H7CAD scene::tests::fixture_ -- --nocapture
cargo test --bin H7CAD --quiet
cargo clippy -p H7CAD -- -D warnings
```

### 4.2 嵌套块 ByBlock 链递归继承

**问题**：`src/scene/render.rs::render_style_for_block_sub` 已支持单层 INSERT 的 ByBlock 子实体继承父 INSERT 的 color/linetype/lineweight。但当子实体本身是 INSERT（嵌套块）时，其内部递归调用 `render_style_for(document, e)` 重新求值，**祖先链断裂**。

`src/scene/mod.rs` L6297–6332 的 `Insert::explode_from_document` 单层调用是症结所在。

**方案**：把 ByBlock 解析改成**显式向下传递的 4 元组 `BlockChainStyle`**。任何 explode 出来的子 INSERT 拿到这个 4 元组，按需更新（如果它自己是 ByBlock，则继承；否则用自己的）后再向下传一层。

**改动清单**

| 文件 | 改动 |
|---|---|
| `src/scene/render.rs` | 新增 `pub(super) struct BlockChainStyle { color, pat_len, pat, lw_px }` + `BlockChainStyle::from_top(document, insert)` + `BlockChainStyle::resolve_sub(document, sub) -> ResolvedStyle` |
| `src/scene/mod.rs` | `tessellate_entity_at_world_offset` 的 INSERT 分支重构为递归形式：每层 explode 把 `BlockChainStyle` 传给下一层；若 sub 是 INSERT 则在递归前先 fold sub 的非 ByBlock 字段进去 |

**新增测试**

- `fixture_three_level_nested_block_byblock_color_chains_through_all_levels` — A→B→C 三层 INSERT，最里层 LINE color=ByBlock，linetype=ByBlock，lineweight=ByBlock；INSERT_A color=red，INSERT_B color=ByBlock，INSERT_C color=ByBlock；最终 wire 颜色=red
- `fixture_three_level_nested_block_byblock_linetype_chains_through_all_levels` — 同上但走 linetype 链
- `fixture_byblock_breaks_when_intermediate_insert_overrides_color` — A=red, B=blue, C=ByBlock，C 里 LINE=ByBlock → 应得 blue（验证非链而是逐层更新）

**验收**

```bash
cargo test --bin H7CAD scene::tests::fixture_three_level -- --nocapture
cargo test --bin H7CAD --quiet
```

---

## 5. Phase 1 — 字段宽度与高频显示字段

按用户感知度排序，每个子任务独立 PR。

### 5.1 LWPolyline / Polyline 宽度

| 文件 | 改动 |
|---|---|
| `src/scene/tessellate.rs::tessellate_lwpolyline` | 读 `constant_width` 与 `vertex.start_width / end_width`；非零时按 trapezoid strip 生成实心 ribbon 顶点而非 1 条 wire |
| `src/scene/tessellate.rs::tessellate_polyline2d` | `Vertex2D.start_width / end_width` 同上 |
| `src/entities/lwpolyline.rs` / `polyline.rs` | snap/grip 不变，只渲染层加 ribbon |
| 新增 `src/scene/wire_ribbon.rs` | 把可变宽度 polyline 按 sub-segment 分段，每段 trapezoid → tri-strip |

**测试**

- `lwpolyline_constant_width_emits_ribbon_strip` — 整段 width=1.0 的 polyline tessellation 产出顶点数比同形状零宽至少多 4×N
- `lwpolyline_tapered_width_per_vertex` — start=2.0/end=0.0 的单段 polyline 产出 trapezoid
- `polyline_legacy_per_vertex_width_matches_lwpolyline` — 两种 polyline 在等价输入下的 ribbon 顶点 ≈ 一致（容差 1e-6）

### 5.2 Spline weights + closed + periodic

| 文件 | 改动 |
|---|---|
| `src/entities/spline.rs::tessellate_spline` | de Boor 求值改用 rational NURBS 公式 (∑ wᵢ·Bᵢ·Pᵢ) / (∑ wᵢ·Bᵢ)；空 weights 视为均匀 1.0 兼容旧路径 |
| 同上 | `flags.closed` 时把首末控制点重叠或扩展 knot 满足 C¹ 续接；`periodic` 时按周期延展节点向量 |

**测试**

- `spline_unit_weights_match_uniform_bspline` — weights=[1,1,1,1] 与无 weights 路径产出 bit-identical
- `spline_circle_with_rational_weights_round_trips_within_eps` — 用 7-控点 rational NURBS 圆，tessellate 后所有顶点距圆心 |r - r̂| < 1e-3
- `spline_closed_flag_smoothly_joins_endpoints` — 首末点 + 切向均连续

### 5.3 MINSERT 阵列展开

| 文件 | 改动 |
|---|---|
| `src/scene/mod.rs` 的 INSERT 分支 | 检测 `column_count > 1 \|\| row_count > 1`，按 `(row_spacing, column_spacing)` 复制 instance；每个副本走相同 ByBlock 链解析 |
| `src/snap/mod.rs` | MINSERT snap_pts 包含每个副本的 Insertion 点 |

**测试**

- `minsert_3x4_emits_12_block_instances` — `column_count=3, row_count=4` 产出 12 倍的 wire 数
- `minsert_with_byblock_color_inherits_minsert_own_color` — 单元胞 ByBlock，MINSERT 自身 color=red → 12 个副本全 red

### 5.4 Dimension 从 dimstyle 应用样式

| 文件 | 改动 |
|---|---|
| `src/scene/tessellate.rs::tessellate_dimension` | 不再硬编码 arrow=0.12；查 `document.dim_styles[dim.dim_style]` 取 `DIMSCALE / DIMASZ / DIMEXO / DIMEXE`；fallback 到 `STANDARD` |
| `src/entities/dimension.rs` | `dimension_text_natural_rotation` 已修 code 53；本轮无改动，仅消费 dimstyle |

**测试**

- `dimension_arrow_size_scales_with_dimscale` — DIMSCALE=2.0 时箭头实际尺寸 = 2 × DIMASZ
- `dimension_extension_offset_uses_dimexo` — extension line 起点偏移与 DIMEXO 一致
- `dimension_falls_back_to_standard_when_dim_style_missing` — dim.dim_style="UnknownStyle" 时不 panic，使用 STANDARD

### 5.5 RasterImage clip_boundary

| 文件 | 改动 |
|---|---|
| `src/scene/tessellate.rs::populate_images` 或 image render path | 用 `clip_boundary` 多边形/矩形构造一张 alpha mask，与 RGBA pixels 复合 |
| 兼容 GPU shader：把 clip 多边形作为 SSBO 传给 fragment shader 做 inside-test，或在 CPU 侧 pre-multiply alpha |

**测试**

- `image_with_polygonal_clip_renders_only_inside_polygon` — 8×8 全白图 + 三角形 clip → 只有三角形内部像素 alpha>0
- `image_without_clip_renders_full_rectangle` — 缺 clip_boundary 走旧路径

### 5.6 Polyline3D 顶点 SPLINE_VERTEX flag

| 文件 | 改动 |
|---|---|
| `src/entities/polyline.rs::tessellate_polyline3d` | 读 `Vertex3D.VertexFlags`：SPLINE_CONTROL 顶点参与 spline 控制网；SPLINE_VERTEX 顶点是 fitted 输出（直接连）；常规顶点走旧路径 |

**测试**

- `polyline3d_with_spline_fitted_vertices_renders_smooth_curve` — 含 SPLINE_VERTEX 的 polyline3d 产出非折线
- `polyline3d_without_spline_flags_renders_polyline` — 兼容旧路径

---

## 6. Phase 2 — Bridge 补缺 + Reader bug 长尾

### 6.1 Native → compat 投影补 6 类实体

`src/io/native_bridge.rs` 当前对以下变体返回 `None`，导致打开后看不见：

| EntityData | 投影策略 |
|---|---|
| `Helix` | wire 螺旋 sample；复用 `acadrust::EntityType::Spline` |
| `Surface { extruded / lofted / revolved / swept / plane / nurb }` | 用各自的 ACIS wire 边线，复用 `acadrust::EntityType::Spline` 或 `Polyline3D` |
| `Light` | 占位 marker（cross + label）|
| `Camera` | 占位 marker |
| `Section` | section plane 用 4 顶点 polyline 框 |
| `ProxyEntity` | bounding box + 中心 X 标记，不丢失原 raw_codes |

每类一个独立 PR：`feat(bridge): map nm::<Variant> → compat for runtime display`，复制 `2026-04-25-arc-large-radial-dim-bridge-plan.md` 的模板。

**测试**：每个变体新增 `fixture_<variant>_displays_after_bridge` 锁定打开后 wires/hatches 非空。

### 6.2 5 个新实体 reader/writer

按 `2026-04-22-post-dimalt-roadmap.md` Path B 节奏，每轮 1 个：

| 轮次 | 实体 | 关键字段 | 主要风险 |
|---|---|---|---|
| R52 | OLE2FRAME | `binary_data: Vec<u8>` + insertion + size | binary 流的写出不能 corrupt |
| R53 | MPOLYGON | 复用 Hatch 的 boundary_paths + pattern | EntityData 新变体 |
| R54 | GEOPOSITIONMARKER | lat/lon/alt + label | 字段简单 |
| R55 | MATERIAL（实体引用形式） | 扩展 attribute pair | 当前只在 OBJECTS section 处理 |
| R56 | FIELD | template + cached_value | 字段语义复杂，先支持 cached_value 显示 |

**模板**：每个实体一个 PR：`feat(dxf): add <ENTITY_TYPE> reader/writer/bridge`。

### 6.3 Reader bug 长尾修复

| Bug | 修复位置 | 测试 |
|---|---|---|
| AttributeEntity rotation 缺 to_radians | `crates/h7cad-native-dxf/src/entity_parsers.rs::parse_attrib` | 读 `rotation 50` 字段后 `* PI / 180` |
| AttributeDefinition rotation 同上 | `parse_attdef` | 同上 |
| Shape rotation（低优先） | `parse_shape` | shape 渲染本身缺，先记录 |

**测试**：`tests/parser_rotation_bug.rs` 新文件，3 条 unit。

---

## 7. Phase 3 — acadrust 运行时移除（B5b–B5g）

承接 `2026-04-17-acadrust-removal-plan.md`，其余 5 子批：

| 子批 | 范围 | 工作量 | 验收 |
|---|---|---:|---|
| B5b | LINE / CIRCLE / ARC / POINT / ELLIPSE 5 个 draw command 切 native_store | 1 天 | `cargo check --no-default-features` 减 ≥ 5 处错 |
| B5c | LWPOLYLINE / SPLINE / TEXT / HATCH 4 个 draw command（先扩 `nm::EntityData` 缺字段）| 2-3 天 | 同上减 ≥ 8 处 |
| B5d | MOVE / ROTATE / SCALE / MIRROR / OFFSET 编辑命令收尾 | 2 天 | 同上减 ≥ 12 处 |
| B5e | MATCHPROP / XDATA / 选择族 | 3 天 | 同上减 ≥ 15 处 |
| B5f | Viewport / Insert / Layer 表（原 B4 核心）| 5 天 | 同上减至 ≤ 10 处 |
| B5g | 删除 `src/entities/*.rs` 的 `#[cfg(feature = "acadrust-compat")]` adapter；从 Cargo.toml 删 feature | 半天 | `cargo check --no-default-features` 全绿 |

每批独立 PR；最终 `acadrust::` 在 `src/` 下的引用从 ~700 → 0（除 `src/io/mod.rs` 的 DwgReader/DwgWriter 边界）。

---

## 8. 不在本计划

- **DWG body_decode 修复** — 见 `2026-04-22-post-dimalt-roadmap.md` Path C，独立战略
- **HEADER 长尾扩展** — Path A，ROI 递减，按需推进
- **Fuzz / property test** — Path D，可在 P3 完成后单独立项
- **Binary DXF 写出** — 长尾，无明确驱动
- **3DSOLID / Region / Body 真 ACIS parse** — 生态难度过高，依赖外部 crate
- **命令面缺口（XCLIP / BEDIT / ATTMAN 等）** — `ROADMAP.md` 中已记录，与文件层无关，按用户声音独立排期
- **CXF 外 Unicode 字体加载** — 字体子系统重构，独立计划
- **Snap 套件（Insertion / 嵌套 / Hatch boundary）** — 与本轮显示保真不冲突，独立 PR 链

---

## 9. 验收门（每 Phase 必跑）

```bash
# Phase 共通
cargo build --workspace --all-targets --locked
cargo test --workspace --all-targets --locked
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all -- --check

# DXF 专属
cargo test -p h7cad-native-dxf --quiet
cargo test --bin H7CAD scene::tests::fixture_ -- --nocapture
cargo test --bin H7CAD scene::tests::dualstore_ -- --nocapture
cargo test --bin H7CAD scene::tests::e2e_ -- --nocapture

# Phase 3 专属
cargo check -p H7CAD --no-default-features --locked    # 期望 0 error
```

每 PR commit message 用 `feat/fix/refactor(<scope>): …` 格式；`<scope>` ∈ {`dxf`, `bridge`, `tessellate`, `scene`, `acadrust-removal`}。

---

## 10. 时间表（粗估）

| 周次 | 内容 | 累计 PR |
|---|---|---:|
| W1 | P0.1 OCS→WCS + P0.2 嵌套 ByBlock | 2 |
| W1.5 | P1.1 Polyline width + P1.2 Spline weights | 4 |
| W2 | P1.3 MINSERT + P1.4 Dimension dimstyle | 6 |
| W2.5 | P1.5 RasterImage clip + P1.6 Polyline3D spline flag | 8 |
| W3 | P2.1 Helix + 6 类 Surface/Light/Camera/Section/Proxy bridge | 14 |
| W3.5 | P2.2 5 个新实体（一周一个，并行写）| 19 |
| W4 | P2.3 Reader bug 修复 + P3.B5b 启动 | 21 |
| W4–6 | P3 B5b–B5g 串行（含每子批独立验收）| 27 |

P0+P1 完成即可消除"打开任意真实 DXF 立刻肉眼可见的偏差"。P2 完成后产品上 DXF 显示路径达到生产级。P3 完成后整个运行时单一真源迁完，可以从 README 撕掉 "acadrust legacy" 注解。

---

## 11. 状态

- [ ] Phase 0 — OCS→WCS + 嵌套 ByBlock 链
- [ ] Phase 1 — 字段宽度族（6 子任务）
- [ ] Phase 2 — Bridge 补 6 类 + 5 新实体 + Reader bug
- [ ] Phase 3 — acadrust 移除 B5b–B5g
- [ ] 全计划闭合

---

## 附录 A：相关 plan 索引

- `2026-04-17-acadrust-removal-plan.md` — B1–B5g 总图
- `2026-04-21-dwg-save-version-honesty-plan.md` — 版本选择 UI 缺口
- `2026-04-22-post-dimalt-roadmap.md` — Path A/B/C/D 战略选项
- `2026-04-24-dxf-2d-display-closure-plan.md` — 本计划的前置（已闭合）
- `2026-04-24-write-dxf-strict-plan.md` — Phase 1 已落地
- `2026-04-25-arc-large-radial-dim-bridge-plan.md` — bridge 补缺模板
- `2026-04-26-native-dwg-next-plan.md` — DWG 路径战略

## 附录 B：相关 git 资产

- `INTEGRATION_GAPS.md` — 本计划主要 gap 输入源
- `ROADMAP.md` — UI 命令缺口（与本计划解耦）
- `COMMANDS.md` — 256 命令实现状态
- `CHANGELOG.md` — 历史轮次记录（截至三十九轮）
