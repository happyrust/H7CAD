# H7CAD 下一步开发计划

> **日期**: 2026-04-30  
> **目标**: 在不扩大 dirty worktree 风险的前提下，把当前 H7CAD 工作拆成可验证、可审查、可连续推进的开发批次。  
> **基线**: `README.md` 标记 native DWG reader 仍未对外启用；`CHANGELOG.md` 已记录 OCS->WCS Phase 0.1 的 6/17 实体接入；当前 `git diff --stat` 只有 `src/io/pid_import.rs` 与 `src/io/svg_export.rs` 存在真实文本 diff，其余 tracked `M` 多数表现为行尾 / index 噪声。

---

## 1. 当前判断

### 已具备的推进基础

- DXF 文件层已经基本闭环：native DXF reader / strict writer / roundtrip 测试已覆盖主要实体。
- OCS->WCS 任意轴算法已经落到 `h7cad-native-model::geom_ocs`，并已接入 Circle / Arc / LwPolyline / Polyline2D / Spline / Shape 这 6 类高频实体。
- native DWG AC1018、facade、viewport / owner fallback 已有 stacked PR 计划与分支策略，后续应继续保持小 PR 发布。
- 当前真实文本 diff 很小，适合先做工作区收口，再继续功能开发。

### 主要风险

- H7CAD 根目录有大量 agent / editor / scratch 目录，默认不能提交。
- `CHANGELOG.md` 已包含较多未发布说明，必须等对应代码 / docs PR 落定后再最终收口。
- 多个 tracked Rust 文件显示 `M` 但 `git diff --stat` 不计入真实 diff，疑似 CRLF / index 噪声，不能直接 `git add .`。
- `src/io/pid_import.rs` 的版本常量与 `Cargo.toml` 版本存在依赖关系，不能在未同步版本 bump 的分支里单独提交。
- `src/io/svg_export.rs` 的 fixture schema 更新依赖 `WireModel::aabb` 结构变更，必须跟几何/schema 分支对齐。

---

## 2. 优先级路线

### P0: 先收口工作区，避免后续开发踩脏状态

**目标**: 把可提交内容、暂缓内容、永不提交内容分清楚。

**操作**

1. 保持禁止 `git add .`。
2. 对 tracked `M` 文件分三类确认：
   - 真实 diff: `src/io/pid_import.rs`, `src/io/svg_export.rs`, `CHANGELOG.md`
   - 行尾 / index 噪声: OCS 相关实体文件与 `src/scene/acad_to_truck.rs`
   - 需要等依赖分支的 fixture/schema 更新
3. 将本地工具目录列入 ignore-policy 决策，但只在用户明确要求时提交 `.cursor/`, `.agents/`, `.claude/` 等规则 / skill 文件。
4. 将 docs 归档与代码 PR 分离，不把计划文档混进功能代码提交。

**验证**

```powershell
git diff --stat
git diff --name-status
git diff --numstat -- crates/h7cad-native-model/src/geom_ocs.rs src/entities/arc.rs src/entities/circle.rs src/entities/lwpolyline.rs src/entities/polyline.rs src/entities/ray.rs src/entities/shape.rs src/entities/spline.rs src/entities/traits.rs src/scene/acad_to_truck.rs
git status --porcelain=v2
```

**停止条件**

- 任一“行尾噪声”文件出现真实 Rust 源码 diff 时，停止并拆为独立代码批次。
- 准备提交本地 agent/editor/tool 目录时，必须先得到用户确认。

**执行记录（2026-04-30）**

- `git diff --stat` 只显示 `src/io/pid_import.rs` 与 `src/io/svg_export.rs` 两个真实文本 diff。
- `CHANGELOG.md` 与 OCS 相关 tracked `M` 文件在 `git status --porcelain=v2` 中 index / working hash 相同，当前按行尾或 index 噪声处理，不进入 staging。
- `src/io/pid_import.rs` 当前 diff 为 `SPPID_SOFTWARE_VERSION: 0.1.3 -> 0.1.7` 与测试 fixture 增补 `endpoint_decode_error: None`。
- `src/io/svg_export.rs` 当前 diff 为测试 fixture 增补 `aabb: WireModel::UNBOUNDED_AABB`。
- 结论：P1 的 fixture/schema follow-up 不能盲目提交，必须先确认 base branch 同时具备 package version `0.1.7` 与 `WireModel::aabb` schema。

### P1: 发布剩余 closeout 小批次

**目标**: 继续 `r50-model-owner-fallback` 之后的 stacked PR，不把未关联改动混入。

**建议批次**

