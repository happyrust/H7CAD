# PID Real Geometry Display Implementation Plan

> **Date:** 2026-04-30  
> **Goal:** 让 H7CAD 从当前 `.pid` 拓扑/诊断预览升级为能显示 Smart P&ID 文件中真实 Sheet 图形的 CAD 视图。  
> **Scope:** H7CAD 显示链路 + `pid-parse` 几何解码模型。第一版优先覆盖真实样本的线、折线、文本、端点连接和符号占位，不追求一次性完全复刻 SmartPlant 的全部样式。

---

## 1. Current State

### 已经具备

- H7CAD 已经支持 `.pid` 打开入口：
  - `src/io/mod.rs::open_document_blocking` 对 `pid` 扩展调用 `pid_import::open_pid`
  - `src/app/update.rs::Message::FileOpened` 会安装 `PidTabState`
  - `PidOpenBundle.native_preview` 会进入 `Scene::set_native_doc`
- `src/io/pid_import.rs` 已有 `pid_document_to_preview`，能把 `PidImportView` / `PidLayoutModel` 转成 H7CAD native 图元。
- `pid-parse` 已经能读取 CFB、TaggedTxtData、JSite、Dynamic Attributes、Sheet endpoint、object graph、cross-reference 和 heuristic layout。
- H7CAD native model 已能承载本计划需要的第一批图元：
  - `Line`
  - `LwPolyline`
  - `Arc`
  - `Circle`
  - `Text`
  - `MText`
  - `Insert`

### 当前限制

- `pid-parse/src/layout.rs` 明确说明当前 layout 是 topology-driven heuristic，不是 CAD geometry decode。
- `SheetGeometry` 当前只包含 `texts`、`endpoints`、`coordinate_hints`，这些是 probe evidence，不是可直接渲染的真实几何。
- H7CAD 当前 PID 视图的主图来自 `derive_layout`，因此更像对象关系图，而不是原始 P&ID 图纸。
- 如果直接在 H7CAD 侧“美化预览”，会把 heuristic 图画得更像图纸，但仍不是文件里的真实图形。

## 2. Target Architecture

推荐把真实 PID 图形显示拆成三层。

### Layer A: `pid-parse` 解码层

新增稳定 DTO：

```rust
pub struct NormalizedPidGeometry {
    pub entities: Vec<PidGraphicEntity>,
    pub warnings: Vec<String>,
}

pub struct PidGraphicEntity {
    pub id: String,
    pub drawing_id: Option<String>,
    pub graphic_oid: Option<u32>,
    pub kind: PidGraphicKind,
    pub source: PidGraphicProvenance,
    pub confidence: PidGeometryConfidence,
}
```

`PidGraphicKind` 第一版建议覆盖：

- `Line { start, end }`
- `Polyline { points, closed }`
- `Arc { center, radius, start_angle, end_angle }`
- `Circle { center, radius }`
- `Text { insertion, value, height, rotation }`
- `SymbolInstance { insertion, symbol_path, rotation, scale }`
- `Unknown { note }`

所有实体必须带 provenance：stream path、byte range、record id / field_x / graphic_oid、解析来源是 decoded 还是 inferred。

### Layer B: H7CAD 投影层

在 `src/io/pid_import.rs` 中新增一条优先路径：

```text
PidDocument
  -> pid_parse::geometry::build_normalized_geometry(...)
  -> pid_geometry_to_native_doc(...)
  -> PidOpenBundle.native_preview
```

当真实几何为空或置信度不足时，继续 fallback 到现有：

```text
PidDocument
  -> derive_layout(...)
  -> pid_document_to_preview(...)
```

### Layer C: UI / 回归层

H7CAD 保持现有 `.pid` 打开路径不变；只改变 `PidOpenBundle.native_preview` 的来源优先级。

验证面包括：

- PID tab 正常打开
- 真实几何图层优先显示
- 装饰诊断图层不影响主图 fit
- `PIDSHOT` 或现有 deterministic screenshot 路径输出可回归 PNG

## 3. Delivery Phases

### Phase 0: 建立真实样本基线

**Goal:** 明确当前真实样本中有哪些可用证据，避免盲解。

