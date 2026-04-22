# 开发计划：SNAP/GRID 家族 6 变量扩充（二十五）

> 起稿：2026-04-22（第二十五轮）
> 前置：HEADER 已覆盖 82 变量（~27%）。上一轮二十四补了 i64 helper +
> `$REQUIREDVERSIONS`。本轮继续沿同一路径前进——把早就存在的三元布尔
> `orthomode / gridmode / snapmode` 真正闭环，给它们补上"值"侧的 6 个
> 伴生变量，让"snap / grid 开关已开"不再等价于"spacing 全部归零"。

## 动机

当前 `DocumentHeader` 里 `snapmode` / `gridmode` 两个 bool 已被 Tier-0
覆盖，但 AutoCAD 里打开 snap / grid 时**必须**配合 `$SNAPUNIT` /
`$GRIDUNIT` 指定间距，配合 `$SNAPBASE` 指定基准点，配合 `$SNAPSTYLE`
决定正交 vs 等轴测，配合 `$SNAPANG` 决定旋转角，配合 `$SNAPISOPAIR`
决定等轴测三面方向。H7CAD 之前 reader / writer 对这 6 个变量**全部
忽略**，导致 roundtrip 后 snap/grid 的布尔开关还在、间距 /基准 /风格
全部归零，UI 上会出现"看起来开着但实际不工作"的诡异状态。

本轮把这 6 个成对变量补齐，让 "snap on = 有间距 = 有基准 = 有风格"
的语义真正闭合。所有字段都是**io 层纯透传**——各 bit 的业务含义由
UI / 命令层解释，不在 DXF 层做决策。

## 目标

### 字段

按 AutoCAD DXF 参考（R14 + R2000 均适用；R2018 沿用）：

| 字段 | 类型 | `$` 变量 | DXF code | Default | 语义 |
|---|---|---|---|---|---|
| `snap_base` | `[f64; 2]` | `$SNAPBASE` | 10 / 20 | `[0.0, 0.0]` | snap 栅格基准点（current UCS） |
| `snap_unit` | `[f64; 2]` | `$SNAPUNIT` | 10 / 20 | `[0.5, 0.5]` | X / Y 方向 snap 间距 |
| `snap_style` | `i16` | `$SNAPSTYLE` | 70 | `0` | 0 = 正交；1 = 等轴测 |
| `snap_ang` | `f64` | `$SNAPANG` | 50 | `0.0` | snap 栅格旋转角（弧度） |
| `snap_iso_pair` | `i16` | `$SNAPISOPAIR` | 70 | `0` | 等轴测面：0 = 左、1 = 上、2 = 右 |
| `grid_unit` | `[f64; 2]` | `$GRIDUNIT` | 10 / 20 | `[0.5, 0.5]` | grid 栅格 X / Y 间距 |

插入位置：`DocumentHeader` 里**紧跟** `snapmode` / `gridmode` 之后、
`orthomode` 相邻——让"mode flag + 伴生值"两拨字段在 struct 源码里
形成视觉相邻的 6+6 双列布局，review diff 阶段一眼可读。

### 默认值选型

- `snap_unit` / `grid_unit` 选 `[0.5, 0.5]` —— AutoCAD 新建 imperial
  drawing 的出厂默认；公制模板一般也落在 `10.0` 附近但走另一条模板
  初始化路径，和 HEADER 本身无关。io 层取"imperial 默认"与 `ltscale`
  / `textsize`（2.5）/ `dimscale`（1.0）等现有默认保持同一基线。
- `snap_base = [0.0, 0.0]` —— 原点；与 `insbase` 默认一致。
- `snap_ang = 0.0` / `snap_style = 0` / `snap_iso_pair = 0` —— 正交、
  旋转 0、左等轴测面。
- 全部是"最常见、最守旧"的选择；任何用户从空模板起步的绘图都会落在
  这一套默认上。

## 非目标

