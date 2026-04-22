# 开发计划：DXF HEADER Spline + MLine 6 变量扩充

> 起稿：2026-04-21（第十二轮）  
> 前置：HEADER 已覆盖 50 变量。本轮再补 6 个常见次要变量（Spline 默认 3 个 + 当前 MLine 3 个），把 HEADER 总覆盖推到 56。

## 动机

Spline / MLine 是 AutoCAD 常用实体类型，他们的"当前默认"配置存在 HEADER 段：

- `$SPLFRAME / $SPLINESEGS / $SPLINETYPE` — Spline 显示控制多边形 / 段数 / 类型
- `$CMLSTYLE / $CMLJUST / $CMLSCALE` — 当前 MLine 风格 / 对齐 / 比例

H7CAD 当前 reader 忽略，writer 不写。读 AutoCAD .dxf 写回后这 6 个设置丢失。

## 目标 6 变量

| 字段 | 类型 | `$` 变量 | DXF code | Default |
|---|---|---|---|---|
| `splframe` | bool | `$SPLFRAME` | 70 | false (0) |
| `splinesegs` | i16 | `$SPLINESEGS` | 70 | 8 |
| `splinetype` | i16 | `$SPLINETYPE` | 70 | 6 (cubic B-spline) |
| `cmlstyle` | String | `$CMLSTYLE` | 2 | `"Standard"` |
| `cmljust` | i16 | `$CMLJUST` | 70 | 0 (top) |
| `cmlscale` | f64 | `$CMLSCALE` | 40 | 1.0 |

`$SPLINETYPE` 值域：5 = quadratic B-spline, 6 = cubic B-spline。
`$CMLJUST` 值域：0 = top, 1 = middle, 2 = bottom。

## 非目标

- 不接入 Spline 实体的实际 segs / type 渲染（HEADER 仅 default，实体自带 override）
- 不动 MLINE entity 的 style / justification 处理（已有独立字段）
- 不扩 `$SPLINEDIT` 等高级 spline 编辑变量

## 关键设计

### 1. Model

`DocumentHeader`（插在 DIM Tier 1 之后 / `handseed` 之前）：

```rust
// Spline defaults.
/// `$SPLFRAME` (code 70): show spline control polygon. Default false.
pub splframe: bool,
/// `$SPLINESEGS` (code 70): line segments per spline patch.
/// Default 8.
pub splinesegs: i16,
/// `$SPLINETYPE` (code 70): default spline curve type
/// (5 = quadratic, 6 = cubic). Default 6.
pub splinetype: i16,

// Multi-line (MLINE) defaults.
/// `$CMLSTYLE` (code 2): current MLine style name. Default "Standard".
pub cmlstyle: String,
/// `$CMLJUST` (code 70): current MLine justification
/// (0 = top, 1 = middle, 2 = bottom). Default 0.
pub cmljust: i16,
/// `$CMLSCALE` (code 40): current MLine scale factor. Default 1.0.
pub cmlscale: f64,
```

`Default` 填上述。

### 2. Reader

```rust
"$SPLFRAME" => doc.header.splframe = i16v(70) != 0,
"$SPLINESEGS" => doc.header.splinesegs = i16v(70),
"$SPLINETYPE" => doc.header.splinetype = i16v(70),
"$CMLSTYLE" => doc.header.cmlstyle = sv(2).to_string(),
"$CMLJUST" => doc.header.cmljust = i16v(70),
"$CMLSCALE" => doc.header.cmlscale = f(40),
```

### 3. Writer

按 AutoCAD 顺序追加在 DIM Tier 1 之后：

```rust
// ── Spline defaults ──
w.pair_str(9, "$SPLFRAME");
w.pair_i16(70, if doc.header.splframe { 1 } else { 0 });
w.pair_str(9, "$SPLINETYPE");
w.pair_i16(70, doc.header.splinetype);
w.pair_str(9, "$SPLINESEGS");
w.pair_i16(70, doc.header.splinesegs);

// ── MLine defaults ──
w.pair_str(9, "$CMLSTYLE");
w.pair_str(2, &doc.header.cmlstyle);
w.pair_str(9, "$CMLJUST");
w.pair_i16(70, doc.header.cmljust);
w.pair_str(9, "$CMLSCALE");
w.pair_f64(40, doc.header.cmlscale);
```

### 4. 测试

`tests/header_spline_mline.rs`：4 条（read / write / roundtrip / legacy 默认）

## 实施步骤

### M1 — model（5 min）
### M2 — reader（5 min）
### M3 — writer（5 min）
### M4 — 测试（15 min）
### M5 — validator + CHANGELOG（10 min）

## 风险

| 风险 | 缓解 |
|---|---|
| `$SPLINETYPE` 取值非 5/6 | reader 直接透传 i16，不校验；上层语义不强制 |
| `$CMLJUST` 取值非 0/1/2 | 同上，透传 |
| `$CMLSTYLE` 引用的 MLineStyle 字典缺失 | reader / writer 不校验存在性，仅字符串透传 |

## 验收

- `cargo test -p h7cad-native-dxf` 117 → **121** (+4)
- `cargo test --bin H7CAD io::native_bridge` 20/20
- `cargo check -p H7CAD` 零新 warning
- CHANGELOG "2026-04-21（十二）"

## 执行顺序

M1 → M2 → M3 → M4 → M5（严格串行）