**Files:**

- `pid-parse/src/bin/pid_inspect.rs`
- `pid-parse/src/inspect/report.rs`
- `pid-parse/src/parsers/sheet_probe.rs`
- `H7CAD/src/io/pid_import.rs`

**Tasks:**

1. 固定验收样本：
   - `D:\work\plant-code\cad\pid-parse\test-file\工艺管道及仪表流程-1.pid`
2. 输出并保存以下计数：
   - Sheet stream 数量
   - text run 数量
   - coordinate hint 数量
   - endpoint record 数量
   - object / relationship 数量
   - JSite symbol usage 数量
3. 在 H7CAD 添加或更新目标样本测试，断言：
   - `.pid` 可打开
   - `PidOpenBundle.summary.object_count > 0`
   - 当前 fallback preview 仍可生成

**Validation:**

```powershell
cargo test -p H7CAD --bin H7CAD open_target_pid_sample -- --nocapture
cd ..\pid-parse
cargo test --test publish_xml_cli -- --nocapture
```

**Stop Conditions:**

- 真实样本不存在时只 soft-skip，不制造本机路径硬失败。
- 如果 object graph 不可用，先回到 `pid-parse` 修解析，不进入 H7CAD 投影。

### Phase 1: 定义 normalized geometry DTO

**Goal:** 先定稳定输出契约，再逐步填充 parser。

**Files:**

- `pid-parse/src/model.rs`
- `pid-parse/src/schema.rs`
- `pid-parse/src/lib.rs`
- `pid-parse/src/import_view.rs` 或新增 `pid-parse/src/geometry.rs`

**Tasks:**

1. 新增 `NormalizedPidGeometry`、`PidGraphicEntity`、`PidGraphicKind`、`PidGraphicProvenance`。
2. DTO 字段保持保守：
   - 必须能表达来源和置信度
   - 不把未确认字段命名成确定语义
   - 支持 `Unknown` / `Inferred`，但 H7CAD 默认不渲染 unknown
3. 添加 schema snapshot / model tests。
4. 公开 builder：
   - `build_normalized_geometry(doc: &PidDocument) -> NormalizedPidGeometry`

**Validation:**

```powershell
cd ..\pid-parse
cargo test --lib geometry model schema -- --nocapture
cargo test --test parser_panic_safety
```

**Stop Conditions:**

- DTO 一旦暴露为 public API，不在同一阶段反复改名。
- 没有 provenance 的实体不得进入 `NormalizedPidGeometry.entities`。

### Phase 2: Sheet text and coordinate geometry MVP

**Goal:** 把现有 Sheet probe evidence 转成第一批可显示实体。

**Files:**

- `pid-parse/src/parsers/sheet_probe.rs`
- `pid-parse/src/streams/cluster.rs`
- `pid-parse/src/geometry.rs`
- `pid-parse/src/cfb/reader.rs`

**Tasks:**

1. 将 `SheetGeometry.texts` 映射为 `PidGraphicKind::Text`。
2. 用已确认的 coordinate record 才生成 `Line` / `Polyline`。
3. 对仅有 `coordinate_hints` 的数据先输出 `Unknown` 或低置信度实体，不默认渲染。
4. 关联 `SheetEndpoint` 与 `PidRelationship`，为后续线段重建提供 endpoint provenance。
5. 为真实样本输出 geometry summary：
   - text entities
   - line/polyline entities
   - inferred / skipped entities

**Validation:**

```powershell
cd ..\pid-parse
cargo test --lib sheet geometry -- --nocapture
cargo test --test publish_a01_raw_residual -- --nocapture
```

**Stop Conditions:**

- 不能把任意相邻 `(i32, i32)` 都当成坐标渲染。
- 不能为了显示密度牺牲 byte provenance。

### Phase 3: PSM segment and endpoint reconstruction

**Goal:** 从拓扑连接升级为接近原图的线段/管线连接。

**Files:**

- `pid-parse/src/streams/psm_tables.rs`
- `pid-parse/src/parsers/psm_tables.rs`
- `pid-parse/src/crossref.rs`
- `pid-parse/src/layout.rs`
- `pid-parse/src/geometry.rs`

