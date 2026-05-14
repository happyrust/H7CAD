# PID 真实图形显示 Brainstorm 开发方案

> **日期**: 2026-05-07
> **状态**: 方案制定中
> **范围**: H7CAD 加载/打开 `.pid` 文件并显示真实 P&ID 图纸的端到端能力闭环
> **基线**: `main` @ `361602df`，`upstream/main` @ `048bb41a`（落后 7 commits，待合并）
> **前置依赖**: 已完成 PID 加载缺口分析（见 §2）

---

## 1. 背景

H7CAD 的 `.pid` 路径目前可以"打开 + 看见拓扑示意图"，但**看不到原图**。当前现状：

- `.pid` 入口完整：file picker / `open_document_blocking` / `pid_import::open_pid` 三段贯通
- 解析层完整：`PidParser::parse_package` 已能读 CFB / TaggedTxtData / DocVersion / AppObject / JSite / Dynamic Attributes / Cross-reference / object graph
- 预览层完整但只是**拓扑启发式**：`derive_layout` + `pid_document_to_preview` 把对象关系铺成网格图
- 真实几何只接了**一种**：Sheet `coordinate_hints` / `object_geometry_hints` → `Inferred Point` → `PID_GEOM_POINTS`
- 保存 round-trip 完整：`PidWriter` + sidecar copy
- Metadata 编辑、verify、diff、health 命令族完整

但用户打开 `.pid` 后看到的是网格化的对象关系图，不是 SmartPlant 原始图纸。

上游 `HakanSeven12/H7CAD` 同期推进了 7 个 commit（详见 §3），本地 fork 与上游已大幅分歧。

---

## 2. 目标

把 PID 加载 → 显示链路从"拓扑预览"升级到"真实几何渲染"，**而不是**在 H7CAD 侧美化启发式图。

### 主目标

1. `pid-parse` 从真实 Sheet 字节里产出 `Line`/`Text`/`SymbolInstance` 等真实图元（带 source-proven provenance）
2. H7CAD 把真实几何渲染到独立 `PID_GEOM_*` 图层
3. 主视口 fit 到真实几何层，不再被装饰层吞没
4. PID 打开能像 DXF 一样输出 `OpenNotice` 诊断
5. 保留拓扑预览作为 fallback，不破坏现有 metadata edit / save 链路

### 验收标准

- 真实样本（`工艺管道及仪表流程-1.pid` 或同等 fixture）打开后主图来自 `NormalizedPidGeometry`
- 真实几何实体数 > fallback topology 实体数
- `cargo test -p H7CAD pid_import` / `pid_screenshot` 通过
- `cargo test -p pid-parse --lib geometry` 锁定新 promotion gate
- `git diff --check` 无 whitespace 错误

---

## 3. 上游合并影响

`upstream/main` 比 `main` 多 7 个 commit，全部触及 scene/render：

| Commit | 含义 | 与 PID 工作的关系 |
|---|---|---|
| `0e5dec09` annotation scale 系统 | CANNOSCALE / PSLTSCALE / MultiLeader / Leader | PID 文本 / Symbol 渲染时可能复用 |
| `b83c982b` hatch pattern scale + per-entity annotative | hatch 修正 | 间接，PID 当前不出 hatch |
| `45f9f96c` L–S perf | 渲染管线性能 | 可能与本地 native render path 冲突 |
| `2da3640b` 缓存离开 UI 线程 | hatch / image / mesh cache 异步化 | **直接冲突风险**：本地 PID populate_hatches/images/meshes 走 UI 线程 |
| `d30f80f9` dead_code suppress | 微调 | 无 |
| `ca9ec8f4` 缓存赋值后 epoch bump | scene cache bug fix | **直接冲突风险**：与本地 scene/mod.rs 修改重叠 |
| `048bb41a` PolyfaceMesh / PolygonMesh solid fill | mesh 渲染 | 间接 |

### 合并冲突现状

试合并已 abort（已恢复 stash）。冲突文件 25+ 个，关键：