| 批次 | 分支 | 范围 | 提交条件 |
|---|---|---|---|
| Fixture / schema follow-up | `r51-fixture-schema-followup` | `src/io/pid_import.rs`, `src/io/svg_export.rs` 中仍成立的最小 fixture 更新 | 版本号与 `Cargo.toml` 同步；`WireModel::aabb` 已在 base branch 存在 |
| Plan archive | `docs-native-dwg-cli-plan-archive` | `docs/cli.md`, `docs/pdf_export.md`, `docs/plans/2026-04-25-*`, `2026-04-26-*`, R46/R51 计划 | `git diff --check -- docs` 通过 |
| Ignore policy | `chore-ignore-local-agent-dirs` | `.gitignore` 中明确忽略本地工具 / scratch 输出 | 不提交工具目录本体 |
| Changelog closeout | `docs(changelog)` 独立分支 | `CHANGELOG.md` | 只在对应代码/docs PR 落定后更新 |

**验证**

```powershell
cargo test -p H7CAD --bin H7CAD svg_export
cargo test -p H7CAD --bin H7CAD sppid_software_version_tracks_cargo_pkg_version
$env:RUSTFLAGS='-Dwarnings'
cargo check --locked --workspace --all-targets
Remove-Item Env:RUSTFLAGS
```

**停止条件**

- `SPPID_SOFTWARE_VERSION` 与 package version 不一致。
- `WireModel::aabb` 尚未进入当前 base branch。
- docs 中出现“已经完成”的说法但对应代码 PR 尚未存在。

**执行记录（2026-04-30）**

- 当前工作树 `Cargo.toml` 已是 `version = "0.1.7"`。
- 当前工作树已存在 `WireModel::aabb` 与 `WireModel::UNBOUNDED_AABB`。
- `src/io/pid_import.rs` fixture 还需同步补齐 `SheetStream::geometry: None`，否则 H7CAD bin test 编译失败。
- 补齐后验证通过：
  - `cargo test -p H7CAD --bin H7CAD sppid_software_version_tracks_cargo_pkg_version` -> 1/1 passed
  - `cargo test -p H7CAD --bin H7CAD svg_export` -> 75/75 passed

### P2: 完成 DXF runtime display fidelity 的 P0

**目标**: 先解决系统级显示偏差，再进入字段细节补齐。

**2.1 OCS->WCS 剩余实体**

当前已接入 6/17，剩余建议按风险拆分：

| 子批 | 实体 | 主要文件 | 验收重点 |
|---|---|---|---|
| P0.1b | Ellipse / Point / Line | `src/entities/ellipse.rs`, `src/entities/point.rs`, line tessellation 路径 | default normal 与旧行为一致；tilted normal 落在正确平面 |
| P0.1c | Hatch / Dimension | `src/scene/tessellate.rs`, hatch boundary helpers | boundary 顶点和 snap Z 不再被压到 XY |
| P0.1d | MLine / Leader / AttributeDefinition / AttributeEntity / Insert | `src/entities/*`, block explode 路径 | nested / inserted geometry 不重复或丢失变换 |

**2.2 嵌套 ByBlock 链递归继承**

优先实现 `BlockChainStyle`，把 color / linetype / lineweight 显式沿 INSERT 递归向下传递。

**关键测试**

```powershell
cargo test --bin H7CAD scene::tests::fixture_three_level -- --nocapture
cargo test --bin H7CAD --quiet
```

**停止条件**

- OCS 变换引入 default normal `(0,0,1)` 行为变化。
- block explode 递归导致实体重复渲染、无限递归或 ByLayer / ByBlock 语义混淆。

**执行记录（2026-04-30）**

- 先补红灯测试 `fixture_nested_byblock_color_inherits_nearest_insert_override`：三层嵌套 block 中，中间 `INSERT` 为 blue，最内层 `LINE` 为 ByBlock，最终 wire 必须带叶子线端点并继承 blue。
- 红灯定位到两个问题：
  - H7CAD 只展开到嵌套 `INSERT` marker，最内层线端点没有进入 wire。
  - native/acadrust bridge 把 native `color_index = 0` 错映射成 ByLayer；native model 与 DXF 语义均要求 `0 = ByBlock`。
- 已实现：
  - `src/scene/mod.rs` 新增递归 `tessellate_insert_contents`，用 `visited_blocks` 防循环，并保留原始 block 子实体作为样式判定源。
  - `src/io/native_bridge.rs` 修正 `native_color(0) -> Color::ByBlock` 与 `Color::ByBlock -> color_index 0`。
  - 新增桥接双向测试锁定 ByBlock 颜色语义。
