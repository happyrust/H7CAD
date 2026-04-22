# 开发计划：DXF HEADER 核心尺寸标注 8 变量扩充 (DIMxxx Tier 1)

> 起稿：2026-04-21（第十轮）  
> 前置：HEADER 已覆盖 42 变量（15 原有 + 15 绘图环境 + 4 时间戳 + 5 UCS + 3 视图）。  
> 本轮扩 8 个核心 `$DIM*` 变量（AutoCAD 标注默认外观），不一次性啃整个 DIMxxx 家族（100+ 个）。

## 动机

AutoCAD `$DIM*` HEADER 变量定义当前绘图的 "默认标注外观"——即使用户没定义 DIMSTYLE 表，AutoCAD 也会在 HEADER 里写这些值决定"current drawing defaults"。当前 H7CAD HEADER 只覆盖 `$DIMSCALE`（第 15 原有变量），其他 DIM 变量全丢。

本轮挑**外观层面最常用的 8 个**（文字高度、箭头大小、延伸线、gap、小数位等），让真实 AutoCAD DXF 的标注默认配置完整 round-trip。DIMxxx 家族总计 100+ 变量（很多是实验性 / 历史兼容 / 次要细节），本轮**明确不做 Tier 2+**。

## 目标 8 变量

| 字段 | 类型 | `$` 变量 | DXF code | Default | 语义 |
|---|---|---|---|---|---|
| `dimtxt` | f64 | `$DIMTXT` | 40 | 0.18 | 标注文字高度 |
| `dimasz` | f64 | `$DIMASZ` | 40 | 0.18 | 箭头尺寸 |
| `dimexo` | f64 | `$DIMEXO` | 40 | 0.0625 | 延伸线 origin offset |
| `dimexe` | f64 | `$DIMEXE` | 40 | 0.18 | 延伸线 extension |
| `dimgap` | f64 | `$DIMGAP` | 40 | 0.09 | 标注文字 gap |
| `dimdec` | i16 | `$DIMDEC` | 70 | 4 | 线性尺寸小数位 |
| `dimadec` | i16 | `$DIMADEC` | 70 | 0 | 角度尺寸小数位 |
| `dimtofl` | bool | `$DIMTOFL` | 70 | false | 强制文字位于延伸线之间 |

Defaults 来自 AutoCAD 新文档 (imperial units) 的初始值。

## 非目标

- 不扩 Tier 2+：`$DIMALT*` 替代单位家族（`$DIMALT / $DIMALTD / $DIMALTF / $DIMALTR / $DIMALTTD / $DIMALTTZ / $DIMALTU / $DIMALTZ`）、`$DIMBLK*` 箭头 block name 家族、`$DIMFIT` / `$DIMSAH` / `$DIMSD1` / `$DIMSD2` / `$DIMSE1` / `$DIMSE2` / `$DIMTAD` / `$DIMTIX` / `$DIMTMOVE` / `$DIMUPT` / `$DIMZIN` 等
- 不改 TABLES.DIMSTYLE（独立 `DimStyleProperties` 已有部分字段，DIMxxx 到 DIMSTYLE 的双向同步留未来）
- 不做"单位切换自动调整默认值"（metric vs imperial），恒用 imperial 默认
- 不做"DIMxxx 变量对渲染的实际接入"（仅保真字段；标注渲染路径独立）

## 关键设计

### 1. Model

`DocumentHeader`（`crates/h7cad-native-model/src/lib.rs`）在 `viewdir`（上一轮新增）和 `handseed` 之间插入：

