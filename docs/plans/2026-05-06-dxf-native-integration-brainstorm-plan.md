# DXF Native Integration 后续开发方案

> **日期**: 2026-05-06
> **范围**: H7CAD DXF 打开/显示/保存链路的 native-first 收口。
> **当前状态**: DXF read/write 主链路已走 `h7cad-native-dxf` + `h7cad-native-model`；运行时仍保留 `acadrust` compat 投影。
> **本文件目标**: 用 brainstorm 方式收敛下一阶段开发路线，并拆成可独立验证的小任务。

---

## 1. 当前事实

H7CAD 的 DXF 结合方式已经不是“外部导入插件”，而是主运行时的一条 native-first 数据链：

1. `.dxf` 打开进入 `src/io/mod.rs::load_dxf_native_blocking`。
2. `h7cad_native_dxf::read_dxf_bytes` 生成 `h7cad_native_model::CadDocument`。
3. `native_bridge::native_doc_to_acadrust` 生成 compat 显示投影。
4. `Scene.native_store` 保存 native 真源；`Scene.document` 暂时保存 compat 投影。
5. 保存时优先走 `NativeStore::save` -> `io::save_native` -> `save_dxf` -> `h7cad_native_dxf::write_dxf`。

已完成的补强：

- DXF 打开会对 `Unknown` / `ProxyEntity` / unknown object / proxy object 生成 `OpenNotice::Warning`。
- native render 已增加 `Ellipse` 和 `Spline` 转换资格。
- 已补 `load_file_native_blocking` 真实 `.dxf` open-path 测试。
- `cargo test --workspace` 通过。

---

## 2. Brainstorm: 三条可选路线

### 路线 A: 继续扩大 native render 覆盖

目标是让更多 DXF 实体绕过 compat 投影，直接从 native model 渲染。

优点：

- 对“移除 acadrust”长期目标最直接。
- 每多支持一个实体，native-first 链路就更可信。
- 可按实体类型逐个小 PR 推进。

缺点：

- 不是所有实体都适合直接加入 supported list。
- 例如 `Polyline` 包含 bulge、mesh、polyface 语义，若 native tessellation 没完整支持就会覆盖 compat fallback，导致显示保真下降。
- 需要逐类实体确认“native 渲染不比 compat 差”。

### 路线 B: 优先做 fidelity gaps

目标是修真实 DXF 中最容易被用户看见的显示偏差：OCS/WCS、ByBlock 递归继承、宽度、Spline weights、MINSERT、Dimension 样式等。

优点：

- 用户感知强。
- 能直接改善真实工程图打开效果。
- 适合围绕 fixture 和 round-trip 建测试矩阵。

缺点：

- 涉及面比单实体 native conversion 大。
- 有些问题是系统性改动，例如 OCS/WCS 会触碰多类实体。

### 路线 C: 先做可观测性和诊断面板

目标是把 native/compat/fallback 的选择透明化，让打开 DXF 后能看到哪些内容 native 渲染、哪些 fallback、哪些 preserved-only。

优点：

- 风险最低。
- 对调试真实 DXF 很有价值。
- 可以指导后续实体覆盖优先级。

缺点：

- 不直接改善几何显示。
- 容易变成工具层，不解决根因。

## 推荐路线

推荐 **B 优先，C 辅助，A 谨慎推进**。

原因：当前 DXF 文件层已基本闭环，瓶颈不在“能不能读写”，而在“真实图纸打开后显示是否可信”。单纯扩大 native render supported list 有风险，必须先按 fidelity gaps 建测试和改造。可观测性作为辅助，用来解释 fallback 和 preserved-only 内容。

---

## 3. 开发阶段

### Phase 0: 保持当前 DXF diagnostics 变更可交付

目标：把本轮已做的 diagnostics + Ellipse/Spline native conversion 整理成可 review 状态。

文件：

- `src/io/diagnostics.rs`
- `src/io/mod.rs`
- `src/scene/acad_to_truck.rs`
- `src/scene/mod.rs`
- `crates/h7cad-native-dxf/tests/entity_2d_roundtrip.rs`（已有测试，只运行不改）

验收：

```powershell
cargo test -p H7CAD io::diagnostics
cargo test -p H7CAD scene::acad_to_truck
cargo test -p H7CAD io::tests::dxf_open_path_surfaces_unknown_entity_notice
cargo test -p h7cad-native-dxf --test entity_2d_roundtrip
cargo test --workspace
rustfmt --edition 2021 --check src/io/diagnostics.rs src/io/mod.rs src/scene/acad_to_truck.rs src/scene/mod.rs
```

备注：

- `cargo fmt --check` 当前会在非本轮文件 `src/modules/registry.rs` 上失败；除非单独开格式清理任务，否则不把它混入本轮 DXF patch。

### Phase 1: DXF fallback 可观测性

目标：打开 DXF 后记录 native render 和 compat fallback 的实体类型统计，帮助决定后续优先级。

方案：

1. 在 `Scene` 或 render build 阶段收集每帧/每次 geometry rebuild 的统计。
2. 统计字段：
   - native-rendered entity type counts
   - compat-rendered entity type counts
   - skipped/preserved-only entity type counts
