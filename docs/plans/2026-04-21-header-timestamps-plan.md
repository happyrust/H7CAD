# 开发计划：DXF HEADER 时间戳 4 变量扩充（Julian date 透传）

> 起稿：2026-04-21（第七轮）  
> 前置：`docs/plans/2026-04-21-header-drawing-vars-plan.md` 已完成（15 绘图环境变量已覆盖）。本轮继续扩 HEADER 覆盖面，加入 4 个时间戳变量，保留真实 AutoCAD DXF 的 "创建时间 / 最近编辑时间 / 总编辑时长 / 用户计时器" 元数据。

## 动机

真实 AutoCAD 输出的 DXF HEADER 段普遍携带 `$TDCREATE` / `$TDUPDATE` / `$TDINDWG` / `$TDUSRTIMER` 四个时间戳（全 code 40，f64 Julian date 或 fractional days）。当前 H7CAD DXF reader 的 `read_header_section` match 分支没有识别这几个变量，读到后被默认 `_ => {}` 分支丢弃；writer 自然也不写。

后果：读 AutoCAD .dxf → 写回后时间戳全部复位为默认 0.0，历史记录丢失。

**本轮处理原则：不做 Julian Date 转换**。把 4 个值都作为 raw `f64` 存，语义完全保留但**不引入 chrono / time 依赖**。UI 层如需显示人类可读时间，可后续独立加一个 `julian_to_iso_8601` helper（非本轮 scope）。

## 目标

1. `DocumentHeader` 扩 4 字段：
   - `tdcreate: f64`（`$TDCREATE`，code 40，Julian date，default 0.0）
   - `tdupdate: f64`（`$TDUPDATE`，code 40，Julian date，default 0.0）
   - `tdindwg: f64`（`$TDINDWG`，code 40，fractional days，default 0.0）
   - `tdusrtimer: f64`（`$TDUSRTIMER`，code 40，fractional days，default 0.0）
2. Reader 在 `read_header_section` 的 match 中识别这 4 个 `$TD*`
3. Writer 在 `write_header` 中对称输出
4. 测试：read / write / roundtrip / legacy 回落默认；至少 4 条

## 非目标

- 不引入 chrono / time crate 做 Julian date 转换
- 不扩 `$DATE` / `$TDNOW` 等其他时间相关变量（`$DATE` 实际不在 AutoCAD 标准 HEADER 中；`$TDNOW` 是内存变量不落盘）
- 不改 `DocumentHeader` 的 Default 语义（新字段都 default 0.0）
- 不改 writer 输出顺序（新变量追加在 handseed 之前，紧跟现有绘图环境段）

## 关键设计

### 1. Model 扩字段

`crates/h7cad-native-model/src/lib.rs::DocumentHeader` 在 `psltscale` 和 `handseed` 之间插入 4 字段：

```rust
// Timestamp metadata (Julian date or fractional days, f64).
/// `$TDCREATE` (code 40): drawing creation time as Julian date.
pub tdcreate: f64,
/// `$TDUPDATE` (code 40): drawing last-update time as Julian date.
pub tdupdate: f64,
/// `$TDINDWG` (code 40): total editing time in fractional days.
pub tdindwg: f64,
/// `$TDUSRTIMER` (code 40): user-elapsed timer in fractional days.
pub tdusrtimer: f64,
```

`Default` 全部给 0.0。

### 2. Reader 扩派发

在 `read_header_section` 的 match 已有分支末尾加 4 个 arm：

```rust
"$TDCREATE" => doc.header.tdcreate = f(40),
"$TDUPDATE" => doc.header.tdupdate = f(40),
"$TDINDWG" => doc.header.tdindwg = f(40),
"$TDUSRTIMER" => doc.header.tdusrtimer = f(40),
```

现有 `f(c)` helper 已返回 `f64`。

### 3. Writer 扩输出

在 `write_header` 的 `$PSLTSCALE` 之后、`$HANDSEED` 之前插入：

```rust
w.pair_str(9, "$TDCREATE");
w.pair_f64(40, doc.header.tdcreate);

w.pair_str(9, "$TDUPDATE");
w.pair_f64(40, doc.header.tdupdate);

w.pair_str(9, "$TDINDWG");
w.pair_f64(40, doc.header.tdindwg);

w.pair_str(9, "$TDUSRTIMER");
w.pair_f64(40, doc.header.tdusrtimer);
```

### 4. 测试矩阵

新建 `crates/h7cad-native-dxf/tests/header_timestamps.rs`：

- `header_reads_all_4_timestamps`：手写含 4 个 `$TD*` 的 HEADER → 读取 → 每个字段精确（容忍 1e-9）
- `header_writes_all_4_timestamps`：构造 doc 填充 4 字段 → write → 扫 text 找 `$TD*` + code 40 + value
- `header_roundtrip_preserves_all_4_timestamps`：read → write → read，f64 差 < 1e-9
- `header_legacy_file_without_td_fields_loads_with_zero`：legacy HEADER 不含 `$TD*` → 4 字段为 0.0

## 实施步骤

### M1 — model 扩字段（5 min）

`DocumentHeader` 加 4 pub f64 字段 + `Default` 填 0.0。

### M2 — reader（10 min）

`read_header_section` 加 4 arm。

### M3 — writer（10 min）

`write_header` 4 块 pair 输出。

### M4 — 测试（20 min）

`tests/header_timestamps.rs`，4 条测试 + 共享 helper（可复用前轮 `find_var_pair` 的模式）。

### M5 — validator + CHANGELOG（10 min）

- `cargo test -p h7cad-native-dxf` 101 → **105** (+4)
- `cargo test --bin H7CAD io::native_bridge` 20 / 20 无回归
- CHANGELOG 追加 "2026-04-21（七）" 条目

## 风险与缓解

| 风险 | 缓解 |
|---|---|
| 其它处构造 `DocumentHeader { ... }` 缺字段 | `cargo check` 精确定位；前轮同样模式验证过可行 |
| f64 round-trip 精度 | 容忍度 1e-9（与前轮 HEADER plan 一致），`format_f64` 10 位精度足够 |
| Julian date 精度 loss（数量级 2.4e6）| `format_f64` 输出 10 位小数 → Julian date 0.1 秒级精度损失，可接受；不在 1e-9 绝对阈值语境下 |

实际上关于 Julian date 精度：AutoCAD 的 `$TDCREATE` 精确到秒级（1 秒 ≈ 1.16e-5 day）。`format_f64` 的 10 位小数 = 1e-10 day 精度，完全够。

## 验收

- `cargo test -p h7cad-native-dxf` ≥ **105** tests passed
- `cargo test --bin H7CAD io::native_bridge` 20 / 20
- `cargo check -p H7CAD` 零新 warning
- CHANGELOG 条目

## 执行顺序

M1 → M2 → M3 → M4 → M5（严格串行，每步过 compile）