**Tasks:**

1. 深化 `PSMsegmenttable` record 结构。
2. 建立 segment -> Sheet endpoint -> relationship -> object 的来源链。
3. 对可确认的 segment 输出 `Line` / `Polyline`。
4. 将 `GraphicOID`、`DrawingID`、`field_x` 合并到 geometry provenance。
5. 保留 `layout.segments` 作为 fallback，而不是与真实 segment 混用。

**Validation:**

```powershell
cd ..\pid-parse
cargo test --lib psm crossref geometry -- --nocapture
cargo test --test publish_meta_parity -- --nocapture
```

**Stop Conditions:**

- 如果 segment table 语义不能稳定复现，先以 `Inferred` 输出，不让 H7CAD 默认显示。
- 不在 H7CAD 内部重做 segment 推导；推导必须在 `pid-parse` 归一化。

### Phase 4: Symbol instance projection

**Goal:** 让设备、阀门、仪表等对象至少有可识别符号位置。

**Files:**

- `pid-parse/src/streams/jsite.rs`
- `pid-parse/src/crossref.rs`
- `pid-parse/src/geometry.rs`
- `H7CAD/src/io/pid_import.rs`

**Tasks:**

1. 从 `JSite` / `symbol_path` / `GraphicOID` 建立 symbol usage 到 drawing object 的映射。
2. `PidGraphicKind::SymbolInstance` 输出：
   - insertion
   - rotation
   - scale
   - symbol_path
   - drawing_id / graphic_oid
3. H7CAD 第一版渲染为 named block placeholder：
   - 根据 symbol basename 建 block 名
   - 未能解析 `.sym` 时画简化 glyph
   - 保留 xdata / index，方便属性面板定位
4. 后续再考虑解析 `.sym` 文件或接入符号库。

**Validation:**

```powershell
cd ..\pid-parse
cargo test --lib jsite crossref geometry -- --nocapture
cd ..\H7CAD
cargo test -p H7CAD --bin H7CAD pid_import -- --nocapture
```

**Stop Conditions:**

- 不把同一 symbol_path 的所有 usage 渲染在同一个位置。
- 没有 insertion 坐标时只进入诊断列表，不进入主图层。

### Phase 5: H7CAD real geometry rendering path

**Goal:** H7CAD 优先显示真实 PID 几何，fallback 到旧拓扑预览。

**Files:**

- `H7CAD/src/io/pid_import.rs`
- `H7CAD/src/app/update.rs`
- `H7CAD/src/app/document.rs`
- `H7CAD/src/scene/mod.rs`
- `H7CAD/src/io/pid_screenshot.rs`

**Tasks:**

1. 新增 `pid_geometry_to_native_doc(geometry, doc) -> (CadDocument, PidPreviewIndex)`。
2. 新增图层：
   - `PID_GEOM_LINES`
   - `PID_GEOM_TEXT`
   - `PID_GEOM_SYMBOLS`
   - `PID_GEOM_INFERRED`
   - `PID_DIAGNOSTICS`
3. `open_pid` 中选择渲染策略：
   - 有 high-confidence geometry -> real geometry
   - 无 high-confidence geometry -> existing topology preview
4. 更新 fit 逻辑：
   - PID real geometry tab 优先 fit `PID_GEOM_`
   - fallback preview 继续 fit `PID_OBJECTS_` / `PID_LAYOUT_TEXT` / `PID_RELATIONSHIPS`
5. `PidPreviewIndex` 同时支持 `drawing_id` 和 `graphic_oid` 反查。

**Validation:**

```powershell
cargo test -p H7CAD --bin H7CAD pid_import -- --nocapture
cargo test -p H7CAD --bin H7CAD pid_screenshot -- --nocapture
cargo check --locked --workspace --all-targets
```

**Stop Conditions:**

- 不能破坏现有 `.pid` metadata edit / save path。
- 不能让诊断面板图层影响主图 `fit`.
- 不能在 native 和 compat preview 之间产生实体计数明显不一致而无测试解释。

### Phase 6: Golden screenshot and regression gate

**Goal:** 用稳定截图验证“显示真实图形”没有退化。

