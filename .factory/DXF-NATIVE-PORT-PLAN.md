# H7CAD DXF Native Port — 开发计划

> 基准：ACadSharp (C#) → h7cad-native-dxf (Rust)
> 更新：2026-04-09 (最后更新于本轮)
> 状态标记：✅ 已完成 · 🔨 进行中 · ⬜ 未开始

---

## 总览

将 ACadSharp 的完整 DXF 读取能力移植到 Rust native 栈（h7cad-native-dxf），
最终替代 acadrust 依赖，实现自主可控的 DXF/DWG IO。

### 里程碑规划

| 里程碑 | 目标 | 门槛 |
|--------|------|------|
| **M1** — DXF Read Core | 能读取标准 DXF 文件，产出完整 `CadDocument` | ACadSharp 测试样例 100% 通过 |
| **M2** — DXF Write | `CadDocument` → DXF 文件往返 | roundtrip 基础实体无损 |
| **M3** — DWG Read | 最小 DWG 读取 | 基础 DWG R2000+ 可读 |
| **M4** — 主程序切换 | H7CAD 主程序接入 native 栈 | 桌面 smoke 通过 |

---

## M1：DXF Read Core — Feature 链

### Phase 0：工作区骨架 ✅

| Feature | 状态 | 说明 |
|---------|------|------|
| m1-crate-workspace-skeleton | ✅ | 6 个 crate 骨架已建立 |
| m1-builder-handle-owner-core | ✅ | native-builder 句柄/ownership 核心 |

### Phase 1：Tokenizer ✅

| Feature | 状态 | 说明 |
|---------|------|------|
| m1-dxf-tokenizer-groupcode | ✅ | `DxfTokenizer` + `GroupCode` + `DxfValue` 解码，5 个单元测试通过 |

### Phase 2：Section 框架 ✅

| Feature | 状态 | 对标 ACadSharp |
|---------|------|----------------|
| m1-dxf-section-reader-framework | ✅ | `DxfReader.Read()` 主循环：SECTION/ENDSEC 状态机，按段名分发 |
| m1-dxf-stream-reader-trait | ✅ | `DxfStreamReader`：text 格式流读取（binary 待 Phase 13） |

**核心设计**：
- `DxfSectionDispatcher`：遇 `(0, "SECTION")` → 读 `(2, name)` → dispatch
- 支持 HEADER / CLASSES / TABLES / BLOCKS / ENTITIES / OBJECTS
- 未知 section 跳到 ENDSEC

### Phase 3：头信息与元数据

| Feature | 状态 | 对标 ACadSharp |
|---------|------|----------------|
| m1-dxf-header-reader | ✅ | `$ACADVER` → `DxfVersion`（R12..R2018 全覆盖），多值变量自动跳过 |
| m1-dxf-classes-reader | ✅ | `DxfClass`（dxf_name/cpp_class/app/proxy_flags/is_entity），连续 CLASS 条目解析 |
| m1-dxf-version-encoding | ⬜ | 编码切换（≥AC1021 → UTF-8），当前 text-only 模式暂不需要 |

### Phase 4：Tables Section ✅

| Feature | 状态 | 对标 ACadSharp |
|---------|------|----------------|
| m1-dxf-tables-section-reader | ✅ | TABLE…ENDTAB 循环，handle+name 提取 |
| m1-dxf-table-layer | ✅ | LAYER name+handle → SymbolTable |
| m1-dxf-table-ltype | ✅ | LTYPE name+handle → SymbolTable |
| m1-dxf-table-style | ✅ | STYLE name+handle → SymbolTable |
| m1-dxf-table-dimstyle | ✅ | DIMSTYLE name+handle → SymbolTable |
| m1-dxf-table-vport | ⬜ | VPORT 视口配置（跳过） |
| m1-dxf-table-block-record | ✅ | BLOCK_RECORD name+handle → SymbolTable |
| m1-dxf-table-appid | ✅ | APPID name+handle → SymbolTable |
| m1-dxf-table-ucs-view | ✅ | UCS + VIEW name+handle → SymbolTable |

> 注：当前仅提取 name+handle 填 SymbolTable，表条目详细属性（颜色/线型模式/尺寸变量等）待后续增量补充

### Phase 5：Blocks Section ✅

| Feature | 状态 | 对标 ACadSharp |
|---------|------|----------------|
| m1-dxf-blocks-section-reader | ✅ | BLOCK…ENDBLK 读取，块内实体暂跳过 |
| m1-dxf-block-record-linkage | ✅ | 自动注册到 block_record SymbolTable |

### Phase 6：Entities Section — 基础几何

| Feature | 状态 | 对标 ACadSharp |
|---------|------|----------------|
| m1-dxf-entities-section-reader | ✅ | 实体分发框架 + common codes（5/8/6/62） |
| m1-dxf-entity-common-codes | ✅ | handle/layer/linetype/color → Entity 基类 |
| m1-dxf-entity-line | ✅ | LINE（start/end 3D） |
| m1-dxf-entity-circle-arc | ✅ | CIRCLE + ARC（center/radius/angles） |
| m1-dxf-entity-point | ✅ | POINT（position 3D） |
| m1-dxf-entity-ellipse | ✅ | ELLIPSE（center/major_axis/ratio/params） |
| m1-dxf-entity-lwpolyline | ✅ | LWPOLYLINE（vertices + bulge + closed） |
| m1-dxf-entity-text | ✅ | TEXT（insertion/height/value/rotation） |
| m1-dxf-entity-spline | ✅ | SPLINE（degree/knots/control_points/fit_points） |
| m1-dxf-entity-3dface | ✅ | 3DFACE（4 corners） |
| m1-dxf-entity-solid-trace | ✅ | SOLID / TRACE（4 corners） |
| m1-dxf-entity-ray-xline | ✅ | RAY / XLINE（origin + direction） |
| m1-dxf-entity-unknown | ✅ | 未知实体类型保留为 Unknown{entity_type} |

### Phase 7：Entities — 文字与注释

| Feature | 状态 | 对标 ACadSharp |
|---------|------|----------------|
| m1-dxf-entity-text | ✅ | TEXT（在 Phase 6 完成） |
| m1-dxf-entity-mtext | ✅ | MTEXT（insertion/height/width/value/rotation，code 1+3 拼接） |
| m1-dxf-entity-attrib-attdef | ✅ | ATTRIB（tag/value/insertion/height）+ ATTDEF（tag/prompt/default） |
| m1-dxf-entity-dimension | ✅ | DIMENSION 全 7 种子类型（Linear/Aligned/Angular2Line/Angular3Pt/Radius/Diameter/Ordinate），含 style_name/measurement/attachment_point/first_point/second_point/angle_vertex/dimension_arc/leader_length/rotation/ext_line_rotation 等完整字段 |
| m1-dxf-entity-leader | ✅ | LEADER（vertices + arrowhead） |
| m1-dxf-entity-multileader | ⬜ | MULTILEADER（嵌套 `{`/`}` 上下文解析，**高复杂度**） |
| m1-dxf-entity-tolerance | ⬜ | TOLERANCE |

### Phase 8：Entities — 复杂实体

| Feature | 状态 | 对标 ACadSharp |
|---------|------|----------------|
| m1-dxf-entity-polyline | ✅ | POLYLINE（2D/3D/面片/网格）+ VERTEX/SEQEND 序列读取 |
| m1-dxf-entity-hatch | ✅ | HATCH（pattern_name + solid_fill，边界环数据待补） |
| m1-dxf-entity-insert | ✅ | INSERT（block_name/insertion/scale/rotation） |
| m1-dxf-entity-mline | ✅ | MLINE（vertices/style_name/scale） |
| m1-dxf-entity-table | ⬜ | ACAD_TABLE |
| m1-dxf-entity-viewport | ✅ | VIEWPORT（center/width/height） |
| m1-dxf-entity-image-wipeout | ✅ | IMAGE（insertion/uv_vectors/size）+ WIPEOUT（clip_vertices） |
| m1-dxf-entity-mesh | ⬜ | MESH |

### Phase 9：Entities — 3D/ACIS

| Feature | 状态 | 对标 ACadSharp |
|---------|------|----------------|
| m1-dxf-entity-3dsolid | ⬜ | BODY / 3DSOLID / REGION（ACIS 数据块读取） |
| m1-dxf-entity-underlay | ⬜ | PDFUNDERLAY / DWFUNDERLAY / DGNUNDERLAY |
| m1-dxf-entity-ole2frame | ⬜ | OLE2FRAME |
| m1-dxf-entity-shape | ⬜ | SHAPE |

### Phase 10：Objects Section ✅

| Feature | 状态 | 对标 ACadSharp |
|---------|------|----------------|
| m1-dxf-objects-section-reader | ✅ | 对象分发框架 + handle/owner 提取 |
| m1-dxf-object-dictionary | ✅ | DICTIONARY / ACDBDICTIONARYWDFLT（key→handle 条目） |
| m1-dxf-object-imagedef | ✅ | IMAGEDEF（file_name/size）+ IMAGEDEF_REACTOR（image_handle） |
| m1-dxf-object-xrecord | ✅ | XRECORD（data_pairs 保留） |
| m1-dxf-object-group | ✅ | GROUP（description + entity_handles） |
| m1-dxf-object-mlinestyle | ✅ | MLINESTYLE（name/description/element_count） |
| m1-dxf-object-layout | ✅ | LAYOUT（name/tab_order/block_record_handle/plot_paper_size/plot_origin） |
| m1-dxf-object-plotsettings | ✅ | PLOTSETTINGS（page_name/printer_name/paper_size） |
| m1-dxf-object-tablestyle | ✅ | TABLESTYLE（name/description） |
| m1-dxf-object-mleaderstyle | ✅ | MLEADERSTYLE（name/content_type/text_style_handle） |
| m1-dxf-object-sortentstable | ✅ | SORTENTSTABLE（entity_handles/sort_handles） |
| m1-dxf-object-dictionaryvar | ✅ | DICTIONARYVAR（schema/value） |
| m1-dxf-object-scale | ✅ | SCALE（name/paper_units/drawing_units/is_unit_scale） |
| m1-dxf-object-visualstyle | ✅ | VISUALSTYLE（description/style_type） |
| m1-dxf-object-material | ✅ | MATERIAL（name） |
| m1-dxf-object-dimassoc | ✅ | DIMASSOC（associativity/dimension_handle） |

### Phase 11：组码映射框架

| Feature | 状态 | 对标 ACadSharp |
|---------|------|----------------|
| m1-dxf-groupcode-map | ⬜ | `DxfMap` 等价物：derive 宏 or 手写，子类 100 + 组码 → 字段 |
| m1-dxf-reference-type | ⬜ | 区分立即赋值 vs 延迟 Handle 解析 |

### Phase 12：模板与文档构建

| Feature | 状态 | 对标 ACadSharp |
|---------|------|----------------|
| m1-dxf-template-system | ⬜ | `CadTemplate` / `CadEntityTemplate`：句柄、表引用占位 |
| m1-dxf-document-builder | ⬜ | `DxfDocumentBuilder.BuildDocument()`：句柄→对象、owner 归属、字典构建 |
| m1-dxf-xdata-reader | ⬜ | XData（1000–1071 系列组码）读取 |

### Phase 13：整合与验证

| Feature | 状态 | 对标 ACadSharp |
|---------|------|----------------|
| m1-dxf-facade-wire-up | ✅ | `h7cad-native-facade::load(Dxf, bytes)` → `read_dxf` |
| m1-dxf-acad-fixture-tests | ✅ | 7 个 ACadSharp 样本全部通过（AC1009~AC1032） |
| m1-dxf-binary-format | ⬜ | 二进制 DXF 格式支持（sentinel + Int16 组码） |
| m1-dxf-encoding-legacy | ⬜ | AC1009 二进制变体 + 非 UTF-8 编码支持 |

---

## 推荐执行顺序

```
Phase 0-1 (✅ 已完成)
    │
    ▼
Phase 2: Section 框架 ← 【当前下一步】
    │
    ├─→ Phase 3: Header/Classes/版本
    │
    ├─→ Phase 11: 组码映射框架 (可并行)
    │
    ▼
Phase 4: Tables
    │
    ▼
Phase 5: Blocks
    │
    ▼
Phase 6: 基础几何实体 (LINE/CIRCLE/ARC...)
    │
    ├─→ Phase 7: 文字/注释实体
    │
    ├─→ Phase 8: 复杂实体 (HATCH/POLYLINE/INSERT...)
    │
    ├─→ Phase 9: 3D/ACIS 实体
    │
    ▼
Phase 10: Objects
    │
    ▼
Phase 12: 模板/文档构建
    │
    ▼
Phase 13: 整合验证
```

---

## 复杂度热力图

| 组件 | 代码量估计 | 复杂度 | 备注 |
|------|-----------|--------|------|
| Section 框架 | ~200 行 | ★★ | 状态机，依赖清晰 |
| Header 读取 | ~150 行 | ★★ | 变量表映射 |
| Tables | ~600 行 | ★★★ | 9 种表类型 |
| 基础几何实体 | ~400 行 | ★★ | LINE/CIRCLE 等直接映射 |
| POLYLINE 全变体 | ~300 行 | ★★★★ | 2D/3D/面片/网格/AC1009 legacy |
| HATCH | ~500 行 | ★★★★★ | 边界环/图案/渐变，ACadSharp 中最复杂 |
| DIMENSION 6 种 | ~400 行 | ★★★★ | 类型分发 + 子类匹配 |
| MULTILEADER | ~350 行 | ★★★★ | 嵌套结构解析 |
| 3DSOLID/ACIS | ~200 行 | ★★★ | ACIS 数据块 |
| Objects 全量 | ~500 行 | ★★★ | 10+ 对象类型 |
| 组码映射宏 | ~300 行 | ★★★★ | derive 宏设计 |
| 模板/Builder | ~400 行 | ★★★★ | 两阶段构建，句柄图 |

**总估计**：M1 完整实现约 **4000-5000 行** Rust 代码

**当前代码量**：~1800 行（dxf） + ~600 行（model） = ~2400 行，约 M1 的 50%

---

## 既有阻塞

- `cargo test --workspace` 失败：根包 `src/io/print_to_printer.rs` 引用 `windows_sys` 未正确解析
- 与 DXF native port 无关，但阻碍全量回归验证
- 建议：在根 `Cargo.toml` 修复 `windows_sys` 依赖，或 crate 级单独测试

---

## 验证策略

1. **每个 Feature 完成后**：`cargo test -p h7cad-native-dxf` 通过
2. **每个 Phase 完成后**：用最小 DXF 样本做端到端验证
3. **M1 完成门槛**：ACadSharp 测试样例中的 DXF 文件均可正确解析为 `CadDocument`
4. **双栈对比**：同一 DXF 分别用 acadrust 和 native 加载，对比关键字段