- 验证通过：
  - `cargo test -p H7CAD --bin H7CAD native_to_acadrust_maps_color_zero_to_byblock`
  - `cargo test -p H7CAD --bin H7CAD acadrust_to_native_maps_byblock_to_color_zero`
  - `cargo test -p H7CAD --bin H7CAD fixture_nested_byblock_color_inherits_nearest_insert_override`
  - `cargo test -p H7CAD --bin H7CAD fixture_nested_block_insert_displays_through_bridge`
  - `cargo test -p H7CAD --bin H7CAD fixture_three_level_nested_blocks_display_through_bridge`
  - `cargo test -p H7CAD --bin H7CAD fixture_circular_block_reference_terminates_without_overflow`
- 注意：本轮运行过 `cargo fmt --all`，当前 `src/io/native_bridge.rs` / `src/io/pid_import.rs` / `src/io/svg_export.rs` / `src/scene/mod.rs` 的 diff 统计被格式化放大；提交前应先决定是接受整文件格式化，还是压回最小逻辑 diff。

### P3: DXF P1 高频字段补齐

**目标**: 解决用户打开真实 DXF 后最容易肉眼发现的字段缺失。

建议顺序：

1. LWPolyline / Polyline width：先实现恒定宽度，再支持 per-vertex taper。
2. Spline weights + closed / periodic：rational NURBS 保持 unit weights 旧路径 bit-identical。
3. MINSERT 阵列展开：渲染与 snap 同步覆盖每个副本。
4. Dimension dimstyle 应用：消费 DIMSCALE / DIMASZ / DIMEXO / DIMEXE。
5. RasterImage clip_boundary：先 CPU alpha mask，必要时再 GPU shader 优化。
6. Polyline3D vertex flags：区分 SPLINE_CONTROL / SPLINE_VERTEX。

**每个子批验收**

- 新增最小 fixture / 单元测试锁定旧路径兼容。
- `cargo test --bin H7CAD --quiet` 通过。
- 变更影响 native model / bridge 时补跑对应 crate 测试。

### P4: native DWG 对外启用前置闭环

**目标**: 在 reader 继续完善时保持 README 中“public runtime DWG loading not enabled”的诚实状态。

建议顺序：

1. 继续 AC1018 real sample coverage，先提高 diagnostic 可解释性。
2. 每个 EntityData 投影补缺独立 PR：Helix, Surface, Light, Camera, Section, ProxyEntity。
3. facade 只在“错误分类稳定 + fallback notice 清晰 + 用户可恢复”后解除 `native DWG reader not implemented yet`。

**验收**

```powershell
cargo test -p h7cad-native-dwg -- --test-threads=1
cargo check -p h7cad-native-facade
```

---

## 3. 执行节奏

1. **第 1 天**: 完成 P0 工作区收口，确认哪些文件可提交、哪些只是行尾噪声。
2. **第 2-3 天**: 发布 P1 中的 fixture/schema follow-up 与 docs archive 小 PR。
3. **第 4-6 天**: 实现 P2 的嵌套 ByBlock 链和 OCS 剩余低风险实体。
4. **第 2 周**: 进入 P3 的 width / spline / MINSERT 三个最高价值字段。
5. **第 3 周起**: P3 后半段与 P4 native DWG 补缺并行推进。

---

## 4. Definition of Done

- 每个 PR 只有一个主题，能独立解释、独立回滚。
- 不提交本地工具目录、scratch 文件、vendor 临时目录。
- 新增行为都有最小回归测试；旧行为兼容路径有 default fixture 锁定。
- `CHANGELOG.md` 只在对应代码 / docs 已进入 PR 或已合并后更新。
- 对外能力描述保持诚实：native DWG runtime loading 未启用前，README / CLI 行为不提前承诺。

---

## 5. 下一步可直接执行的第一批命令

```powershell
git diff --stat
git diff --name-status
git diff --numstat -- src/io/pid_import.rs src/io/svg_export.rs CHANGELOG.md
git diff --numstat -- crates/h7cad-native-model/src/geom_ocs.rs src/entities/arc.rs src/entities/circle.rs src/entities/lwpolyline.rs src/entities/polyline.rs src/entities/ray.rs src/entities/shape.rs src/entities/spline.rs src/entities/traits.rs src/scene/acad_to_truck.rs
git diff --check -- docs CHANGELOG.md
```

若上述命令确认只有 `pid_import.rs` / `svg_export.rs` / `CHANGELOG.md` 有真实 diff，则先按 P1 的依赖关系决定是否提交，其他 `M` 文件暂不 staging。
