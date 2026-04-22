# 开发计划：显示 & 渲染控制家族 5 变量扩充（二十六）

> 起稿：2026-04-22（第二十六轮）
> 前置：HEADER 已覆盖 88 变量（~29%）。上一轮二十五补了 SNAP/GRID 家族
> 6 变量并顺带升级 `format_f64` 到 shortest round-trip。
> 本轮选最**小风险、最高密度**路径：5 个 code 70 / `i16` HEADER 变量，
> 全部是"显示层布尔 / 小枚举"，io 层纯透传。

## 动机

当前 `DocumentHeader` 覆盖的 mode flag 集中在"输入 / 交互"维度：
`orthomode / gridmode / snapmode / fillmode / mirrtext`。AutoCAD 里还有
一组**独立的显示 & 渲染维度**布尔 / 小枚举，控制的是"屏幕上 3D 实体
怎么绘制"的默认行为：

| 维度 | AutoCAD 语义 | 为什么 io 需要知道 |
|------|-------------|------------------|
| 轮廓线显隐 | `$DISPSILH` = 3D 实体在线框视图里是否画 silhouette edges | UI 打开 3D 视图时默认显示与否 |
| 实时拖拽 | `$DRAGMODE` = 拖动对象时实时预览的模式 | 交互体验默认值 |
| 自动重生成 | `$REGENMODE` = 改缩放时是否自动重生几何 | 大型绘图的性能默认 |
| 着色边 | `$SHADEDGE` = SHADE 命令时着色 + 边缘组合 | 渲染方式默认 |
| 漫反射比 | `$SHADEDIF` = 漫反射与环境光的比例（0–100） | 光照默认 |

H7CAD 之前 reader / writer **全部忽略这 5 个变量**，roundtrip 后默认值
归零（或归 AutoCAD 默认），用户自定义的显示偏好全部丢失。本轮把这 5 个
变量一次补齐，让 "用户保存 → H7CAD 读 → H7CAD 写" 三步后显示偏好一致。

## 目标

### 字段

按 AutoCAD DXF Reference（R14 → R2018 均有）：

| 字段 | 类型 | `$` 变量 | DXF code | Default | 语义 |
|---|---|---|---|---|---|
| `dispsilh` | `i16` | `$DISPSILH` | 70 | `0` | 3D 实体线框视图显示 silhouette 边：0 = 关 / 1 = 开 |
| `dragmode` | `i16` | `$DRAGMODE` | 70 | `2` | 拖拽预览：0 = 关 / 1 = 开 / 2 = auto（AutoCAD 默认 2） |
| `regenmode` | `i16` | `$REGENMODE` | 70 | `1` | 自动重生：0 = 关 / 1 = 开（AutoCAD 默认 1） |
| `shadedge` | `i16` | `$SHADEDGE` | 70 | `3` | SHADE 模式：0 = 面着色（无边） / 1 = 面着色 + 边 / 2 = 面隐藏线 / 3 = 面线框（AutoCAD 默认 3） |
| `shadedif` | `i16` | `$SHADEDIF` | 70 | `70` | 漫反射比（0–100，AutoCAD 默认 70） |

插入位置：`DocumentHeader` 里**紧跟** `$MIRRTEXT / $ATTMODE` 的
"Drawing mode flags" 组尾，但**晚于** `Snap & grid geometry` 子组
(由二十五轮添加)。这样 struct 源码里形成：

```
// Drawing mode flags       ← 既有输入维度 bool (orthomode 等)
// Snap & grid geometry     ← 二十五轮添加的值 (snap_base 等)
// Display & render flags   ← 本轮添加 (dispsilh 等)
// Current drawing attrs    ← 既有 (clayer 等)
```

每组内部保持内聚，组间按"输入→空间→渲染→属性"的语义层递进。

### 默认值选型

- `dispsilh = 0`：AutoCAD 默认关闭 silhouette（仅在用户打开 3D 视图时
  手动启用）
- `dragmode = 2`：AutoCAD 默认 auto —— 拖动时自动决定是否启用预览
- `regenmode = 1`：AutoCAD 默认 on —— 任何缩放都会自动重生几何
- `shadedge = 3`：AutoCAD 默认 3 = "面线框"（最省力的渲染）
- `shadedif = 70`：AutoCAD 默认 70（70% 漫反射 + 30% 环境光）

全部对齐 AutoCAD factory default；与既有默认（`attmode=1`、
`fillmode=true` 等）一致的"开箱即用"偏见。

## 非目标

- **不**校验 `shadedif` 在 `0..=100` 范围内（AutoCAD 允许任意值；UI
  层负责 clamping）
