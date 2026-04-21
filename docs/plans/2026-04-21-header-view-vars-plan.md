# 开发计划：DXF HEADER 当前视图 3 变量扩充

> 起稿：2026-04-21（第九轮）  
> 前置：
> - `docs/plans/2026-04-21-header-drawing-vars-plan.md`（15 绘图环境）
> - `docs/plans/2026-04-21-header-timestamps-plan.md`（4 时间戳）
> - `docs/plans/2026-04-21-header-ucs-family-plan.md`（5 UCS）
> 本轮继续扩 HEADER，补齐"当前活动视图"3 变量。

## 动机

当 H7CAD 打开真实 AutoCAD DXF 时，HEADER 段的 `$VIEWCTR / $VIEWSIZE / $VIEWDIR` 决定打开后默认视口的中心 / 高度 / 视线方向。当前 reader 忽略 → 用户打开 AutoCAD 保存的 .dxf 时视图总是回到 H7CAD 默认 fit_all 生成的视口，AutoCAD 端精心 pan/zoom 的视图设置完全失效；writer 写回也不带视图设置。

本轮扩 3 个字段，让"活动视图"元数据完整 round-trip。**不接入 Scene.camera**（独立工作：接入 camera 需要转换 UCS → camera transform，涉及 `glam::Mat4` 矩阵运算，scope 大）。

## 目标

1. `DocumentHeader` 扩 3 字段：
   - `viewctr: [f64; 2]`（`$VIEWCTR`，code 10/20，default `[0, 0]`）
   - `viewsize: f64`（`$VIEWSIZE`，code 40，default 1.0）
   - `viewdir: [f64; 3]`（`$VIEWDIR`，code 10/20/30，default `[0, 0, 1]` = Z-up 视线，面向 XY 平面）
2. Reader match 加 3 arm
3. Writer 对称输出
4. 测试：read / write / roundtrip / legacy，4 条

## 非目标

- 不接入 `Scene::camera`（UCS → camera 的矩阵运算是独立 scope）
- 不处理 `$VIEWTWIST`（view rotation angle, code 40, rarely set）
- 不处理 `$VIEWMODE` (code 70, perspective flag)
- 不扩 TABLES.VIEW 表（by-name view 字典独立 scope）
- 不做"默认 viewsize 合适区间"的智能推断

## 关键设计

### 1. Model

`DocumentHeader`（插在 timestamp 之后 / `handseed` 之前）：

```rust
// Active-view metadata.
/// `$VIEWCTR` (codes 10/20): current view center point (WCS).
pub viewctr: [f64; 2],
/// `$VIEWSIZE` (code 40): current view height (i.e. visible world
/// height along the view's Y axis). Default 1.0.
pub viewsize: f64,
/// `$VIEWDIR` (codes 10/20/30): current view direction, from view
/// target to the eye (WCS). Default [0, 0, 1] (top-down plan view).
pub viewdir: [f64; 3],
```

`Default` 填 `[0, 0]` / `1.0` / `[0, 0, 1]`。

### 2. Reader

```rust
"$VIEWCTR" => doc.header.viewctr = [f(10), f(20)],
"$VIEWSIZE" => doc.header.viewsize = f(40),
"$VIEWDIR" => doc.header.viewdir = [f(10), f(20), f(30)],
```

### 3. Writer

按 AutoCAD 惯例顺序在 timestamp 之后、`$HANDSEED` 之前输出：

```rust
w.pair_str(9, "$VIEWCTR");
w.point2d(10, doc.header.viewctr);

w.pair_str(9, "$VIEWSIZE");
w.pair_f64(40, doc.header.viewsize);

w.pair_str(9, "$VIEWDIR");
w.point3d(10, doc.header.viewdir);
```

### 4. 测试

`tests/header_view_vars.rs`：

- `header_reads_all_3_view_vars`：非默认视图（`viewctr=[100, 200], viewsize=42.5, viewdir=[1, 1, 1] / sqrt(3)`）→ 精确读取
- `header_writes_all_3_view_vars`：构造 → write → 扫 `$VIEW*` 存在
- `header_roundtrip_preserves_all_3_view_vars`：read → write → read，容忍 1e-9
- `header_legacy_file_without_view_fields_loads_with_defaults`：legacy → default

## 实施步骤

### M1 — model（5 min）

`DocumentHeader` 加 3 字段 + `Default`。

### M2 — reader（5 min）

加 3 arm。

### M3 — writer（5 min）

加 3 pair 块。

### M4 — 测试（15 min）

4 条集成测试。

### M5 — validator + CHANGELOG（10 min）

- `cargo test -p h7cad-native-dxf` 109 → **113** (+4)
- `cargo test --bin H7CAD io::native_bridge` 无回归
- CHANGELOG "2026-04-21（九）"

## 风险与缓解

| 风险 | 缓解 |
|---|---|
| `$VIEWDIR` 为零向量或非单位向量 | reader 透传，不校验；上层代码如要规范化视线向量自行处理 |
| `$VIEWSIZE = 0` | 不做 clamp（AutoCAD 也允许），zero-size view 由 UI 层处理 |
| 默认 viewdir `[0, 0, 1]` 与 viewctr `[0, 0]` + viewsize `1.0` 可能让 fit_all 看不到任何东西 | 这只是初始化值，实际 open 流程会走 `fit_all` / `fit_layers_matching`，不依赖 header 视图变量 |

## 验收

- `cargo test -p h7cad-native-dxf` ≥ **113**
- `cargo test --bin H7CAD io::native_bridge` 20 / 20
- `cargo check -p H7CAD` 零新 warning
- CHANGELOG 条目

## 执行顺序

M1 → M2 → M3 → M4 → M5（严格串行）
