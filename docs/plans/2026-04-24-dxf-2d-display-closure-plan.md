# DXF 常规二维图纸读写与显示收口计划

> **日期**: 2026-04-24
> **目标**: 关闭常规二维工程图的 DXF 读写与默认显示缺口
> **范围**: 不追求 AutoCAD 全量高保真；proxy/动态块/复杂 3D/ACIS 私有对象不在本轮

---

## 架构概览

```
DXF 文件
  ↓ read_dxf_bytes()
h7cad-native-dxf (读/写)
  ↓
h7cad-native-model::CadDocument (EntityData enum)
  ↓ native_doc_to_acadrust()        ← native_bridge
acadrust::CadDocument (compat)
  ↓ entity_wires() / populate_hatches / populate_images
Scene → GPU 渲染
```

**关键设计**: native 是保存真源，compat 是默认显示投影。`native_render_enabled` 默认关闭。

---

## Task 1: DXF 覆盖矩阵

### 验收实体覆盖状态

| 实体 | DXF Read | DXF Write | Bridge→compat | 默认显示(compat) | Native直渲染 | 备注 |
|------|----------|-----------|----------------|-----------------|-------------|------|
| **LINE** | ✅ | ✅ | ✅ | ✅ tessellate | ✅ | 完整 |
| **CIRCLE** | ✅ | ✅ | ✅ | ✅ tessellate | ✅ | 完整 |
| **ARC** | ✅ | ✅ | ✅ | ✅ tessellate | ✅ | 完整 |
| **POINT** | ✅ | ✅ | ✅ | ✅ tessellate | ✅ | 完整 |
| **ELLIPSE** | ✅ | ✅ | ✅ | ✅ tessellate | — | native直渲未列入supported |
| **LWPOLYLINE** | ✅ | ✅ | ✅ | ✅ tessellate | ✅ | 完整 |
| **POLYLINE** (2D/3D/Mesh) | ✅ | ✅ | ✅ (all 4 types) | ✅ tessellate | — | native直渲未列入supported |
| **SPLINE** | ✅ | ✅ | ✅ | ✅ tessellate | — | native直渲未列入supported |
| **TEXT** | ✅ | ✅ | ✅ | ✅ tessellate | ✅ | 完整 |
| **MTEXT** | ✅ | ✅ | ✅ | ✅ tessellate | ✅ | 完整 |
| **DIMENSION** | ✅ | ✅ | ✅ (dedicated fn) | ✅ tessellate | ✅ | 完整 |
| **LEADER** | ✅ | ✅ | ✅ | ✅ tessellate | — | native直渲未列入supported |
| **MLEADER/MULTILEADER** | ✅ | ✅ | ✅ (dedicated fn) | ✅ tessellate | ✅ | 完整 |
| **INSERT** | ✅ | ✅ | ✅ (w/ attribs) | ✅ tessellate | ✅* | *有attribs时bail out native |
| **ATTRIB** | ✅ | ✅ | ✅ | ✅ (via INSERT) | — | |
| **HATCH** | ✅ | ✅ | ✅ | ✅ populate_hatches | ✅ synced | native通过synced_hatch_models补充 |
| **SOLID** | ✅ | ✅ | ✅ | ✅ populate_hatches | — | 通过Hatch填充路径显示 |
| **3DFACE** | ✅ | ✅ | ✅ | ✅ tessellate | — | |
| **IMAGE** | ✅ | ✅ | ✅→RasterImage | ✅ populate_images | — | 依赖IMAGEDEF resolve |
| **WIPEOUT** | ✅ | ✅ | ✅ | ✅ (wipeout layer) | — | |
| **VIEWPORT** | ✅ | ✅ | ✅ | ✅ (layout viewport) | — | |
| **图层/线型/颜色/线宽** | ✅ | ✅ | ✅ (apply_common) | ✅ | ✅ | 通过entity common fields |
| **块定义** | ✅ (BLOCKS section) | ✅ | ⚠️ 部分 | ⚠️ | — | 见缺口1 |
| **布局** | ✅ (LAYOUT objects) | ✅ | ⚠️ 部分 | ⚠️ | — | 见缺口2 |

### 非验收实体（仅供参考）