```rust
// Default dimension style (subset — 8 most common Tier-1 variables).
/// `$DIMTXT` (code 40): text height. Default 0.18.
pub dimtxt: f64,
/// `$DIMASZ` (code 40): arrow size. Default 0.18.
pub dimasz: f64,
/// `$DIMEXO` (code 40): extension-line origin offset. Default 0.0625.
pub dimexo: f64,
/// `$DIMEXE` (code 40): extension-line extension. Default 0.18.
pub dimexe: f64,
/// `$DIMGAP` (code 40): dimension-text gap. Default 0.09.
pub dimgap: f64,
/// `$DIMDEC` (code 70): decimal places for linear dims. Default 4.
pub dimdec: i16,
/// `$DIMADEC` (code 70): decimal places for angular dims. Default 0.
pub dimadec: i16,
/// `$DIMTOFL` (code 70): force dim text inside extension lines.
/// Default false.
pub dimtofl: bool,
```

`Default` 填表格中的值。

### 2. Reader

```rust
"$DIMTXT" => doc.header.dimtxt = f(40),
"$DIMASZ" => doc.header.dimasz = f(40),
"$DIMEXO" => doc.header.dimexo = f(40),
"$DIMEXE" => doc.header.dimexe = f(40),
"$DIMGAP" => doc.header.dimgap = f(40),
"$DIMDEC" => doc.header.dimdec = i16v(70),
"$DIMADEC" => doc.header.dimadec = i16v(70),
"$DIMTOFL" => doc.header.dimtofl = i16v(70) != 0,
```

### 3. Writer

按 AutoCAD 输出顺序（`$DIMSCALE` 附近保持邻近）。`$DIMSCALE` 当前在 writer 位置较早（line 120 附近），我把 8 个新变量**聚集插入** `$DIMSCALE` 之后，形成一个完整 DIM 区块：

```rust
w.pair_str(9, "$DIMSCALE");
w.pair_f64(40, doc.header.dimscale);
// ── Dimension defaults (Tier 1) ──
w.pair_str(9, "$DIMASZ");
w.pair_f64(40, doc.header.dimasz);
// ... 余下 7 对 ...
```

### 4. 测试

`tests/header_dim_tier1.rs`：

- `header_reads_all_8_dim_tier1_vars`：非默认值（`dimtxt=0.5, dimasz=0.3, dimdec=6, dimtofl=true` 等）→ 精确读取
- `header_writes_all_8_dim_tier1_vars`：构造 → write → 8 个 `$DIM*` 字符串都在
- `header_roundtrip_preserves_all_8_dim_tier1_vars`：read → write → read，f64 容忍 1e-9
- `header_legacy_file_without_dim_tier1_loads_with_defaults`：legacy → imperial defaults

## 实施步骤

### M1 — model（10 min）

8 pub 字段 + `Default`。

### M2 — reader（10 min）

8 arm 追加。

### M3 — writer（10 min）

8 对 pair 追加到 `$DIMSCALE` 后。

### M4 — 测试（20 min）

`tests/header_dim_tier1.rs`，4 条。

### M5 — validator + CHANGELOG（10 min）

- `cargo test -p h7cad-native-dxf` 113 → **117** (+4)
- `cargo test --bin H7CAD io::native_bridge` 无回归
- CHANGELOG "2026-04-21（十）"

## 风险与缓解

| 风险 | 缓解 |
|---|---|
| `$DIMTOFL` AutoCAD 实际类型是 i16（0/1）不是 bool | 我们透传 i16 → bool（!=0 是 true）并对称写回，语义等价 |
| Metric DXF 期望 metric defaults（2.5 instead of 0.18）| 用户如何配置 drawing units 是另一个层的工作；HEADER defaults 固定 imperial，reader 读 metric 值时字段会被正确覆盖 |
| 其他 native_bridge / UI 代码直接构造 `DocumentHeader { ... }` 缺字段 | `cargo check` 会精确报错；当前 9 轮已验证 `DocumentHeader` 不是被其它 crate 逐字段构造的（用 `Default` + 字段赋值） |

## 验收

- `cargo test -p h7cad-native-dxf` ≥ **117**
- `cargo test --bin H7CAD io::native_bridge` 20/20
- `cargo check -p H7CAD` 零新 warning
- CHANGELOG 条目

## 执行顺序

M1 → M2 → M3 → M4 → M5（严格串行）