- **不**校验 `shadedge` 在 `0..=3` 枚举内（同上，AutoCAD 将来可能
  扩 4/5）
- **不**把 `dispsilh / regenmode` 用 `bool` 存（虽然语义是 0/1，但
  `$DRAGMODE / $SHADEDGE / $SHADEDIF` 是真 i16，**保持家族一致性**
  胜于局部 bool 优化；下游消费者用 `!= 0` 做 bool 判断即可）
- **不**修改既有 `$ORTHOMODE / $GRIDMODE / $SNAPMODE / $FILLMODE /
  $MIRRTEXT` 的 bool 存储（历史决定）
- **不**触碰 DWG 侧或 `real_dwg_samples_baseline_m3b` 红灯（与本轮
  HEADER io 改动完全正交）

## 关键设计

### 1. Model（`crates/h7cad-native-model/src/lib.rs`）

紧跟 Snap & grid geometry 组尾（`grid_unit` 之后），`clayer` 之前：

```rust
pub grid_unit: [f64; 2],

// Display & render flags — value side of the default 3D viewport /
// shading behaviour. All stored as `i16` for family consistency
// (AutoCAD stores all five at code 70). io layer is pure passthrough;
// semantic clamping (e.g. shadedif ∈ 0..=100) is the UI's concern.
/// `$DISPSILH` (code 70): display silhouette edges on 3D solids in
/// wireframe views. 0 = off (default), 1 = on.
pub dispsilh: i16,
/// `$DRAGMODE` (code 70): interactive drag preview.
/// 0 = off, 1 = on, 2 = auto (default, AutoCAD picks).
pub dragmode: i16,
/// `$REGENMODE` (code 70): automatic geometry regeneration on zoom /
/// view change. 0 = manual REGEN required, 1 = auto (default).
pub regenmode: i16,
/// `$SHADEDGE` (code 70): SHADE command edge / face combination.
/// 0 = faces shaded, no edges;
/// 1 = faces shaded + edges drawn;
/// 2 = faces hidden-line;
/// 3 = faces wireframe (default).
pub shadedge: i16,
/// `$SHADEDIF` (code 70): diffuse-to-ambient light ratio during
/// SHADE, as a percentage 0..=100. AutoCAD default 70. io layer
/// stores the raw i16 — UI is responsible for clamping on input.
pub shadedif: i16,
```

`Default::default()` 同步追加（接在 `grid_unit` 之后）：

```rust
grid_unit: [0.5, 0.5],

dispsilh: 0,
dragmode: 2,
regenmode: 1,
shadedge: 3,
shadedif: 70,
```

### 2. Reader（`crates/h7cad-native-dxf/src/lib.rs`）

在二十五轮的 SNAP/GRID arm 组尾（`$GRIDUNIT` 之后、`$CLAYER` 之前）
插入 5 arm：

```rust
"$GRIDUNIT" => doc.header.grid_unit = [f(10), f(20)],

// Display & render flags.
"$DISPSILH" => doc.header.dispsilh = i16v(70),
"$DRAGMODE" => doc.header.dragmode = i16v(70),
"$REGENMODE" => doc.header.regenmode = i16v(70),
"$SHADEDGE" => doc.header.shadedge = i16v(70),
"$SHADEDIF" => doc.header.shadedif = i16v(70),
```

### 3. Writer（`crates/h7cad-native-dxf/src/writer.rs`）

在 `$GRIDUNIT` 输出之后、`$CLAYER` 之前插入 5 段：

```rust
w.pair_str(9, "$GRIDUNIT");
w.pair_f64(10, doc.header.grid_unit[0]);
w.pair_f64(20, doc.header.grid_unit[1]);

// ── Display & render flags ─────────────────────────────────────────────
w.pair_str(9, "$DISPSILH");
w.pair_i16(70, doc.header.dispsilh);

w.pair_str(9, "$DRAGMODE");
w.pair_i16(70, doc.header.dragmode);

w.pair_str(9, "$REGENMODE");
w.pair_i16(70, doc.header.regenmode);

w.pair_str(9, "$SHADEDGE");
w.pair_i16(70, doc.header.shadedge);

w.pair_str(9, "$SHADEDIF");
w.pair_i16(70, doc.header.shadedif);
```

### 4. 测试（`crates/h7cad-native-dxf/tests/header_display_render.rs`）

4 条，模式对齐二十五轮的 `header_snap_grid.rs`：

1. `header_reads_display_render_family` — 构造 HEADER 里 5 变量**全部
   非默认**的 DXF → 读后每字段按 ground-truth 精确恢复。
