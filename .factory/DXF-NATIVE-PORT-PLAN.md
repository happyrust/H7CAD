# H7CAD DXF Native Port — 开发计划

> 基准：ACadSharp (C#) → h7cad-native-dxf (Rust)
> 更新：2026-04-09 (第4轮更新)
> 状态标记：✅ 已完成 · 🔨 进行中 · ⬜ 未开始

---

## 总览

将 ACadSharp 的完整 DXF 读取能力移植到 Rust native 栈（h7cad-native-dxf），
最终替代 acadrust 依赖，实现自主可控的 DXF/DWG IO。

### 里程碑规划

| 里程碑 | 目标 | 门槛 |
|--------|------|------|
| **M1** — DXF Read Core | 能读取标准 DXF 文件，产出完整 `CadDocument` | ACadSharp 测试样例 100% 通过 |
| **M2** — DXF Write | `CadDocument` → DXF 文件往返 | roundtrip 基础实体无损 ✅ |
| **M3** — DWG Read | 最小 DWG 读取 | 基础 DWG R2000+ 可读 |
| **M4** — 主程序切换 | H7CAD 主程序接入 native 栈 | 桌面 smoke 通过 |

### 当前进度

| 维度 | 数据 |
|------|------|
| Rust 代码量 | ~3200 行 (dxf, 含 writer) + ~1050 行 (model) = ~4250 行 |
| 测试数 | 55 (51 DXF + 4 Model) |
| 实体类型 | 33+ 种，0 Unknown (AC1015) |
| 对象类型 | 17 种，92% 覆盖 (AC1018) |
| 表属性 | LAYER/LTYPE/STYLE/DIMSTYLE 详细解析 |
| Header 变量 | 16 个 ($ACADVER...$HANDSEED) |
| 交叉引用 | owner_handle + resolve API |
| DXF Writer | ✅ 完整写入 + roundtrip 验证 |
| M1 完成度 | **~85%** |
| M2 完成度 | **~90%** (基础 roundtrip 已通过) |

---

## M1：DXF Read Core — Feature 链

### Phase 0-1：基础骨架 + Tokenizer ✅

已完成工作区骨架、handle/ownership 核心、DxfTokenizer + GroupCode + DxfValue 解码。

### Phase 2：Section 框架 ✅

DxfStreamReader + SECTION/ENDSEC 状态机 + 6 段分发。

### Phase 3：头信息与元数据 ✅

| Feature | 状态 | 说明 |
|---------|------|------|
| $ACADVER | ✅ | R12..R2018 全覆盖 |
| Header 多值变量 | ✅ | EXTMIN/EXTMAX/INSBASE/LIMMIN/LIMMAX/LTSCALE/PDMODE/PDSIZE/TEXTSIZE/DIMSCALE/LUNITS/LUPREC/AUNITS/AUPREC/HANDSEED |
| DxfClass 读取 | ✅ | dxf_name/cpp_class/app/proxy_flags/is_entity |
| 编码切换 | ⬜ | ≥AC1021 → UTF-8 |

### Phase 4：Tables Section ✅

| Feature | 状态 | 说明 |
|---------|------|------|
| TABLE…ENDTAB 框架 | ✅ | handle+name 提取 |
| LAYER 详细属性 | ✅ | color/linetype/lineweight/frozen/locked/true_color/plot → LayerProperties |
| LTYPE 详细属性 | ✅ | description/pattern_length/segments → LinetypeProperties |
| STYLE 详细属性 | ✅ | height/width_factor/oblique_angle/font_name → TextStyleProperties |
| DIMSTYLE 详细属性 | ✅ | dimscale/dimasz/dimexo/dimgap/dimtxt/dimdec/dimlunit/dimaunit → DimStyleProperties |
| VPORT | ⬜ | 视口配置 |

### Phase 5：Blocks Section ✅

| Feature | 状态 | 说明 |
|---------|------|------|
| BLOCK…ENDBLK 读取 | ✅ | base_point + block_entity_handle |
| 块内实体解析 | ✅ | 复用 read_entity，25 块含 176 实体 |
| 自动注册 BlockRecord | ✅ | BLOCK → BLOCK_RECORD 关联 |

### Phase 6-8：实体解析 ✅

**33+ 种实体类型，0 Unknown (AC1015)**

| 类别 | 已实现 |
|------|--------|
| 基础几何 | LINE, CIRCLE, ARC, POINT, ELLIPSE, SPLINE (weights+tangents), 3DFACE, SOLID, TRACE, RAY, XLINE |
| 文字注释 | TEXT, MTEXT (code 1+3 拼接), ATTRIB, ATTDEF, DIMENSION (7 子类型 19 字段), LEADER, TOLERANCE |
| 复杂实体 | LWPOLYLINE, POLYLINE (2D/3D/Mesh+VERTEX/SEQEND), INSERT (+ATTRIB+SEQEND 序列), HATCH (boundary paths), VIEWPORT, MLINE, IMAGE, WIPEOUT |
| 3D | SHAPE, 3DSOLID, REGION, BODY |
| 通用属性 | handle, owner_handle, layer_name, linetype_name, color_index, true_color, lineweight, invisible, transparency |

### Phase 10：Objects Section ✅

**17 种对象类型，92% 覆盖 (AC1018)**

DICTIONARY, XRECORD, GROUP, LAYOUT, PLOTSETTINGS, DICTIONARYVAR, SCALE, VISUALSTYLE, MATERIAL, IMAGEDEF, IMAGEDEF_REACTOR, MLINESTYLE, MLEADERSTYLE, TABLESTYLE, SORTENTSTABLE, DIMASSOC + Unknown

### Phase 11-12：映射框架 + 模板构建

| Feature | 状态 | 说明 |
|---------|------|------|
| 组码映射宏 | ⬜ | derive 宏 or 手写映射 |
| 模板系统 | ⬜ | CadTemplate 句柄占位 |
| 文档构建器 | ⬜ | 句柄→对象、owner 归属 |
| XData 读取 | ⬜ | 1000-1071 系列组码 |

### Phase 13：整合与验证 ✅

| Feature | 状态 | 说明 |
|---------|------|------|
| Facade 接通 | ✅ | load(Dxf, bytes) + save(Dxf, doc) |
| ACadSharp 样本 | ✅ | 7 个样本全部通过 (AC1009~AC1032) |
| 后处理 | ✅ | handle seed 同步 + pre-seeded 清理 |
| 交叉引用 API | ✅ | resolve_color/linetype/lineweight, model/paper space, resolve_insert_block |
| 便捷 API | ✅ | entity_type_counts, compute_extents, model_space_entities |
| Binary DXF | ⬜ | sentinel + Int16 组码 |
| Legacy 编码 | ⬜ | AC1009 + 非 UTF-8 |

---

## M2：DXF Write ✅ (基础完成)

| Feature | 状态 | 说明 |
|---------|------|------|
| DxfWriter 核心 | ✅ | group code/value 写入、f64 精度处理 |
| HEADER 写入 | ✅ | $ACADVER + 15 个变量 + $HANDSEED |
| CLASSES 写入 | ✅ | 全部字段 |
| TABLES 写入 | ✅ | VPORT/LTYPE/LAYER/STYLE/DIMSTYLE/BLOCK_RECORD |
| BLOCKS 写入 | ✅ | BLOCK…ENDBLK + 块内实体 |
| ENTITIES 写入 | ✅ | 33+ 种实体完整写入 |
| OBJECTS 写入 | ✅ | 17 种对象完整写入 |
| Facade save() | ✅ | DXF 格式写入接通 |
| Roundtrip 测试 | ✅ | 最小文档 + 实体 + AC1015 真实文件 |

---

## 模块结构

```
h7cad-native-dxf/src/
├── lib.rs              (~1500 行) Section readers, API, tests
├── tokenizer.rs        (~233 行)  DxfToken, GroupCode, DxfValue, DxfTokenizer
├── entity_parsers.rs   (~800 行)  30+ parse_* 纯函数
└── writer.rs           (~600 行)  DXF 文本写入器

h7cad-native-model/src/
└── lib.rs              (~1050 行) CadDocument, Entity, EntityData, tables, objects

h7cad-native-facade/src/
└── lib.rs              (~25 行)   load/save 统一接口
```

---

## 剩余工作（M1 完全完成需要）

| 项目 | 优先级 | 复杂度 | 说明 |
|------|--------|--------|------|
| MULTILEADER 详细解析 | 中 | ★★★★ | 当前简化处理 |
| VPORT 表 | 低 | ★★ | 视口配置 |
| XData 读取 | 中 | ★★★ | 应用扩展数据 |
| Binary DXF | 中 | ★★★ | sentinel + Int16 组码 |
| ACAD_TABLE | 低 | ★★★ | 表格实体 |
| MESH 详细 | 低 | ★★ | 顶点/面片数据 |
| 编码支持 | 低 | ★★ | UTF-8 / legacy codepage |

---

## 验证策略

1. 每个 Feature：`cargo test -p h7cad-native-dxf` 通过
2. 每个 Phase：ACadSharp 样本端到端验证
3. M1 门槛：7 个 ACadSharp DXF 样本全部正确解析 ✅
4. M2 门槛：roundtrip 基础实体无损 ✅
5. 交叉验证：model/paper space 分类、ByLayer 颜色解析、INSERT→Block 引用
