# DXF Hatch Fidelity Phase 2 Brainstorm 开发方案

> **日期**: 2026-05-06
> **状态**: Slice 1-2 已完成，待执行 Slice 3
> **推荐路线**: Hatch fidelity 深化
> **基线**: `main` 已推送到 `origin/main`，`cargo fmt`、`cargo check --workspace`、`cargo test --workspace` 已通过。

---

## 1. 背景

H7CAD 当前 DXF 路径已经形成 native-first 的基本闭环：

- DXF 读取走 `h7cad_native_dxf::read_dxf_bytes`，运行时保留 native document。
- `OpenNotice` 已能提示 Unknown/Proxy preserved-only 内容。
- native render stats 已能区分 native-rendered、compat-fallback、preserved-only。
- Ellipse/Spline 已进入 native render 支持范围。
- Ellipse OCS、DIMSTYLE/Dimension 参数、nested INSERT ByBlock 继承已完成。
- native Hatch 在 nested INSERT 中的 ByBlock 颜色继承与 INSERT scale/rotation boundary 回归已补上。

下一阶段应继续围绕 Hatch。原因是 Hatch 在工程图中高频、视觉错误明显，而且现有代码已有 native hatch model 与 insert transform 路径，适合小步补齐。

---

## 2. 目标

本阶段目标不是重写 Hatch 系统，而是把最容易造成“打开图纸看起来不对”的 DXF Hatch 保真缺口收敛成可测试切片：

1. Hatch 边界在 OCS、INSERT scale、rotation、translation 组合下稳定。
2. Hatch arc/ellipse edge 在变换后保持正确方向和弧段。
3. Hatch pattern 的 scale、angle、spacing 能从 DXF/native model 进入渲染或诊断路径。
4. Hatch solid fill 与 pattern fill 的行为边界清楚：能渲染的渲染，暂不支持的给 observability。

验收标准：

- 每个行为都有最小 regression test。
- 每个 slice 只动必要模块。
- `cargo test -p H7CAD nativerender_hatch` 或等价筛选测试通过。
- 阶段结束跑 `cargo check --workspace`、`cargo test --workspace`、`git diff --check`。

---

## 3. 非目标

本阶段明确不做：

- 不一次性实现全部 DXF Hatch pattern 语义。
- 不重构渲染管线或 GPU fill 架构。
- 不处理所有非 Hatch 标注实体，例如 Leader/MLeader。
- 不自动提交未跟踪工具目录、旧计划文件、日志或 `vendor_tmp/`。
- 不为了兼容未发布分支叠加临时 shim；如果当前分支内部模型不合适，直接改成清晰结构。

---

## 4. 候选路线

### 路线 A: Boundary-first

先保证 Hatch 边界几何正确，再处理 pattern。

优点：

- 最贴近用户可见错误。
- 测试可用点坐标、bbox、segment count 验证。
- 与已完成 INSERT transform 回归连续。

缺点：

- 暂时不改善 pattern 视觉细节。

### 路线 B: Pattern-first

先补 pattern scale/angle/spacing 的 native model、parser、writer、render 使用。

优点：

- DXF roundtrip 保真收益明确。
- 可形成 parser/writer 层稳定能力。

缺点：

- 如果边界 transform 仍有缺口，pattern 看起来仍会错。
- 可能牵涉 model 字段扩展和 writer 更新，范围更大。

### 路线 C: Observability-first

先建立 Hatch fidelity stats，把 unsupported pattern、edge type、OCS 情况暴露出来。

优点：

- 低风险。
- 能指导后续优先级。

缺点：

- 用户可见渲染改善较少。

---

## 5. 推荐方案

推荐 **A + C 的轻量组合**：

1. 以 Boundary-first 为主，先补最影响显示正确性的 Hatch 边界变换。
2. 同时加最小 observability，记录哪些 Hatch pattern/edge 仍未完全支持。
3. Pattern 写入下一阶段，除非某个 boundary 测试必须补 pattern 字段才能表达。

原因：

- 当前已有 nested INSERT hatch test，继续补边界风险最低。
- Hatch pattern 完整实现会快速扩大范围。
- Observability 能避免“渲染失败但用户无提示”的问题。

---

## 6. 执行切片

### Slice 1: Hatch OCS boundary transform

状态：已完成。

目标：

- 验证 Hatch boundary 点从 OCS 转 WCS 后再进入 render path。
- 覆盖非默认 extrusion，例如 `(0, 1, 0)` 或 tilted normal。

建议测试：

```powershell
cargo test -p H7CAD nativerender_hatch_ocs_boundary_uses_extrusion
```

预期实现：

- 检查 `src/scene/mod.rs` 中 `hatch_model_from_native` 与 INSERT hatch transform path。
- 复用已有 OCS/WCS helper，不新增平行数学实现。
- 如果 Hatch boundary 当前只存 XY 点，则在 native model 或 tessellation 边界处明确转换责任。

验收：

- transformed boundary 至少一个点的 `z` 或非 XY 分量能证明 extrusion 被使用。
- 默认 normal 的 Hatch 行为不变。

执行结果：

- 新增 `nativerender_hatch_ocs_boundary_uses_extrusion`。
- RED 确认：当前 Hatch boundary 未使用 `Entity.extrusion`。
- GREEN 实现：`hatch_model_from_native` 使用 `transform::ocs_point_to_wcs` 把 Hatch OCS boundary 点投影到 WCS 后再写入 `HatchModel.boundary`。
- 已验证 `cargo test -p H7CAD nativerender_hatch_ocs_boundary_uses_extrusion`、`cargo test -p H7CAD nativerender_hatch`、`cargo test -p H7CAD nativerender_insert`、`cargo check --workspace`。