- **不**验证 `snap_style=1` 时 `snap_unit.x == snap_unit.y`（AutoCAD
  内部强制等轴测 snap 的 X / Y 间距相等，但这是 UI 规则，io 层只做
  透传，不插手）
- **不**自动把 `snap_ang` wrap 到 `[0, 2π)`（AutoCAD 允许任意浮点，
  我们也照传）
- **不**修改 `snapmode` / `gridmode` 的 reader / writer（它们已落地，
  本轮只追加"值"侧）
- **不**触碰 DWG 侧 —— `h7cad-native-dwg` 的 AC1015 entity 解码有
  `real_dwg_samples_baseline_m3b` 挂账（26 LINE vs 40 baseline 的
  红灯），但那是 M3B 分支的工作，与 HEADER io 层改动正交，不在本
  Sprint 内

## 关键设计

### 1. Model（`crates/h7cad-native-model/src/lib.rs`）

在 `DocumentHeader` 的 "Drawing mode flags" 组尾（`attmode` 之后、
`clayer` 之前）追加一段 "Snap & grid geometry" 子组，6 字段连续声明：

```rust
pub attmode: i16,

// Snap & grid geometry (伴生 snapmode / gridmode / orthomode 三布尔)
/// `$SNAPBASE` (codes 10/20): snap grid base point in current UCS.
pub snap_base: [f64; 2],
/// `$SNAPUNIT` (codes 10/20): X / Y snap spacing. Default 0.5 (imperial).
pub snap_unit: [f64; 2],
/// `$SNAPSTYLE` (code 70): 0 = rectangular, 1 = isometric.
pub snap_style: i16,
/// `$SNAPANG` (code 50): snap grid rotation, radians. Default 0.0.
pub snap_ang: f64,
/// `$SNAPISOPAIR` (code 70): isometric plane selection
/// (0 = left, 1 = top, 2 = right). Only meaningful when
/// `snap_style == 1`. Default 0.
pub snap_iso_pair: i16,
/// `$GRIDUNIT` (codes 10/20): grid display spacing X / Y. Independent
/// of `snap_unit`—AutoCAD lets snap and grid use different spacings.
pub grid_unit: [f64; 2],
```

`Default::default()` 同步追加：

```rust
attmode: 1,

snap_base: [0.0, 0.0],
snap_unit: [0.5, 0.5],
snap_style: 0,
snap_ang: 0.0,
snap_iso_pair: 0,
grid_unit: [0.5, 0.5],
```

### 2. Reader（`crates/h7cad-native-dxf/src/lib.rs`）

在 `read_header_section` 的 `$ATTMODE => doc.header.attmode = i16v(70)`
arm 之后插入 6 个 arm：

```rust
"$SNAPBASE" => {
    doc.header.snap_base = [f(10), f(20)];
}
"$SNAPUNIT" => {
    doc.header.snap_unit = [f(10), f(20)];
}
"$SNAPSTYLE" => doc.header.snap_style = i16v(70),
"$SNAPANG" => doc.header.snap_ang = f(50),
"$SNAPISOPAIR" => doc.header.snap_iso_pair = i16v(70),
"$GRIDUNIT" => {
    doc.header.grid_unit = [f(10), f(20)];
}
```

`f` / `i16v` 是现有闭包，无需新 helper（2D 点 read 出来的 `[f64; 2]`
与 model 同形状）。

### 3. Writer（`crates/h7cad-native-dxf/src/writer.rs`）

在 `write_header` 里 `$ATTMODE` 的 pair 之后插入 6 段，紧挨着写——
让 on-disk 输出也保持 "mode flag → 伴生值" 的顺序一致：

