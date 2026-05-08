# DXF Fidelity 下一阶段 Brainstorm 开发方案

> **日期**: 2026-05-06
> **前置状态**: 已完成 DXF diagnostics、native render stats、Ellipse/Spline native render、Ellipse OCS、DIMSTYLE/dimension 参数、native nested ByBlock inheritance。
> **验证状态**: `cargo check --workspace`、`cargo test --workspace`、`git diff --check` 均通过。
> **主要风险**: 当前工作树 diff 很大，包含 merge/format/feature 累积改动，继续开发前要避免不可审阅。

---

## 1. 当前成果

已完成的用户可见能力：

- 打开 DXF 时能对 Unknown/Proxy preserved-only 内容给 warning。
- native render stats 能区分 native-rendered / compat-fallback / preserved-only。
- native Ellipse/Spline 可以进入 native render 支持集合。
- tilted native Ellipse 使用 `entity.extrusion`，不再被压回 XY。
- native Dimension 使用 DIMSTYLE 的 `dimasz/dimexo/dimexe/dimtxt/dimscale` 关键显示参数。
- DXF DIMSTYLE 表修正 code 映射：`44=DIMEXE`，`147=DIMGAP`。
- native nested INSERT 中 `ByBlock` color/linetype/lineweight 可继承最近有效 INSERT style。

这些改动已经形成一组“DXF fidelity + observability”能力闭环。

---

## 2. Brainstorm 候选路线

### 路线 A: 先收敛可审阅 diff

目标：

把当前大 diff 按逻辑切片整理，降低后续开发风险。

切片建议：

1. Merge/build compatibility fixes。
2. DXF diagnostics + OpenNotice。
3. Native render stats。
4. Ellipse/Spline + OCS fixes。
5. DIMSTYLE/dimension fidelity。
6. Nested ByBlock inheritance。
7. 计划文档与进度文档。

优点：

- 最能降低“继续改导致无法 review”的风险。
- 当前已有 workspace 级验证，可形成可交付点。

缺点：

- 需要用户明确是否提交，以及是否允许整理已有大 diff。

### 路线 B: 继续 Hatch fidelity

目标：

补 Hatch 边界/填充在 OCS、nested INSERT、pattern scale 上的保真缺口。

候选测试：

1. `native_hatch_insert_transform_preserves_boundary`
2. `native_hatch_byblock_inherits_insert_color`
3. `native_hatch_pattern_scale_uses_entity_linetype_or_header`
4. `native_hatch_arc_edges_survive_insert_rotation`

优点：

- Hatch 是工程图高频实体，显示错会非常明显。
- 已有 native hatch model path，可做小步 regression。

风险：

- Hatch path 牵涉 fill model、wire model、insert transform，范围比 Dimension 大。
- 需要区分 boundary transform 与 render style inheritance，不能混成一个大修。

### 路线 C: 继续 Multileader/Leader fidelity

目标：

让 Leader/MultiLeader 的文字、leader line、arrow、style 更接近 DXF 语义。

候选测试：

1. `native_multileader_text_uses_style_height`
2. `native_multileader_byblock_inherits_insert_color`
3. `native_leader_arrow_size_uses_dimstyle_or_mleader_style`

优点：

- 标注系统完整性更好。
- 与刚完成的 DIMSTYLE 方向一致。

风险：

- MultiLeader style 模型可能尚不完整，容易先被 data model 卡住。

---

## 3. 推荐路线

推荐先走 **路线 A: 收敛可审阅 diff**，然后再做 **路线 B: Hatch fidelity**。

原因：

1. 当前 `git diff --stat` 很大，继续功能开发会增加不可审阅风险。
2. 已完成的 DXF fidelity 切片已经足够形成一个阶段性成果。
3. Hatch 是下一个高价值实体，但最好建立在干净 review 边界上。

若用户要求“继续写代码，不整理提交”，则按路线 B 的第一个 Hatch 小测试开始。

---

## 4. 收敛计划

### Phase 0: 只读盘点

命令：

```powershell
git status --short
git diff --name-only
git diff --stat
```

目标：

- 确认哪些文件属于本轮 DXF fidelity。
- 标记 merge/format 产生的大面积噪声。
- 不做 reset，不回滚用户改动。

### Phase 1: 逻辑分组

分组：

- Group 1: native model + native dxf DIMSTYLE。
- Group 2: tessellation/render fidelity。
- Group 3: diagnostics/stats observability。
- Group 4: tests。
- Group 5: docs/progress。

输出：

- 一份待提交清单。
- 每组验证命令。

### Phase 2: 最小验证

必跑：

```powershell
cargo check --workspace
cargo test --workspace
git diff --check
```

可选：

```powershell
cargo test -p h7cad-native-dxf --test entity_2d_roundtrip
cargo test -p H7CAD tessellate_native_dimension
cargo test -p H7CAD nativerender_insert
```

### Phase 3: 准备提交

只有用户明确要求提交时才执行：

- 先 `git status`
- 只 stage 相关文件
- 写一个概括 DXF fidelity 的 commit message
- 不 push

---

## 5. Hatch Fidelity 执行计划

若继续开发，选择第一个小切片：

### Test 1: nested INSERT hatch ByBlock color

目标：

验证 nested INSERT 中 native Hatch 的 `ByBlock` 颜色继承最近有效 INSERT。

RED 预期：

- 当前 `native_insert_hatch_models` 直接 `render_style_native(document, child)`，没有 inherited style。
- Hatch `color_index=0` 很可能得到 ACI 0/default，而不是 INSERT color。

实现方向：

- 复用 `render_style_native_inheriting`。
- 给 `native_insert_hatch_models` 增加 inherited style 参数。
- 与 wire path 保持一致。

验收：

```powershell
cargo test -p H7CAD nativerender_nested_insert_hatch_byblock_uses_nearest_insert_color
cargo test -p H7CAD nativerender_insert
cargo test --workspace
```

### Test 2: hatch boundary transform stays stable

目标：

确认 insert scale/rotation/translation 后 hatch boundary 与 wire geometry 同步。

风险：

- 这一步可能暴露 fill tessellation 与 boundary wire 的不同 transform 路径。
- 如果失败，必须拆成独立小修。

---

## 6. 停止条件

停止继续功能开发并转入整理/提交的条件：

- workspace test/check 仍通过，但 diff 已经难以人工 review。
- 新测试需要改动超过 3 个核心模块。
- Hatch/MLeader data model 缺口超过当前小切片。
- 出现需要回滚用户/merge 改动的风险。