- `src/scene/mod.rs`：本地 4742 行差异 vs upstream 1234 行新增（**最大冲突点**）
- `src/scene/render.rs`、`src/scene/tessellate.rs`、`src/scene/pipeline/face3d_gpu.rs`
- `src/app/mod.rs` / `update.rs` / `view.rs`
- `src/io/mod.rs`
- 多个 entity 文件（arc/circle/line/lwpolyline/polyline/ray/shape/solid）
- 多个 annotate / draw 模块
- `Cargo.lock` / `Cargo.toml`

### 合并策略候选

需用户拍板，三选一：

**路线 α（推荐）：scene/render 域逐文件 cherry-pick**
- 只取 `ca9ec8f4`（cache epoch fix）和 `2da3640b`（cache off-thread）的渲染相关 hunk
- 跳过 annotation scale 系统（与 PID 不直接相关，可下一轮再合）
- 优点：风险可控，PID 工作可继续
- 缺点：人工切片，需 1-2 小时审 hunk

**路线 β：完整 merge upstream/main 并人工解冲突**
- 一次性合 7 个 commit
- 优点：分歧不再扩大
- 缺点：25+ 文件冲突，scene/mod.rs 单文件已是 5976 行级差异，预计半天到一天，且影响 PID 工作节奏

**路线 γ：暂不合并，PID 工作在当前 main 上推进**
- 把 upstream merge 列为独立任务，由专门 PR 处理
- 优点：PID 工作零阻塞
- 缺点：分歧继续累积，annotation scale 与 PID 文本 placement 可能将来重做

> **本方案默认按路线 γ 推进**，把上游合并独立成 §11 Track-2 任务。如果用户选 α/β，§4 切片不变，仅在 Slice 0 之前插入 merge 阶段。

---

## 4. 候选路线

### 路线 A：Parser-first（pid-parse 先出真实几何）

先在 `pid-parse` 把 `Line` / `Text` / `SymbolInstance` 从 Sheet 字节里 promote 出来，H7CAD 等 DTO 出现后再接渲染。

优点：
- 符合现有 Probe/Decode 分层铁律
- H7CAD 改动小，等 DTO 定型再接
- promotion gate 在 `pid-parse` 一处把关，避免噪声扩散

缺点：
- 阻塞在 Sheet record grammar 逆向（Phase 9B）
- fixture 仅 5 个，promotion gate 仍 `text_over_threshold=0` / `identity_over_threshold=0`
- 用户短期看不到主图改善

### 路线 B：Renderer-first（H7CAD 先把 DTO 接全）

先把 `add_geometry_entities` 扩出 `Line` / `Text` / `SymbolInstance` 分支 + 新建 `PID_GEOM_LINES` / `_TEXT` / `_SYMBOLS` 图层 + 改 fit 优先级。即便 `pid-parse` 暂时只出 `Point`，H7CAD 先就绪。

优点：
- H7CAD 不阻塞，可独立推进
- 改动局部，集中在 `src/io/pid_import.rs` 和 `src/app/update.rs`
- 一旦 `pid-parse` 出 Line/Text，立即可见

缺点：
- 短期看不到行为变化（DTO 还是空）
- 渲染分支可能"先建好但用不到"

### 路线 C：Notice-first（先把诊断通道接通）

先做 `pid_summary_to_notices` + `load_pid_native_with_notices`，让 PID 打开时出 `OpenNotice`，揭示哪些子系统已就绪、哪些 unresolved。

优点：
- 用户可见反馈最快
- 设计已就绪（`docs/plans/2026-04-25-cli-pid-input-plan.md`）
- 改动最小（io/diagnostics.rs + io/mod.rs + io/pid_import.rs）

缺点：
- 不解决"看不到原图"的核心问题
- 只是把已知信息搬到 UI

### 路线 D：Fixture-first（先扩 fixture）

先收集真实 `.pid` 样本扩到 8-12 个，让 `pid-parse` Phase 9A baseline 能解锁后续 promotion gate 调整。

优点：
- 是 Phase 9B/9C/9D 的硬前置
- 一次投入长期受益

缺点：
- 需用户/外部协作获取私有真实样本
- 短期不出可见变化

---

## 5. 推荐方案

推荐 **B + C + D 并行，A 独立 track**：