3. 首期只写 debug/test helper，不直接做 UI 面板。
4. 后续再接 command line 或 diagnostics panel。

候选文件：

- `src/scene/mod.rs`
- `src/scene/render.rs`
- `src/io/diagnostics.rs`
- `src/app/update.rs`

测试：

- 构造 native doc：`Line` + `Ellipse` + `Unknown`。
- 打开/构造 scene 后统计应显示：
  - `Line/Ellipse` 可 native render
  - `Unknown` preserved-only
- 保证统计不会影响实际 `wires_for_block` 输出。

### Phase 2: OCS/WCS 显示保真

目标：修 normal/extrusion 不为 `(0,0,1)` 时的显示偏差。

当前已知：

- `h7cad-native-model` 已有 `geom_ocs`，暴露 `arbitrary_axis`、`ocs_to_wcs`、`ocs2d_to_wcs`。
- `Spline` native conversion 已开始使用 `entity.extrusion`。
- 仍需逐实体确认 `Circle/Arc/Ellipse/LwPolyline/Polyline/Hatch/Leader/MLine/Dimension/Insert` 的 OCS 处理是否完整。

推荐执行顺序：

1. 先写测试，不改实现：
   - tilted circle
   - tilted arc
   - tilted lwpolyline
2. 对已有正确实体加 regression。
3. 对失败实体逐个改 `to_truck_with_normal` 或 tessellation path。
4. 每类实体单独提交。

候选文件：

- `crates/h7cad-native-model/src/geom_ocs.rs`
- `src/entities/circle.rs`
- `src/entities/arc.rs`
- `src/entities/ellipse.rs`
- `src/entities/lwpolyline.rs`
- `src/entities/polyline.rs`
- `src/scene/acad_to_truck.rs`
- `src/scene/tessellate.rs`

测试：

```powershell
cargo test -p H7CAD scene::acad_to_truck
cargo test -p H7CAD ocs
cargo test -p h7cad-native-dxf --test entity_2d_roundtrip
```

### Phase 3: ByBlock 嵌套继承

目标：三层以上 nested INSERT 中，颜色、线型、线宽的 ByBlock 继承链不断裂。

设计：

- 引入一个显式 `BlockChainStyle`。
- 每层 INSERT 解析自己的 common style。
- 子实体若为 ByBlock，则继承当前 chain style。
- 子 INSERT 若自己覆盖颜色/线型/线宽，则更新 chain 后再递归。

候选文件：

- `src/scene/render.rs`
- `src/scene/mod.rs`
- `src/scene/tessellate.rs`

测试：

- `A(red) -> B(ByBlock) -> C(ByBlock) -> LINE(ByBlock)` 最终 red。
- `A(red) -> B(blue) -> C(ByBlock) -> LINE(ByBlock)` 最终 blue。
- linetype 和 lineweight 同样覆盖。

### Phase 4: 高风险实体专项

目标：只处理“真实 DXF 常见且当前保真不足”的实体，不追求一次补全所有 DXF。

优先级：

1. `LWPolyline` / legacy `Polyline` 宽度。
2. `Spline` weights + closed flag。
3. `MINSERT` 阵列展开。
4. `Dimension` dimstyle 尺寸参数。
5. Raster image clip boundary。

原则：

- 每个实体先加 fixture 或 round-trip/display test。
- 实现必须保持 compat fallback 不降级。
- native supported list 只在该实体 fidelity >= compat 后再扩。

---

## 4. 近期执行清单

下一次实际编码建议从 Phase 1 开始：

1. 新增一个小型统计结构，例如 `NativeRenderStats`。
2. 给 `Scene::native_wires_for_model_space` 增加测试级统计入口。
3. 写一个测试覆盖 native rendered / compat fallback / preserved-only 三类。
4. 不做 UI；只为后续 diagnostics panel 铺路。

执行进度：

- 已新增 `NativeRenderStats`。
- 已新增 `Scene::native_render_stats`。
- 已用 TDD 验证 `Line` -> native-rendered、`Hatch` -> compat-fallback、`Unknown` -> preserved-only。
- 该统计入口当前用 `#[allow(dead_code)]` 标记，只作为测试/调试入口，不接 UI，避免引入运行时耦合。

如果要继续修显示效果，则从 Phase 2 的 tilted circle/arc regression tests 开始，先让测试暴露真实偏差，再改实现。

Phase 2 已启动：

- circle/arc 已有 normal-aware 测试覆盖。
- LwPolyline 已有 normal-aware 实现，native conversion 已传入 `entity.extrusion`；`cargo test -p H7CAD entities::lwpolyline` 通过 4 个测试。
- 新增 tilted native ellipse regression：`convert_native_ellipse_uses_extrusion_normal_for_minor_axis`。
- 修复 native ellipse conversion，使其传入 `entity.extrusion`，并修正 ellipse quadrant snap point 的 z 坐标计算。

---

## 5. 验收定义

本计划不是一次性大 PR。每个小 PR 必须满足：

- 有一条明确用户价值。
- 有 targeted test。
- 不降低现有 round-trip 和 display fidelity。
- `cargo test --workspace` 通过。
- 未把 unrelated dirty workspace 改动混入。