```rust
w.pair_str(9, "$ATTMODE");
w.pair_i16(70, doc.header.attmode);

w.pair_str(9, "$SNAPBASE");
w.pair_f64(10, doc.header.snap_base[0]);
w.pair_f64(20, doc.header.snap_base[1]);

w.pair_str(9, "$SNAPUNIT");
w.pair_f64(10, doc.header.snap_unit[0]);
w.pair_f64(20, doc.header.snap_unit[1]);

w.pair_str(9, "$SNAPSTYLE");
w.pair_i16(70, doc.header.snap_style);

w.pair_str(9, "$SNAPANG");
w.pair_f64(50, doc.header.snap_ang);

w.pair_str(9, "$SNAPISOPAIR");
w.pair_i16(70, doc.header.snap_iso_pair);

w.pair_str(9, "$GRIDUNIT");
w.pair_f64(10, doc.header.grid_unit[0]);
w.pair_f64(20, doc.header.grid_unit[1]);
```

### 4. 测试（`crates/h7cad-native-dxf/tests/header_snap_grid.rs`）

新增 **4 条**，覆盖 reader / writer / roundtrip / legacy：

1. `header_reads_snap_grid_family` — 构造 HEADER 里**全部 6 变量
   显式非默认**的 DXF 串，读后 6 个字段按 ground-truth 准确恢复；
   特别断言 `snap_unit.x != snap_unit.y`（0.25 vs 0.5）以确保不是
   读错列。
2. `header_writes_snap_grid_family` — 用非默认值构造 doc → write →
   在输出字符串里按顺序搜索 `$SNAPBASE` / `$SNAPUNIT` / `$SNAPSTYLE`
   / `$SNAPANG` / `$SNAPISOPAIR` / `$GRIDUNIT` 均存在，且 code 10/20
   的值精确匹配（字符串 `contains` 对齐的方式足够——不引入 float
   比较）。
3. `header_roundtrip_preserves_snap_grid_family` — 最关键：构造非默认
   值 → write → read → 6 字段与起点完全相等（`f64` 走 `bit_eq` 风格
   的精确比较，因为 `format_f64` 给 10 位精度下不会真正 drift；若
   出现 drift 也说明 format 本身需要修）。
4. `header_legacy_file_without_snap_grid_loads_with_defaults` — 构造
   **完全没有** `$SNAP* / $GRID*` 的 legacy HEADER，读后 6 字段命中
   `Default` 值。保障现有 legacy 测试不被新字段破坏。

### 5. Ground-truth 值选择（测试里用）

为了每个字段都能通过单独断言暴露 bug：

- `snap_base = [3.25, -7.125]` —— 非原点、含负数、非整数
- `snap_unit = [0.25, 0.5]` —— X ≠ Y（防"读串 code 10/20"）
- `snap_style = 1` —— 等轴测（与默认 0 区分）
- `snap_ang = 0.7853981633974483` —— π/4 = 45°（与默认 0.0 明显不同，
  同时给 format_f64 10 位精度一个"易受 drift"压力）
- `snap_iso_pair = 2` —— 右等轴测面（与默认 0 区分）
- `grid_unit = [1.0, 2.0]` —— X ≠ Y、与 `snap_unit` 不同（防"读到了
  snap_unit 却写到 grid_unit"或反之）

## 实施步骤

| 步骤 | 工作内容 | 预估 |
|---|---|---|
| M1 | `DocumentHeader` 扩 6 字段 + Default | 3 min |
| M2 | reader 6 个 arm | 2 min |
| M3 | writer 6 对 pair | 2 min |
| M4 | 新测试文件 4 条（含 ground-truth 常量） | 10 min |
| M5 | `cargo test -p h7cad-native-dxf` 全绿 | 1 min |
| M6 | `cargo check -p H7CAD`、`cargo check -p h7cad-native-facade` | 1 min |
| M7 | `ReadLints` 改动文件 + CHANGELOG 追加 "二十五" 条目 | 5 min |

## 验收

- `cargo test -p h7cad-native-dxf` **149 → 153**（+4 个 header_snap_grid）
- `cargo check -p H7CAD` 零新 warning
- `cargo check -p h7cad-native-facade` 零新 warning（Facade 不受影响，
  但作为下游的 smoke 确认）