### Slice 2: Hatch arc edge under INSERT rotation

状态：已完成。

目标：

- 验证 arc edge 经过 INSERT rotation/scale 后仍保持端点、方向和采样段稳定。

建议测试：

```powershell
cargo test -p H7CAD nativerender_insert_hatch_arc_edge_applies_rotation
```

预期实现：

- 在 native Hatch test fixture 中构造 arc edge。
- 比较输出 boundary 的首尾点或 bbox，而不是逐点锁死所有采样点。
- 若现有模型没有 arc edge 表达，先补最小 native fixture 支持。

验收：

- 旋转 90 度后的 bbox 与预期一致。
- 不破坏已有 `nativerender_insert_hatch_boundary_applies_scale_and_rotation`。

执行结果：

- 新增 `nativerender_insert_hatch_arc_edge_applies_rotation`。
- RED 确认：native Hatch circular/elliptic arc 角度按弧度采样，未按 DXF/native 的度数语义转换。
- GREEN 实现：`hatch_model_from_native` 在采样 native Hatch circular/elliptic arc 时先将 start/end angle 从度转换为弧度。
- 已验证 `cargo test -p H7CAD nativerender_insert_hatch_arc_edge_applies_rotation`、`cargo test -p H7CAD nativerender_hatch`、`cargo test -p H7CAD nativerender_insert`、`cargo test -p h7cad-native-dxf --test entity_2d_roundtrip`、`cargo check --workspace`。

### Slice 3: Hatch pattern support inventory

目标：

- 把当前 native Hatch pattern 支持范围写成可测试事实。
- 对 unsupported pattern 信息提供诊断或 stats，而不是静默丢失。

建议测试：

```powershell
cargo test -p H7CAD native_hatch_pattern_support_stats_reports_unsupported_pattern
```

预期实现：

- 优先放在现有 diagnostics/stats 附近，避免 UI 先行。
- 输出数量级信息，例如 unsupported pattern count、edge type count。
- 不在本 slice 实现完整 pattern renderer。

验收：

- 打开含 unsupported Hatch pattern 的 native document 时，测试能观察到统计或 notice。
- 支持范围文案可被未来 UI 使用。

### Slice 4: DXF parser/writer field audit

目标：

- 盘点 Hatch 相关 DXF group code 在 native model 中是否保留。
- 优先补 roundtrip 损失明显、实现低风险的字段。

候选字段：

- pattern name
- pattern angle
- pattern scale
- pattern double flag
- boundary path count
- edge type

建议测试：

```powershell
cargo test -p h7cad-native-dxf --test entity_2d_roundtrip roundtrip_hatch_preserves_pattern_fields
```

验收：

- 至少形成字段矩阵，标记 supported、preserved-only、missing。
- 如果改 parser/writer，必须加 roundtrip 断言。

---

## 7. 主要文件

预期高频文件：

- `src/scene/mod.rs`
- `src/scene/render.rs`
- `src/scene/transform.rs`
- `src/io/diagnostics.rs`
- `crates/h7cad-native-model/src/lib.rs`
- `crates/h7cad-native-dxf/src/lib.rs`
- `crates/h7cad-native-dxf/src/writer.rs`
- `crates/h7cad-native-dxf/tests/entity_2d_roundtrip.rs`

只在必要时触碰：

- `src/scene/tessellate.rs`
- `src/scene/acad_to_truck.rs`
- UI notice 展示层

---

## 8. 测试策略

每个 slice 使用 TDD：

1. 写最小失败测试，失败原因必须对应目标缺口。
2. 做最小实现。
3. 跑 slice 测试。
4. 跑相关回归集合。
5. 阶段末跑 workspace 验证。

阶段命令：

```powershell
cargo fmt
cargo check --workspace
cargo test -p H7CAD nativerender_hatch
cargo test -p H7CAD nativerender_insert
cargo test -p h7cad-native-dxf --test entity_2d_roundtrip
cargo test --workspace
git diff --check
```

---

## 9. 风险与控制

风险：

- Hatch fill、boundary wire、INSERT transform 可能走不同路径。
- OCS 转换容易和 INSERT transform 顺序混淆。
- pattern 完整渲染可能牵涉较多 DXF 语义。
- 当前工作区仍有未跟踪本地工具和历史计划文件，不能盲目提交。

控制：

- 每个 slice 至多修改 3 个核心模块。
- 坐标测试用少量关键点或 bbox，不锁死过多采样细节。
- Pattern 先 inventory，再决定是否实现。
- 每次提交前只 stage 本阶段相关文件。

---

## 10. 停止条件

出现以下情况时停止功能开发，转为重新拆分计划：

- 单个 slice 需要重构渲染 pipeline。
- Hatch pattern 实现需要引入新抽象且影响非 Hatch 实体。
- 新测试必须依赖大量真实 DXF fixture，无法构造最小 native fixture。
- `cargo check --workspace` 或 `cargo test --workspace` 出现与本阶段无关的大面积失败。

---

## 11. 第一步建议

从 **Slice 1: Hatch OCS boundary transform** 开始。

理由：

- 与已完成的 Ellipse OCS、INSERT hatch boundary 回归最连续。
- 最容易用小 fixture 证明行为。
- 如果这里失败，后续 pattern 和 arc edge 都没有稳定基础。

第一条任务：

```text
新增 RED test: nativerender_hatch_ocs_boundary_uses_extrusion
```

通过后再进入 Slice 2。