1. **Track-1（H7CAD 内可独立完成）**：路线 B + C
   - Slice 1：`pid_summary_to_notices` 接入（Notice-first）
   - Slice 2：`add_geometry_entities` 扩 Line/Text/Symbol 分支 + 新图层 + fit 优先级（Renderer-first，DTO 暂时为空也无害）
   - Slice 3：UTF-16 metadata 支持（顺手收口）
2. **Track-2（外部依赖）**：路线 D
   - Slice 4：fixture 扩容
3. **Track-3（pid-parse 内深水）**：路线 A
   - Slice 5：Sheet record grammar 逆向（Phase 9B）
   - Slice 6：Text/Symbol promotion gate（Phase 9D）

理由：
- Track-1 完全无外部依赖，立即可推
- Track-2 是 Track-3 的硬前置，越早启动越好
- Track-3 最深，需要 fixture 支撑，节奏独立
- 三 Track 并行避免互相阻塞

---

## 6. 非目标

明确不在本方案范围：

- 不重写 `pid-parse` Probe/Decode 双层模型
- 不一次性实现完整 SmartPlant 符号库 `.sym` 解析
- 不做完整 H7CAD scene 渲染管线重构
- 不在 promotion gate 未达标时让 H7CAD 默认渲染未证明的 Line/Text
- 不做 DWG / Publish XML 闭环（独立 track）
- 不自动提交 vendor_tmp / clippy_full.log / 工具目录
- **不在没有用户授权时合并 upstream 或 push 任何分支**

---

## 7. 执行切片（Track-1 优先）

### Slice 1：PID 打开输出 OpenNotice

**目标**：`load_file_native_blocking` 与 `open_document_blocking` 对 `.pid` 输出 `Vec<OpenNotice>` 而不是 `Vec::new()`，把 `PidImportSummary` 里 unresolved / object_graph_available / cluster_count 等转成 UI 可见诊断。

**预期实现**：

- 在 `src/io/diagnostics.rs` 新增 `from_pid_import_summary(summary: &PidImportSummary) -> Vec<OpenNotice>`
- 在 `src/io/pid_import.rs` 新增 `pub fn load_pid_native_with_notices(path: &Path) -> Result<(nm::CadDocument, Vec<OpenNotice>), String>`
- `src/io/mod.rs::load_file_native_blocking` 的 `.pid` 分支改用 `load_pid_native_with_notices`
- `src/io/mod.rs::open_document_blocking` 的 `OpenedDocument::Pid` 分支返回 notice vec 而非 `Vec::new()`
- 老 `load_pid_native` 保留向后兼容

**TDD 测试**：

```powershell
cargo test -p H7CAD io::tests::pid_open_path_surfaces_unresolved_relationship_notice
cargo test -p H7CAD io::diagnostics::tests::from_pid_import_summary_emits_unresolved_notice
```

**验收**：

- 真实样本打开时命令行至少多一行 `已恢复错误 / 警告` 类提示
- DXF/DWG 现有 notice 行为不变
- 老 `load_pid_native` 调用方（xref / fallback）不破坏

**风险**：

- `OpenNotice` 结构当前是字符串化，PID 可能要新增 source 类型枚举值
- `NoticeCounts::summary_zh` 需测试 PID 数据是否要改写

---

### Slice 2：H7CAD 渲染层接全 PidGraphicKind

**目标**：`add_geometry_entities` 不再只 match `Point`，而是按 kind 分别渲染到 `PID_GEOM_LINES` / `PID_GEOM_TEXT` / `PID_GEOM_SYMBOLS` / `PID_GEOM_INFERRED` 图层，并把这些图层加入 `pid_main_layers` fit 列表。

**预期实现**：