- `ReadLints` 改动的 4 个文件（model lib.rs、dxf lib.rs、writer.rs、
  新 test）零 lint
- CHANGELOG 存在 "2026-04-22（二十五）：SNAP/GRID 家族 6 变量扩充"
  条目，含：动机（与二十四并列的一条自然伴生对）、变更清单、测试
  值选型依据、验证命令
- HEADER 覆盖：82 → **88**（+6）
- 本 plan 文件状态行写 "落地完成"

## 风险

| 风险 | 缓解 |
|---|---|
| `$SNAPBASE` / `$SNAPUNIT` / `$GRIDUNIT` 共享 code 10/20 —— reader 里 `f(10)` / `f(20)` 的 `find` 闭包在当前 arm 上下文隔离（每个 arm 独立 codes 切片），不会跨 arm 串值 | 已被二十三轮的 `$UCSORG/XDIR/YDIR` 三个 3D 点证明——同模式在那里已跑了全部 149 条测试 0 漂移 |
| `snap_style=1` 时 `snap_unit.x` 和 `.y` 应该相等（AutoCAD UI 规则）但我们透传任意值 | io 层非 UI 层；文档里 `//!` 注释已明示；测试用例里特意让它们不等（0.25 vs 0.5）以确认 io 不做验证 |
| `$SNAPANG` 的弧度 π/4 精度 roundtrip 是否会因 `format_f64` 10 位精度漂移 | `π/4 ≈ 0.7853981633974483` 有 16 位有效数字，`format_f64` 的 `{:.10}` 会截到 `0.7853981634`，roundtrip 后再 parse 仍能精确还原（f64 decimal parse 对 10 位小数内无损）。若测试失败即说明 `format_f64` 需要升级到 `{:.17}` 或 `to_string()` shortest-round-trip 风格——届时本 plan 会升级为"format_f64 精度审计"专项 |
| `snap_iso_pair` 在 `snap_style=0` 时无实际语义但 AutoCAD 仍写入 | io 层照 pass；不基于 style 裁剪字段（保持 roundtrip 对称） |

## 执行顺序

M1 → M2 → M3 → M4 → M5 → M6 → M7 → commit（严格串行）

## 与相关 Sprint 的关系

- **不**阻塞 `h7cad-native-dwg` 的 M3B（`real_dwg_samples_baseline_m3b`
  红灯 = 26 LINE vs 40 baseline）；DWG AC1015 entity body 解码是独立
  workstream，本轮 HEADER io 改动与其 0 交集。
- **补齐** `orthomode / gridmode / snapmode` 三元 bool 的伴生值侧；
  下一轮（二十六）自然候选：
  1. `$DIMALT / $DIMALTD / $DIMALTF / $DIMALTRND / $DIMALTTD / $DIMALTTZ / $DIMALTU / $DIMALTZ / $DIMAPOST` —— DIM 替代单位组（9 vars）
  2. `$DISPSILH / $DRAGMODE / $REGENMODE / $SHADEDGE / $SHADEDIF` —— 显示 & 渲染控制（5 vars）
  3. `$LOFTANG1/2 / $LOFTMAG1/2 / $LOFTNORMALS / $LOFTPARAM` —— Loft 3D 默认（6 vars）

## 状态

- [x] 计划定稿（本文件）
- [x] M1 DocumentHeader 6 字段 + Default
- [x] M2 reader 6 arm
- [x] M3 writer 6 对 pair
- [x] M4 新测试文件 4 条
- [x] M5 `cargo test -p h7cad-native-dxf` 149 → 153 全绿
- [x] M6 `cargo check -p H7CAD` / `-p h7cad-native-facade` 零新 warning；`cargo test --bin H7CAD io::native_bridge` 25 / 25
- [x] M7 CHANGELOG "2026-04-22（二十五）" 落地 + ReadLints 零 lint
- [x] 意外收益：`format_f64` 精度升级到 shortest round-trip
      （`f64::to_string()`，保留 `"0.0"` + 整数值补 `.0` 两项约定）