| 实体 | Read | Write | Bridge | 备注 |
|------|------|-------|--------|------|
| RAY/XLINE | ✅ | ✅ | ✅ | 辅助几何 |
| SHAPE | ✅ | ✅ | ✅ | |
| MLINE | ✅ | ✅ | ✅ | |
| TOLERANCE | ✅ | ✅ | ✅ | |
| SOLID3D/REGION | ✅ | ✅ | ✅ | ACIS数据 |
| TABLE | ✅ | ✅ | ✅ | |
| MESH | ✅ | ✅ | ✅ | |
| PDF UNDERLAY | ✅ | ✅ | ✅ | |
| UNKNOWN | ✅ | ✅ | ✅→UnknownEntity | 原样写回 |
| **HELIX** | ✅ | ✅ | ❌ | 本轮不要求 |
| **ARC_DIMENSION** | ✅ | ✅ | ❌ | 本轮不要求 |
| **LARGE_RADIAL_DIM** | ✅ | ✅ | ❌ | 本轮不要求 |
| **SURFACE** | ✅ | ✅ | ❌ | 本轮不要求 |
| **LIGHT/CAMERA/SECTION** | ✅ | ✅ | ❌ | 本轮不要求 |
| **PROXY_ENTITY** | ✅ | ✅ | ❌ | 原样写回但不显示 |

### 关键缺口

**缺口 1: 块内实体不进 compat**
- `native_doc_to_acadrust()` 只遍历 `native.entities`（根实体列表），不走 `block_records[].entities`
- 块定义内的实体不会出现在 compat 文档中
- INSERT 引用的 block 在 compat 侧找不到内容 → 块引用显示为空或异常
- **影响**: 所有通过 INSERT 引用块内容的图纸可能显示不全

**缺口 2: 布局/视口投影不完整**
- `native_doc_to_acadrust()` 不完整地映射 layout/paper space 结构
- paper space 实体可能缺失 → 布局视图不完整

**缺口 3: ProxyEntity 不显示**
- ProxyEntity 可读写但 bridge 返回 None → 显示时丢失
- 第一阶段要求"尽量原样写回"，不要求可视化 → 可接受

---

## Task 2: 补默认显示缺口

### 2.1 补齐 `native_doc_to_acadrust()` 块定义投影

在 `native_doc_to_acadrust()` 中遍历 `native.block_records`，为每个 block 创建对应的 compat block_record 并投影内部实体。

**验收**: 打开包含 INSERT 的 DXF 后，块内容在默认视图可见。

### 2.2 补齐布局 paper space 实体

确保 paper space 中的实体也通过 bridge 投影到 compat。

**验收**: 多 layout DXF 文件在各 tab 中显示正常。

### 2.3 确认 populate_hatches / populate_images 覆盖

当前这两个函数只扫 compat 文档，bridge 投影完整后应自动覆盖。验证无遗漏。

---

## Task 3: 读写往返测试

### 重点字段

- handle/owner chain
- layer, linetype, color (by index and true color), lineweight
- TEXT: alignment, rotation, width_factor, oblique_angle
- MTEXT: attachment_point, line_spacing_factor, drawing_direction, rectangle_height
- DIMENSION: 各几何点、dim_style
- MLEADER: root 结构、leader line
- HATCH: boundary_paths (line/arc/polyline edges), pattern_name, solid_fill
- INSERT: block_name, scale[3], rotation, attribs
- VIEWPORT: center, width, height
- IMAGE: insertion, u_vector, v_vector, image_size, file_path

### 测试策略

为每类验收实体编写 roundtrip test:
```
write_dxf(doc) → bytes → read_dxf_bytes(bytes) → doc2
assert entity_count, key_fields match
```

---

## Task 4: 对齐 native 与 compat 双存储

编辑操作时：
- 已支持实体: native 与 compat 同步更新
- 不支持 direct-native 编辑的实体: 不做静默破坏，保持原 native 数据用于保存

---

## Task 5: 显示回归夹具

### 所需 fixture

| Fixture | 内容 | 检查项 |
|---------|------|--------|
| `basic_geometry.dxf` | LINE/CIRCLE/ARC/POINT/ELLIPSE/LWPOLYLINE | wires 非空 |
| `text_mtext.dxf` | TEXT/MTEXT 各种对齐 | wires 非空 + layer/handle 正确 |
| `dimension_leader.dxf` | DIMENSION/LEADER/MLEADER | wires 非空 + type 正确 |
| `block_insert.dxf` | BLOCK + INSERT w/ attribs | wires 非空 + 块内线段可见 |
| `hatch_solid.dxf` | HATCH + SOLID | hatches 缓存非空 |
| `image_wipeout.dxf` | IMAGE + WIPEOUT | images 缓存非空 |
| `layout_viewport.dxf` | Paper space + VIEWPORT | Model/Paper 不互相污染 |

测试不依赖 GPU 截图，检查加载后 Scene 产生的 wires/hatches/images 非空且关键 handle/type/layer 没丢。

---

## Task 6: 收口验收

每批改动后运行:
```bash
cargo test -p h7cad-native-dxf --quiet
cargo check -p H7CAD
```

