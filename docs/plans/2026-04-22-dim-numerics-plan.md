# 开发计划：DXF HEADER Tier 2 尺寸数字家族扩充（二十二）

> 起稿：2026-04-22（第二十二轮）  
> 前置：HEADER 已覆盖 71 变量（~23%）。本轮补 **6 个尺寸数字格式化变量**：
> `$DIMRND / $DIMLFAC / $DIMTDEC / $DIMFRAC / $DIMDSEP / $DIMZIN`。
> 覆盖推到 77。

## 动机

AutoCAD 的**尺寸数字格式化**家族有 20+ 个 `$DIM*` 变量。H7CAD 前几轮已
落地了 Tier 1 的 10 个（`DIMTXT / DIMASZ / DIMEXO / DIMEXE / DIMGAP /
DIMDEC / DIMADEC / DIMTOFL / DIMSTYLE / DIMTXSTY`），主要覆盖尺寸**几何
布局**。Tier 2 补齐**数字格式化**侧的 6 个最常用项：

1. **`$DIMRND`**（code 40）：舍入精度。如 `0.5` 表示所有标注结果四舍五入
   到 0.5。Default 0.0（不舍入，使用原始测量值）。
2. **`$DIMLFAC`**（code 40）：线性缩放因子。所有线性测量乘以这个值后
   再显示。Default 1.0。用于图纸单位换算场景。
3. **`$DIMTDEC`**（code 70）：**公差文本**的小数位数（独立于主尺寸文本
   的 `$DIMDEC`）。Default 4。
4. **`$DIMFRAC`**（code 70）：分数格式：0 = 水平堆叠，1 = 斜线堆叠，
   2 = 不堆叠。Default 0。仅在 `$DIMLUNIT` 设为分数单位时生效。
5. **`$DIMDSEP`**（code 70）：十进制分隔符的 ASCII 码。46 = `.`，
   44 = `,`（欧洲地区）。Default 46。
6. **`$DIMZIN`**（code 70）：前导 / 尾随零抑制规则位字段：
   - 0 = 保留前导 0 / 尾随 0（默认）
   - 1 = 抑制前导 0（0.5 → .5）
   - 2 = 抑制尾随 0（1.500 → 1.5）
   - 4 = 抑制 0 英尺（0'-6" → 6"）
   - 8 = 抑制 0 英寸（1'-0" → 1'）
   这些 bit 可以组合，所以 `$DIMZIN` 的取值范围是 0-15。Default 0。

没有这 6 个，任何涉及非默认尺寸数字格式的 AutoCAD .dxf 被 H7CAD
roundtrip 后，尺寸显示格式都会静默退回默认状态。

## 目标字段

| 字段 | 类型 | `$` 变量 | DXF code | Default | 语义 |
|---|---|---|---|---|---|
| `dimrnd` | `f64` | `$DIMRND` | 40 | `0.0` | 舍入精度 |
| `dimlfac` | `f64` | `$DIMLFAC` | 40 | `1.0` | 线性缩放因子 |
| `dimtdec` | `i16` | `$DIMTDEC` | 70 | `4` | 公差小数位数 |
| `dimfrac` | `i16` | `$DIMFRAC` | 70 | `0` | 分数堆叠格式 |
| `dimdsep` | `i16` | `$DIMDSEP` | 70 | `46` | 小数分隔符 ASCII |
| `dimzin` | `i16` | `$DIMZIN` | 70 | `0` | 零抑制位字段 |

## 非目标

- **不**补 `$DIMALT*` 备选单位家族（另起一组，涉及 10+ 变量）
- **不**动 `$DIMSTYLE` 表（`DIMSTYLE` SymbolTable 的 entry-level 字段
  与 HEADER 默认值独立，后续单独扩）
- **不**验证 `$DIMZIN` 各 bit 组合的业务含义（reader/writer 纯 i16
  透传，含义由上层渲染层决定）

## 关键设计

### 1. Model（`crates/h7cad-native-model/src/lib.rs`）

在 `DocumentHeader` 的 Tier 1 dim 块（`dimtxsty` 之后）追加：