- `src/io/pid_import.rs::add_geometry_entities` 扩 match：
  - `PidGraphicKind::Line { start, end }` → `nm::EntityData::Line` 到 `PID_GEOM_LINES`
  - `PidGraphicKind::Polyline { points, closed }` → `nm::EntityData::LwPolyline` 到 `PID_GEOM_LINES`
  - `PidGraphicKind::Arc { ... }` → `nm::EntityData::Arc` 到 `PID_GEOM_LINES`
  - `PidGraphicKind::Circle { ... }` → `nm::EntityData::Circle` 到 `PID_GEOM_LINES`
  - `PidGraphicKind::Text { ... }` → `nm::EntityData::Text` 到 `PID_GEOM_TEXT`
  - `PidGraphicKind::SymbolInstance { ... }` → `nm::EntityData::Insert` (named-block placeholder) 到 `PID_GEOM_SYMBOLS`
  - `Point` 现有逻辑保留到 `PID_GEOM_POINTS`
  - `Unknown` → 不渲染，可选写入 inferred 图层
- 渲染按 `confidence` 过滤：默认只渲染 `Decoded` 和 `Inferred`（`field_x.is_some()`），`ProbeOnly` 不渲染
- 新增 `ensure_layer` 调用：`PID_GEOM_LINES`(color 1) / `PID_GEOM_TEXT`(color 7) / `PID_GEOM_SYMBOLS`(color 4) / `PID_GEOM_INFERRED`(color 8)
- `src/app/update.rs::pid_main_layers` 改为 `["PID_GEOM_", "PID_OBJECTS_", "PID_LAYOUT_TEXT", "PID_RELATIONSHIPS"]`，让真实几何层优先 fit
- `preview_index` 仍记录 drawing_id / graphic_oid 反查

**TDD 测试**：

```powershell
cargo test -p H7CAD io::pid_import::tests::add_geometry_entities_renders_line_kind_to_geom_lines_layer
cargo test -p H7CAD io::pid_import::tests::add_geometry_entities_renders_text_kind_to_geom_text_layer
cargo test -p H7CAD io::pid_import::tests::add_geometry_entities_skips_probe_only_entities
cargo test -p H7CAD app::update::tests::pid_open_fits_geom_layer_when_present
```

**验收**：

- 用 synthetic `NormalizedPidGeometry` fixture（手工构造 `Line(Decoded)` 和 `Text(Decoded)`）测试能渲染到对应图层
- `PidPreviewIndex.handles_for_drawing_id` 仍能反查
- 真实样本（DTO 暂时为空）行为不变，主图仍 fit 到 `PID_OBJECTS_*`

**风险**：

- Symbol placeholder 用什么 block？建议沿用 `SPPID_BRAN` 或新增 `PID_SYMBOL_PLACEHOLDER`，`ensure_sppid_bran_block_library` 那一套可借鉴
- Text 高度 / 旋转单位（弧度 vs 度）需对齐 DTO 文档（`PidGraphicKind::Text` 注释说 rotation in radians）

---

### Slice 3：UTF-16 / BOM 元数据支持

**目标**：`edit_pid_drawing_attribute` / `edit_pid_general_element` / `read_pid_*` / `list_pid_metadata` 不再硬拒 BOM / UTF-16 流。

**预期实现**：

- 新增 `src/io/pid_import.rs::decode_xml_bytes(bytes: &[u8]) -> Result<(String, XmlEncoding), String>` 处理 BOM 检测、UTF-16 LE/BE、UTF-8 fallback
- 写回时用相同编码序列化（保 round-trip）
- `replace_stream` 后 `pid_package_store::cache_package` 行为不变

**TDD 测试**：

```powershell
cargo test -p H7CAD io::pid_import::tests::edit_drawing_attribute_handles_utf16_le_bom
cargo test -p H7CAD io::pid_import::tests::edit_general_element_handles_utf8_bom
```

**验收**：

- 已有 UTF-8 用例不退化
- 真实 `.pid`（多为 UTF-16）能成功执行 PIDSETPROP 命令

**风险**：

- `pid_parse::writer::set_drawing_attribute` 当前签名假设 UTF-8 字符串，可能要改为 byte-level 或在调用前后重编码

---

### Slice 4：fixture 扩容（Track-2，外部依赖）

**目标**：`pid-parse` `geometry_fixture_cases()` registry 从 5 扩到 8-12 个真实 `.pid` fixture。

**实施**：

- 收集 SmartPlant 真实样本（外部资源，需人工）
- 加入 `pid-parse/test-file/`
- 更新 `tests/parse_real_files.rs::geometry_fixture_cases()`
- 让 `geometry_fixture_availability_summary` 输出 `available >= 8`