**Files:**

- `H7CAD/src/io/pid_screenshot.rs`
- `H7CAD/src/app/commands.rs`
- `H7CAD/tests/` 或 crate 内 test module

**Tasks:**

1. 为目标样本生成 deterministic PNG。
2. 建立轻量图片断言：
   - 非空白像素数量
   - 主图 bbox 合理
   - 文本/线段图层至少各有实体
3. 可选：保存人工审核 baseline。
4. 命令层保留 `PIDSHOT <path.png>` 作为手动确认入口。

**Validation:**

```powershell
cargo test -p H7CAD --bin H7CAD pid_screenshot -- --nocapture
cargo test -p H7CAD --bin H7CAD open_target_pid_sample -- --nocapture
```

**Stop Conditions:**

- 不用窗口截图作为唯一回归依据。
- 如果 PNG 因字体或平台差异抖动，优先断言几何/像素统计，不直接做整图 byte compare。

## 4. Acceptance Criteria

第一阶段交付可以视为完成，当且仅当：

1. 目标 `.pid` 文件在 H7CAD 中仍走正常 `open_path -> FileOpened -> PID tab` 链路。
2. `PidOpenBundle.native_preview` 中真实几何图元数量大于 fallback topology 图元数量，或至少主图层来自 `NormalizedPidGeometry`。
3. 主视口 fit 到真实图形层，而不是 metadata / fallback / stream 诊断层。
4. 属性面板 / PID browser 仍能通过 `drawing_id` 或 `graphic_oid` 定位图元。
5. `pid-parse` 对所有真实几何实体都能说明 source stream 和 byte/provenance。
6. 目标样本测试和截图统计测试通过。

## 5. Risk Register

| Risk | Impact | Mitigation |
|---|---|---|
| Sheet 坐标 heuristic 误判 | 画出错误图形 | 低置信度实体默认不渲染，只进诊断 |
| PSMsegmenttable 语义不稳定 | 管线连接错乱 | 先 golden summary，再进 H7CAD 主图 |
| `.sym` 符号格式未解析 | 设备/阀门外观不完整 | 第一版用 block placeholder，保留 symbol_path |
| 装饰层压缩主图视口 | 用户看到很小的主图 | fit 只匹配 `PID_GEOM_` 主图层 |
| 破坏 PID metadata save | 现有命令回归 | 不改 `PidPackage` raw stream cache 语义，增加 targeted tests |
| DTO 过早承诺错误字段 | public API 负担 | 所有不确定字段放 provenance / inferred / unknown |

## 6. Recommended First PR Split

### PR 1: `pid-parse` geometry DTO only

- 新增 DTO 和 schema
- `build_normalized_geometry` 返回空或仅 diagnostic
- 不改 H7CAD 行为

### PR 2: Sheet text geometry MVP

- Text entity 从 Sheet text runs 输出
- 真实样本 summary test
- H7CAD 仍不默认消费

### PR 3: H7CAD geometry projection fallback path

- `pid_geometry_to_native_doc`
- 若 geometry 非空则渲染 `PID_GEOM_*`
- fallback 旧 preview

### PR 4: Segment / symbol incremental fidelity

- PSM segment line/polyline
- Symbol placeholder
- 更新 screenshot regression

## 7. Commands

常用验证命令：

```powershell
cd D:\work\plant-code\cad\pid-parse
cargo test --locked --workspace --all-targets
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo fmt --all -- --check

cd D:\work\plant-code\cad\H7CAD
cargo test -p H7CAD --bin H7CAD pid_import -- --nocapture
cargo test -p H7CAD --bin H7CAD pid_screenshot -- --nocapture
cargo check --locked --workspace --all-targets
```

## 8. Open Questions

- 目标用户是否要求“视觉接近 SmartPlant 原图”，还是“几何位置和对象关系正确”优先？
- 是否能取得对应 `.sym` 符号库？如果不能，第一版必须明确是 placeholder。
- 是否存在多个真实 `.pid` 样本可用于交叉验证 Sheet/PSM record 语义？
- 后续是否需要把真实 geometry 输出回 Publish XML / DXF / SVG，还是只用于 H7CAD display？