最终:
```bash
cargo test --workspace --quiet
```

---

## 执行顺序

1. ~~Task 1: 覆盖矩阵~~ ✅ 已完成（本文档）
2. ~~Task 2: 补默认显示缺口~~ ✅ 已完成核心修复
   - `native_doc_to_acadrust()` 现在同步 *Model_Space/*Paper_Space handle
   - 为所有自定义块创建 compat BlockRecord
   - 块内实体现在正确投影到 compat 文档
   - Paper space 实体路由修复（通过 handle 同步）
   - `cargo check -p H7CAD` ✅ | `cargo test -p h7cad-native-dxf` ✅ (174 tests)
3. ~~Task 3: 读写 roundtrip 测试~~ ✅ 已完成（2026-04-24）
   - `crates/h7cad-native-dxf/tests/entity_2d_roundtrip.rs` 从 13 → 30 用例
   - 新增覆盖：LINE / CIRCLE / ARC / POLYLINE × 4 (2D/3D/PolygonMesh/PolyfaceMesh) / DIMENSION (linear + radius) /
     INSERT + ATTRIB / ATTDEF / HATCH (Line + CircularArc + EllipticArc edges) / WIPEOUT / MLINE / MULTILEADER
   - 句柄保活：`roundtrip_preserves_entity_handle` 锁定 handle/owner 写-读往返
   - 扩展 common 字段：`roundtrip_preserves_true_color_transparency_and_invisible`
   - 全部 30 用例 ✅
4. ~~Task 5: 显示回归 fixture~~ ✅ 已完成（2026-04-24）
   - 在 `src/scene/mod.rs#[cfg(test)] mod tests` 新增 7 个 `fixture_*` 测试：
     - `fixture_basic_geometry_wires_cover_every_entity` (LINE/CIRCLE/ARC/POINT/ELLIPSE/LWPOLYLINE)
     - `fixture_text_mtext_wires_preserve_layer_and_handle`
     - `fixture_dimension_and_leader_produce_wires`
     - `fixture_block_insert_shows_block_contents_after_bridge`
     - `fixture_hatch_solid_populates_hatch_cache`
     - `fixture_image_wipeout_project_into_compat_document`
     - `fixture_paper_space_entity_does_not_leak_into_model_wires`
   - 引入新辅助函数 `display_scene()` 复现最终用户默认显示路径（`native_render_enabled = false`）。
   - **附带修复两个 Task 2 之后仍潜伏的桥接缺口**（由这些 fixture 暴露）：
     1. `native_doc_to_acadrust()` 不再让 compat Layout 对象的 `block_record` 字段停在
        `acadrust::CadDocument::new()` 预设值——否则 `current_layout_block_handle()`
        返回与实际 BR 不一致的 handle，`belongs_to_visible_block()` 过滤掉所有实体。
     2. 桥接自定义块内实体时，若 `owner_handle = NULL` 或仍指向 native model space，
        改为显式写入该 block record 的 handle，防止块内线段泄漏到 model space。
   - H7CAD 全量测试 357 ✅（含 7 个新 fixture）。
5. ~~Task 4: 双存储对齐~~ ✅ 已完成（2026-04-24）
   - 修复 `Scene::copy_entities()`：旧实现只往 compat 添加克隆体，`save_dxf`/`save_dwg`
     走 native 导致克隆体在保存时丢失（静默数据丢失）。新版本在 compat 添加后
     立即通过 `acadrust_entity_to_native` 镜像进 native，保持 handle 同值。
   - 新增 4 个锁定测试：
     - `dualstore_copy_entities_mirrors_clones_into_native_document`
     - `dualstore_transform_entities_updates_both_stores_for_supported_entity`
     - `dualstore_erase_clears_derived_caches_not_just_storage`
     - `dualstore_add_entity_unsupported_by_native_bridge_does_not_panic`
   - 保留现有逃生通道：对 `acadrust_entity_to_native` 返回 None 的变体（如 Viewport）
     保留 compat-only 条目而非 panic，仍然可显示；Task 4 的"不做静默破坏"约束得到守护。
6. ~~Task 6: 收口验收~~ ✅ 已完成（2026-04-24）
   - 新增端到端验收测试 `e2e_native_writes_reads_bridges_and_displays_basic_geometry`：
     native build → `write_dxf` → `read_dxf_bytes` → bridge → `display_scene` →
     `entity_wires` / `synced_hatch_models`。覆盖 LINE/CIRCLE/TEXT/HATCH 四条关键通路，
     锁定整条 "读-写-桥-显" 数据链。
   - 指令验收：
     - `cargo test -p h7cad-native-dxf --quiet` ✅
     - `cargo check -p H7CAD` ✅
     - `cargo test --workspace --quiet` ✅ （除下列既存失败外全绿）

### 本轮交付概览

| 维度 | 数量 / 位置 |
|------|-------------|
| 新增 DXF 2D roundtrip 用例 | 17 个，覆盖 LINE/CIRCLE/ARC/POLYLINE×4/DIMENSION×2/INSERT+ATTRIB/ATTDEF/HATCH弧边/WIPEOUT/MLINE/MULTILEADER + 句柄/颜色保活 |
| 新增 Scene 显示回归 fixture | 7 个，`src/scene/mod.rs` `#[cfg(test)] mod tests` |
| 新增双存储对齐测试 | 4 个 `dualstore_*`，锁定 copy/transform/erase 对 native+compat 的一致性 |
| 新增端到端验收测试 | 2 个 `e2e_*`（基础几何 + 块引用），锁定完整数据流 |
| 新增 ProxyEntity 保护 | `roundtrip_proxy_entity_preserves_class_ids_and_raw_payload` 锁定第三方未知实体原样写回 |
| 新增 Paper Space 显示锁定 | `fixture_paper_space_entity_visible_when_layout_is_active`、`fixture_paper_viewport_projects_model_content_into_layout` 锁定布局切换与视口跨布局投射 |
| 修复的桥接缺口 | (1) `native_doc_to_acadrust()` 同步 Layout.block_record；(2) 块内实体 owner 回填；(3) `copy_entities` 镜像进 native 防止保存时静默丢失；(4) 桥接后给 paper-space Viewport 分配顺序 id（≥2）避免渲染器过滤掉 model-space 投射 |
| 修复的 DXF 写器缺口 | `write_blocks()` 曾把 BLOCK 代码 10（基准点）硬编码为 `[0,0,0]`，导致自定义锚点的块在保存后变形。现用 `br.base_point` 写回，由 `roundtrip_block_base_point_survives_write_read_cycle` 锁定。 |
| 新增图层 Roundtrip | `roundtrip_layer_table_preserves_all_render_relevant_fields` 锁定 color/linetype/lineweight/frozen/locked/plot/true_color/off 全字段保活 |
| 新增 XData Roundtrip | `roundtrip_entity_xdata_preserves_multiple_app_blocks` 锁定多应用、多代码组的第三方扩展数据保活 |
| 新增显示属性锁定 | `fixture_bylayer_entity_resolves_compat_display_color_from_layer`（BYLAYER 颜色继承）+ `fixture_insert_with_rotation_and_mirror_still_renders`（带镜像/旋转/缩放的块引用显示）|
| 新增图层可见性锁定 | `fixture_frozen_layer_hides_entities_from_default_display` + `fixture_layer_off_via_negative_aci_hides_entity` 锁定冻结/关闭图层隐藏实体 |
| 新增嵌套块锁定 | `fixture_nested_block_insert_displays_through_bridge` (2 层) + `fixture_three_level_nested_blocks_display_through_bridge` (3 层 SHEET→SYMBOL→DOT) + `fixture_circular_block_reference_terminates_without_overflow` (A→B→A 循环引用不无限递归) |
| 新增稳定性锁定 | `double_roundtrip_basic_geometry_is_structurally_stable`（二次 write-read 后实体类型/几何不漂移）+ `fresh_empty_document_survives_write_read_cycle`（空白文档最小用例）|
| 新增容错/导出锁定 | `fixture_unknown_hatch_pattern_falls_back_without_panic`（自定义 hatch 图案名命中 patterns 库失败时不崩溃）+ `svg_export_to_tempfile_smoke_test`（完整 `export_svg_full` 落盘路径）|
| 新增真实图像加载锁定 | `fixture_image_with_real_file_populates_images_cache` 在 temp dir 生成 4×4 PNG，验证 `populate_images_from_document` 真正解码像素（此前只覆盖了 None 分支） |
| 新增多边界 Hatch + DIMSTYLE 锁定 | `roundtrip_hatch_with_island_topology`（outer + inner hole 两个 boundary_paths 保活）+ `roundtrip_dimstyle_table_preserves_render_relevant_fields`（DIMSTYLE 表全字段保活）|

### 已知既存失败（与本轮收口无关）

- `h7cad-native-dwg::real_dwg_samples_baseline_m3b`：DWG (非 DXF) 读取器对
  `sample_AC1015.dwg` 的 LINE entity_body_decode 基线数只到 26（要求 ≥ 40）。
  本轮修改前即已失败，归属 DWG 阅读器回归，不在本计划范围内。