```rust
/// `$DIMTXSTY` (code 7): current dimension text style name.
/// Default `"Standard"`.
pub dimtxsty: String,

// Tier-2 dim numerics (formatting of measurement text).
/// `$DIMRND` (code 40): rounding value for dim measurements.
/// `0.0` means no rounding (raw measured value). Default 0.0.
pub dimrnd: f64,
/// `$DIMLFAC` (code 40): linear measurement scale factor. All linear
/// dims are multiplied by this before display. Default 1.0.
pub dimlfac: f64,
/// `$DIMTDEC` (code 70): decimal places for tolerance text (distinct
/// from `$DIMDEC` which governs the main dim text). Default 4.
pub dimtdec: i16,
/// `$DIMFRAC` (code 70): fraction format: 0 = horizontal stacked,
/// 1 = diagonal stacked, 2 = not stacked. Only meaningful when
/// `$DIMLUNIT` selects a fractional unit. Default 0.
pub dimfrac: i16,
/// `$DIMDSEP` (code 70): decimal separator as ASCII code point.
/// 46 = `.` (default), 44 = `,` (European).
pub dimdsep: i16,
/// `$DIMZIN` (code 70): zero-suppression bitfield.
/// bit 1 = suppress leading zero, bit 2 = suppress trailing zero,
/// bit 4 = suppress 0-feet, bit 8 = suppress 0-inches. Default 0.
pub dimzin: i16,
```

`Default::default` 追加：`dimrnd: 0.0, dimlfac: 1.0, dimtdec: 4,
dimfrac: 0, dimdsep: 46, dimzin: 0`。

### 2. Reader（`crates/h7cad-native-dxf/src/lib.rs`）

在 Tier 1 dim 的 `"$DIMTXSTY"` arm 之后、spline 分组之前追加：

```rust
// Tier-2 dim numerics.
"$DIMRND" => doc.header.dimrnd = f(40),
"$DIMLFAC" => doc.header.dimlfac = f(40),
"$DIMTDEC" => doc.header.dimtdec = i16v(70),
"$DIMFRAC" => doc.header.dimfrac = i16v(70),
"$DIMDSEP" => doc.header.dimdsep = i16v(70),
"$DIMZIN" => doc.header.dimzin = i16v(70),
```

### 3. Writer（`crates/h7cad-native-dxf/src/writer.rs`）

在 `$DIMTXSTY` pair 之后追加 6 对。保留当前 "Tier-2 dim numerics" 分组
注释以便下轮再补 Tier 3 时同组扩展。

### 4. 测试（`crates/h7cad-native-dxf/tests/header_dim_numerics.rs`）

4 条：

- `header_reads_all_6_dim_numerics`：非默认值精确读入
- `header_writes_all_6_dim_numerics`：构造 → write → 6 个 `$VAR` 字符串存在
- `header_roundtrip_preserves_all_6_dim_numerics`：全字段 roundtrip 保持
- `header_legacy_file_without_dim_numerics_loads_with_defaults`：
  缺省 → 检查各自 Default（注意 `dimlfac=1.0`, `dimtdec=4`, `dimdsep=46`
  而不是 0）

## 实施步骤

| 步骤 | 工作内容 | 预估 |
|---|---|---|
| M1 | `DocumentHeader` 扩 6 字段 + `Default` | 6 min |
| M2 | reader 6 个 arm | 2 min |
| M3 | writer 6 对 pair | 3 min |
| M4 | 新测试文件 4 条 | 12 min |
| M5 | test + check + ReadLints + CHANGELOG | 8 min |

## 验收

- `cargo test -p h7cad-native-dxf` 137 → **141** (+4)
- `cargo test --bin H7CAD io::native_bridge` 25 / 25 不受影响
- `cargo check -p H7CAD` 零新 warning
- `ReadLints` 4 个文件零 lint
- CHANGELOG "2026-04-22（二十二）" 条目存在
- HEADER 覆盖：71 → **77**

## 风险

| 风险 | 缓解 |
|---|---|
| `$DIMLFAC` 正值 vs 负值（负 = 仅应用于 paper-space 引用）差异 | 纯 f64 透传，不做符号语义判断；业务层再解读 |
| `$DIMDSEP=44`（欧洲） 与 CSV 混淆 | 注释明确是 ASCII 码，与文件 IO 编码无关 |
| `$DIMZIN` 位字段 0-15 全取值 | 测试用值选 `3`（bit 1+2）验证 bitfield 能精确保持 |

## 执行顺序

M1 → M2 → M3 → M4 → M5 → commit（严格串行）