**验收**：

- `cargo test --test parse_real_files geometry_fixture_availability_summary` 通过且 `available_count >= 8`

**风险**：

- 真实 `.pid` 通常含敏感工程信息，需脱敏或私有 fixture 路径
- 不是技术阻塞而是流程阻塞

---

### Slice 5：Sheet record grammar 逆向（Track-3，pid-parse 深水）

**目标**：让 `pid-parse::build_normalized_geometry` 至少能在一种 record shape 上 promote 出真实 `Line` 或 `Text`。

**前置**：Slice 4 完成，`identity_over_threshold > 0` 或 `text_quality_passed > 0`。

**实施路线**：见 `pid-parse` Phase 9B/9C/9D 计划。本 H7CAD 方案不展开。

**信号**：当 `pid-parse` 测试报告 `geometry_entities_with_kind_line >= 1` 时，Slice 2 的渲染层立即生效。

---

### Slice 6：golden screenshot 回归门

**目标**：建立 PID 真实几何 PNG 像素统计回归。

**前置**：Slice 5 出真实 Line/Text。

**预期实现**：

- 复用 `src/io/pid_screenshot.rs` deterministic PNG path
- 新增 `tests/pid_real_geometry_screenshot.rs`：
  - 非空白像素数下限
  - 主图 bbox 合理
  - text/line 图层各至少有像素
- 不做整图 byte compare（字体 / 平台抖动）

**验收**：

- `cargo test pid_real_geometry_screenshot` 通过且 baseline 稳定 5 次

---

## 8. 主要文件

预期高频文件：

- `src/io/pid_import.rs`（Slice 1 / 2 / 3 主战场）
- `src/io/mod.rs`（Slice 1 dispatch）
- `src/io/diagnostics.rs`（Slice 1 from_pid_import_summary）
- `src/app/update.rs`（Slice 2 fit_layers_matching）
- `src/io/pid_screenshot.rs`（Slice 6）

只在必要时触碰：

- `src/scene/mod.rs`（合并冲突源，Slice 2 可能要测 layer 渲染但避免改 scene）
- `crates/h7cad-native-model/`（除非 Symbol Insert 缺字段）
- `pid-parse/src/geometry.rs`（Track-3，独立 PR）

---

## 9. 测试策略

每个 Slice 走 TDD：

1. RED：写最小失败测试，断言期望的新行为
2. GREEN：最小实现让测试通过
3. REGRESS：跑相邻测试集合
4. WORKSPACE：阶段末 `cargo check --workspace` + `cargo test --workspace`
5. LINT：`cargo fmt --check` + `git diff --check`

阶段命令：

```powershell
# Slice 1
cargo test -p H7CAD io::diagnostics
cargo test -p H7CAD io::tests::pid_open_path
cargo test -p H7CAD io::pid_import::tests

# Slice 2
cargo test -p H7CAD io::pid_import::tests::add_geometry_entities
cargo test -p H7CAD app::update::tests::pid_open_fits

# Slice 3
cargo test -p H7CAD io::pid_import::tests::edit_drawing_attribute
cargo test -p H7CAD io::pid_import::tests::edit_general_element

# 阶段末
cargo fmt --all -- --check
cargo check --locked --workspace --all-targets
cargo test --locked --workspace
```

---

## 10. 风险登记

| 风险 | 影响 | 缓解 |
|---|---|---|
| upstream merge 冲突半天起步 | 阻塞 Slice 1+ | 选路线 γ：合并独立 track，PID 工作不阻塞 |
| Slice 2 渲染分支建好但 DTO 长期为空 | 看不到行为变化 | synthetic fixture 测试覆盖；Track-3 推进时立即生效 |
| `OpenNotice` 字符串化不够表达 PID 多 summary 字段 | UI 信息密度差 | 第一版先转 1-2 条 high-signal 诊断，避免噪声 |
| Symbol placeholder block 选择影响 round-trip | DXF 导出可能丢符号 | 用 `PID_SYMBOL_PLACEHOLDER` 独立命名，不污染 SPPID_BRAN |
| UTF-16 编辑 round-trip 字节不一致 | metadata edit 破坏 byte parity | Slice 3 必须配 round-trip 测试 |
| fixture 私有性 | Slice 4 阻塞 | 同步申请脱敏样本 / 接受当前 5 fixture 推 Track-3 |
| upstream annotation scale 与 PID 文本系统将来撞车 | Track-3 重做 text rendering | Slice 6 之前合 upstream，让 annotation scale 先就位 |

