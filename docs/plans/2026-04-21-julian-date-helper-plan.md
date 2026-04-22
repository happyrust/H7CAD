# 开发计划：Julian Date ↔ UTC 转换 helper（无 chrono 依赖）

> 起稿：2026-04-21（第十一轮）  
> 前置：`docs/plans/2026-04-21-header-timestamps-plan.md` 已把 `$TDCREATE / $TDUPDATE / $TDINDWG / $TDUSRTIMER` 4 个时间戳作为 f64 Julian date / fractional days 透传入 `DocumentHeader`。本轮补上 UI 层 format 时需要的转换 helper，明确保持"不引入 `chrono` / `time` crate"的承诺。

## 动机

`DocumentHeader.tdcreate = 2458849.82939815` 是 raw Julian date，对用户完全不可读。UI 要显示"2020-01-01 07:54:19 UTC"必须做转换。前轮 plan 明确"等需要时再加 helper"——本轮就是那个"需要时"。

两种实现选择：

1. **引入 `chrono` crate**：成熟 / 多 feature / 绑定 `serde` / 毫秒粒度。代价：workspace 新依赖、编译时间 / binary size 增量、特性膨胀
2. **自写 Fliegel-Van Flandern 算法（1968）**：~30 行 integer-only 代码，零依赖，**秒级精度**（满足 AutoCAD 时间戳语义）。AutoCAD 本身也是这个粒度

本轮选 **方案 2**，理由：
- AutoCAD Julian date 本身就没 sub-second 粒度（`format_f64` 10 位小数 → ~1 ms 但实际不可信）
- H7CAD workspace 保持最小依赖策略
- Fliegel-Van Flandern 是经过 50 年验证的公开算法，Wikipedia / USNO 都能直接拷源码

## 目标

1. 新增 `crates/h7cad-native-model/src/julian.rs`：
   - `pub struct DateTimeUtc { year: i32, month: u32, day: u32, hour: u32, minute: u32, second: u32 }`
   - `pub fn julian_date_to_utc(jd: f64) -> DateTimeUtc`
   - `pub fn utc_to_julian_date(dt: &DateTimeUtc) -> f64`
   - `pub fn format_iso8601(dt: &DateTimeUtc) -> String` — `"2020-01-01T07:54:19Z"`
2. `lib.rs` pub mod / pub use 导出
3. 单测覆盖：
   - DXF Reference 样例值 `2458849.82939815` → 2020-01-01 ~07:54:19 UTC（断言 year / month / day 精确，时分秒容忍 1 秒）
   - Unix epoch `2440587.5` → 1970-01-01 00:00:00 UTC
   - Julian date 2000 年 1 月 1 日正午（`J2000.0 = 2451545.0`）→ 2000-01-01 12:00:00 UTC
   - Round-trip：`utc_to_julian_date(julian_date_to_utc(jd))` 误差 < 1e-6 日（~0.1 秒）
   - `format_iso8601` 对已知 DateTimeUtc 产出正确字符串

## 非目标

- 不支持闰秒（Fliegel-Van Flandern 不考虑）
- 不支持 nanosecond 精度（AutoCAD 时间戳本身只秒级）
- 不支持时区（UTC 唯一）
- 不支持解析 ISO-8601 字符串（反向转换留将来）
- 不接入 UI 显示（本轮仅 model crate 内部 helper）
- 不引入 `chrono` / `time` crate

## 关键算法

Fliegel-Van Flandern (1968) 把 Julian date 整数部分 JDN 转换成 Gregorian 年月日，整数运算：

```rust
fn jdn_to_gregorian(jdn: i64) -> (i32, u32, u32) {
    // JDN = floor(JD + 0.5)
    let l = jdn + 68_569;
    let n = (4 * l) / 146_097;
    let l = l - (146_097 * n + 3) / 4;
    let i = (4_000 * (l + 1)) / 1_461_001;
    let l = l - (1_461 * i) / 4 + 31;
    let j = (80 * l) / 2_447;
    let day = (l - (2_447 * j) / 80) as u32;
    let l = j / 11;
    let month = (j + 2 - 12 * l) as u32;
    let year = (100 * (n - 49) + i + l) as i32;
    (year, month, day)
}
```