2. `header_writes_display_render_family` — 非默认 doc → write → 字符串
   里 5 个 `$VAR` 按 reader arm 顺序出现 + 值精确匹配。
3. `header_roundtrip_preserves_display_render_family` — read → write →
   read 5 字段 bit-identical（i16 不涉及精度，仅防 0/1 bool 被误写
   或枚举被错转）。
4. `header_legacy_file_without_display_render_loads_with_defaults` —
   legacy HEADER 无这 5 变量 → 读出命中 AutoCAD 默认（0 / 2 / 1 / 3
   / 70）。

### 5. Ground-truth 值选择

每字段选**和 Default 不同**的值，让 reader arm 串位的 bug 立刻暴露：

- `dispsilh = 1`（≠ default 0）
- `dragmode = 0`（≠ default 2）
- `regenmode = 0`（≠ default 1）
- `shadedge = 1`（≠ default 3）
- `shadedif = 50`（≠ default 70）

5 个值两两互不相等，即使 reader 弄错 arm 映射也能一眼看出是哪两个
被串了。

## 实施步骤

| 步骤 | 工作内容 | 预估 |
|---|---|---|
| M1 | `DocumentHeader` 扩 5 字段 + Default | 2 min |
| M2 | reader 5 arm | 1 min |
| M3 | writer 5 对 pair | 1 min |
| M4 | 新测试文件 4 条 | 6 min |
| M5 | `cargo test -p h7cad-native-dxf` 全绿 | 1 min |
| M6 | `cargo check -p H7CAD`、`-p h7cad-native-facade` | 2 min |
| M7 | `ReadLints` + CHANGELOG "二十六" 条目 | 4 min |

总预算约 17 min，是目前最省时的一轮。

## 验收

- `cargo test -p h7cad-native-dxf` **153 → 157**（+4 header_display_render）
- `cargo test --bin H7CAD io::native_bridge` 25 / 25 不受影响
- `cargo check -p H7CAD` 零新 warning
- `cargo check -p h7cad-native-facade` 零新 warning
- `ReadLints` 改动的 4 个文件零 lint
- CHANGELOG 存在 "2026-04-22（二十六）" 条目
- HEADER 覆盖：88 → **93**
- 本 plan 文件 §状态 写 "落地完成"

## 风险

| 风险 | 缓解 |
|---|---|
| 5 个变量都用 code 70，reader arm 内部 `i16v(70)` 闭包重复；但每条 arm 独立作用域，不会串读 | 既有 mode flag 组（orthomode / gridmode / snapmode / fillmode / mirrtext）同样 5 条 code 70 arm 并列，无问题，参照同模式 |
| `shadedif` 范围 0–100，但 i16 存储接受 -32768..=32767；AutoCAD 文件里出现 >100 值 | io 层 passthrough，文档注释已明示；测试用 50 验证典型中间值 |
| 未来 AutoCAD 扩 `$SHADEDGE = 4` 或更多 | `i16` 容纳任意；测试不 hardcode 枚举范围，避免锁死 |

## 执行顺序

M1 → M2 → M3 → M4 → M5 → M6 → M7 → commit（严格串行）

## 下一轮候选（二十七）

本轮 / 二十五轮已用完"快赢" HEADER 组。下一轮自然候选：

1. `$DIMALT / $DIMALTD / $DIMALTF / $DIMALTRND / $DIMALTTD / $DIMALTTZ
   / $DIMALTU / $DIMALTZ / $DIMAPOST` — 9 变量，DIM 替代单位家族
   （中等规模；需要 Tier-3 dim 引入新的 `dim_alt_*` 子组）
2. `$LOFTANG1 / $LOFTANG2 / $LOFTMAG1 / $LOFTMAG2 / $LOFTNORMALS /
   $LOFTPARAM` — 6 变量，Loft 3D 默认
3. `$INDEXCTL / $PROJECTNAME / $HYPERLINKBASE / $STYLESHEET` — 4 变量，
   drawing 元数据附加

推荐优先级：**3 → 2 → 1**（按规模从小到大，方便下一轮继续保持"快赢"
节奏）。

## 状态

- [x] 计划定稿（本文件）
- [x] M1 DocumentHeader 5 字段 + Default
- [x] M2 reader 5 arm
- [x] M3 writer 5 对 pair
- [x] M4 新测试文件 4 条
- [x] M5 `cargo test -p h7cad-native-dxf` 153 → 157 全绿
- [x] M6 `cargo check -p H7CAD` / `-p h7cad-native-facade` 零新 warning；`cargo test --bin H7CAD io::native_bridge` 25 / 25
- [x] M7 CHANGELOG "2026-04-22（二十六）" 落地 + ReadLints 零 lint