---

## 11. Track 划分与并行节奏

```
Track-1 (本仓 H7CAD)              Track-2 (fixture)           Track-3 (pid-parse)
┌────────────────────────────┐   ┌──────────────────┐        ┌─────────────────────┐
│ Slice 1: notice 通道       │   │ Slice 4: fixture │ ──────►│ Slice 5: grammar    │
│ Slice 2: 渲染层接全 kind   │   │ 扩到 8-12        │        │ Slice 6 前置: text/ │
│ Slice 3: UTF-16 metadata   │   └──────────────────┘        │ symbol promotion    │
└────────────────────────────┘                                 └─────────────────────┘
        │                                                              │
        ▼                                                              ▼
   ┌───────────────────────────────────────────────────────────────────────┐
   │ Slice 6: H7CAD golden screenshot 回归（DTO 出真实 Line/Text 后启用）  │
   └───────────────────────────────────────────────────────────────────────┘

并行 Track-α (上游合并，需用户拍板)
┌────────────────────────────────────────────────┐
│ 候选: cherry-pick ca9ec8f4 + 2da3640b 渲染修复  │
│ 候选: 完整 merge upstream/main + 解 25+ 冲突   │
│ 候选: 暂不合并                                  │
└────────────────────────────────────────────────┘
```

---

## 12. 决策日志

| 决策 | 理由 |
|---|---|
| 不在 H7CAD 侧美化启发式图 | 美化 = 把错的画得更清楚；真实几何必须从 `pid-parse` 出 |
| Notice / 渲染层 / UTF-16 三 Slice 并入 Track-1 | 全部本仓内可完成，互不阻塞 |
| fixture 扩容独立成 Track-2 | 需要外部样本，节奏不同 |
| Sheet record grammar 留在 `pid-parse` | 符合 Probe/Decode 分层铁律 |
| 上游合并默认走路线 γ | 25+ 冲突 + scene/mod.rs 巨型差异，PID 工作不应阻塞在合并 |
| 默认只渲染 `Decoded` + `Inferred(field_x)` | 防止 ProbeOnly 噪声进主图 |
| Symbol 第一版用 named-block placeholder | `.sym` 解析独立 track，不阻塞主链 |
| 不在本方案处理 .pid → Publish XML / DWG 闭环 | 独立 PR/track |

---

## 13. Open Questions（待用户确认）

1. **上游合并策略**：α/β/γ 三选一？默认 γ。
2. **fixture 扩容**：是否有渠道获取 3-7 个新真实 `.pid` 样本？
3. **Symbol placeholder block 名**：`PID_SYMBOL_PLACEHOLDER` 还是按 `symbol_path` basename 动态命名？
4. **Notice 文案**：第一版只输出 unresolved 关系数 1 条，还是 cluster / object_graph_available / sheet count 全部 5+ 条？
5. **fit 优先级**：`PID_GEOM_*` 完全替换 `PID_OBJECTS_*` 还是叠加？真实几何缺失时仍要 fallback 到拓扑层。

---

## 14. 命令速查

```powershell
# 启动新切片
cd D:\work\plant-code\cad\H7CAD

# 测试当前 PID 路径
cargo test -p H7CAD io::pid_import
cargo test -p H7CAD io::tests
cargo test -p H7CAD io::pid_screenshot

# 阶段末验证
cargo fmt --all -- --check
cargo check --locked --workspace --all-targets
cargo test --locked --workspace
git diff --check

# pid-parse 跨仓验证（Track-3）
cd D:\work\plant-code\cad\pid-parse
cargo test --locked --workspace --all-targets
cargo clippy --locked --workspace --all-targets -- -D warnings
```