小数部分转成 HH:MM:SS：

```rust
// Julian date 的 "noon" 对应整日切换
let noon_offset = jd + 0.5;
let frac = noon_offset - noon_offset.floor();  // [0, 1)
let total_seconds = (frac * 86_400.0).round() as u64;
let hour = (total_seconds / 3600) as u32;
let minute = ((total_seconds % 3600) / 60) as u32;
let second = (total_seconds % 60) as u32;
```

反向（UTC → Julian date）用 Meeus 公式（整数运算）：

```rust
fn gregorian_to_jdn(y: i32, m: u32, d: u32) -> i64 {
    let (y, m) = if m <= 2 { (y - 1, m + 12) } else { (y, m) };
    let a = y / 100;
    let b = 2 - a + a / 4;
    let jdn = (365.25 * (y + 4716) as f64).floor() as i64
        + (30.6001 * (m + 1) as f64).floor() as i64
        + d as i64 + b as i64 - 1524;
    jdn
}
```

小数部分：`(hour * 3600 + minute * 60 + second) / 86400.0 - 0.5`（Julian date noon 偏移）。

## 实施步骤

### M1 — 新增 julian 模块（30 min）

`crates/h7cad-native-model/src/julian.rs`，含上述 3 函数 + `DateTimeUtc` struct。

### M2 — 在 `lib.rs` 导出（5 min）

```rust
pub mod julian;
pub use julian::{DateTimeUtc, julian_date_to_utc, utc_to_julian_date, format_iso8601};
```

### M3 — 单测（30 min）

在 `julian.rs` 底部 `#[cfg(test)] mod tests`：

- `julian_date_reference_value_maps_to_utc`：`2458849.82939815` → 2020-01-01 / 时分秒误差 ≤ 1 秒
- `unix_epoch_julian_date_maps_to_1970`：`2440587.5` → 1970-01-01 00:00:00
- `j2000_maps_to_2000_noon`：`2451545.0` → 2000-01-01 12:00:00
- `julian_date_roundtrip_preserves_date_to_sub_second_precision`：4 种日期轮回（1900 / 2000 / 2020 / 2100）误差 < 1e-6 日
- `format_iso8601_produces_canonical_string`

### M4 — validator + CHANGELOG（10 min）

- `cargo test -p h7cad-native-model` 0 → **5** (+5 新)
- `cargo check -p H7CAD` 零新 warning
- CHANGELOG "2026-04-21（十一）"

## 风险与缓解

| 风险 | 缓解 |
|---|---|
| Fliegel 算法边界（BC 时间 / 异常 JDN）| AutoCAD 时间戳从不早于 1900；约束输入范围 [2415020, 2488070]（1900 Jan 1 – 2100 Jan 1）足够 |
| JDN rounding：`floor(jd + 0.5)` vs `jd as i64` | 用前者（Fliegel 原始定义），避免负 Julian date 错位 |
| `(365.25 * y).floor() as i64` i32 cast 精度 | 年份范围合理，在 i64 下无溢出 |
| UTC 假定 DXF 时间戳都是 UTC 实则可能是本地时间 | AutoCAD DXF Reference 明确说 `$TDCREATE` 是 "local time"，但 H7CAD 的 helper 语义仍作为 UTC 处理（H7CAD 不假定 timezone）；doc comment 注明 |

## 验收

- `cargo test -p h7cad-native-model` ≥ **5** tests passed
- `cargo test -p h7cad-native-dxf` 117/117 保持（不受影响）
- `cargo check -p H7CAD` 零新 warning
- CHANGELOG 条目

## 执行顺序

M1 → M2 → M3 → M4（严格串行）
