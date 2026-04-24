# 开发计划：DIMALT 家族 9 变量扩充（二十九）

> 起稿：2026-04-22（第二十九轮）
> 前置：HEADER 已覆盖 103 变量（~34%）。上一轮二十八 LOFT 6 变量
> 让覆盖越过 100 门槛；本轮走 plan §9 下一轮候选的最后一项——
> DIMALT（DIM 替代单位）家族 9 变量，是目前为止**单轮最大规模**。

## 动机

AutoCAD DIM 系统的"替代单位"（alternate units）子系统：主单位
（primary）默认是英制，**替代单位**可以在每个尺寸标注后的括号里
同步显示公制值（e.g. `5.00 [127]` = 5 英寸后面挂 127mm）。整个子
系统由 9 个 `$DIMALT*` HEADER 变量控制：**开关 + 小数位 + 换算
因子 + round + tolerance 小数位 + tolerance 零压缩 + 单位格式 +
主体零压缩 + 后缀**。

H7CAD 之前 reader / writer **全部忽略这 9 个变量**，任何启用过
"替代单位"的 AutoCAD 图纸经过 roundtrip 后会丢失替代单位配置，
尺寸标注的 SI 括注全部蒸发。本轮一次补齐，让启用 DIM alternate
units 的 drawing 完整往返。

## 目标

按 AutoCAD DXF Reference（R14+ 稳定家族）：

| 字段 | 类型 | `$` 变量 | DXF code | Default | 语义 |
|---|---|---|---|---|---|
| `dim_alt` | `i16` | `$DIMALT` | 70 | `0` | 显示替代单位：0 = 关（default），1 = 开 |
| `dim_altd` | `i16` | `$DIMALTD` | 70 | `2` | 替代单位小数位数（default 2） |
| `dim_altf` | `f64` | `$DIMALTF` | 40 | `25.4` | primary → alt 换算因子（默认 inch→mm = 25.4） |
| `dim_altrnd` | `f64` | `$DIMALTRND` | 40 | `0.0` | 替代单位舍入值；0.0 = 不舍入 |
| `dim_alttd` | `i16` | `$DIMALTTD` | 70 | `2` | 替代单位**公差**小数位数 |
| `dim_alttz` | `i16` | `$DIMALTTZ` | 70 | `0` | 替代单位公差零压缩 bitfield（bit1 = 去前导零、bit2 = 去尾 0、bit4 = 去 0 英尺、bit8 = 去 0 英寸） |
| `dim_altu` | `i16` | `$DIMALTU` | 70 | `2` | 替代单位**单位格式**（1 = 科学 / 2 = 小数（默认）/ 3 = 工程 / 4 = 建筑堆叠 / 5 = 分数堆叠 / 6 = 建筑 / 7 = 分数 / 8 = Windows 桌面） |
| `dim_altz` | `i16` | `$DIMALTZ` | 70 | `0` | 替代单位主体零压缩 bitfield（与 `dim_alttz` 同结构） |
| `dim_apost` | `String` | `$DIMAPOST` | 1 | `""` | 替代单位前后缀（`"<>"` 作占位符；例：`"<> mm"` → 数字前不加、后加 " mm"） |

**引入新的子组** `// Dimension alternate units (DIMALT*)` —— 目前
`DocumentHeader` 里的 DIM 字段 15 个（Tier 1 + Tier 2 + dimstyle
name refs），本轮追加 9 让 DIM 总数到 24。子组结构：

```
// Dimension style — Tier 1 (dimtxt/dimasz/dimexo/…)    ← 既有
// Tier-2 dim numerics (dimrnd/dimlfac/dimtdec/…)        ← 既有
// Dimension alternate units (DIMALT*)                   ← 本轮新增
```

位置：插在 `dim_tier_2 子组`（`dimzin`）之后、`splframe` 之前。

## 非目标

- **不**解码 `dim_altu` 枚举的业务含义（UI 负责展示）
- **不**拆 `dim_alttz / dim_altz` 的 bit 为独立 bool（沿用 `dimzin`
  历史决定：i16 原子 bitfield）
- **不**验证 `dim_altf` / `dim_altrnd` 在某个合理范围；AutoCAD 允许
  任意 f64，负数 / 零都透传
- **不**校验 `dim_apost` 里是否必含 `"<>"` 占位符（AutoCAD UI 才关心）
- **不**触碰 DWG 侧
- **不**自动同步 `dim_alt == 0` 时其余 8 个字段的默认值（AutoCAD 存
  "关闭但保留偏好"是合法形式；io 透传）

## 关键设计

### 1. Model（`crates/h7cad-native-model/src/lib.rs`）

紧跟 Tier-2 dim numerics 子组尾 `dimzin`、`splframe` 之前：

```rust
/// `$DIMZIN` (code 70): zero-suppression bitfield for dim text.
pub dimzin: i16,

// Dimension alternate units (DIMALT*) — 9 var family driving the
// "[metric]"-in-brackets parallel display alongside the primary
// imperial value. io layer is pure passthrough; semantic decoding
// (enum meaning of `dim_altu`, bit unpacking of the two *tz / *z
// bitfields, validation of `dim_apost` "<>" placeholder) is all a
// UI / dim-renderer concern.
/// `$DIMALT` (code 70): master on/off for alternate units.
/// 0 = disabled (default), 1 = enabled.
pub dim_alt: i16,
/// `$DIMALTD` (code 70): decimal places for the alt value.
/// Default 2.
pub dim_altd: i16,
/// `$DIMALTF` (code 40): primary → alt conversion factor. Default
/// 25.4 (inch → mm, AutoCAD's historical factory default for mixed
/// imperial/metric dimensioning).
pub dim_altf: f64,
/// `$DIMALTRND` (code 40): round-off applied to the alt value.
/// 0.0 = no rounding (default).
pub dim_altrnd: f64,
/// `$DIMALTTD` (code 70): decimal places for the alt **tolerance**
/// text (distinct from `dim_altd` which governs the main alt value).
/// Default 2.
pub dim_alttd: i16,
/// `$DIMALTTZ` (code 70): alt tolerance zero-suppression bitfield.
/// bit 1 = suppress leading zero, bit 2 = suppress trailing zero,
/// bit 4 = suppress 0-feet, bit 8 = suppress 0-inches. Bits may
/// combine; 0..=15. Default 0.
pub dim_alttz: i16,
/// `$DIMALTU` (code 70): alt-unit format enum.
/// 1 = scientific, 2 = decimal (default), 3 = engineering,
/// 4 = architectural stacked, 5 = fractional stacked,
/// 6 = architectural, 7 = fractional, 8 = Windows desktop.
pub dim_altu: i16,
/// `$DIMALTZ` (code 70): alt-value zero-suppression bitfield.
/// Same bit layout as `dim_alttz`. Default 0.
pub dim_altz: i16,
/// `$DIMAPOST` (code 1): alt-unit text prefix / suffix. `"<>"` is
/// the placeholder for the numeric value (e.g. `"<> mm"` appends
/// " mm" after the value). Default empty — no pre/suffix.
pub dim_apost: String,

// Spline defaults.
/// `$SPLFRAME` (code 70): show spline control polygon. Default false.
pub splframe: bool,
```

`Default::default()` 追加：

```rust
dimzin: 0,

dim_alt: 0,
dim_altd: 2,
dim_altf: 25.4,
dim_altrnd: 0.0,
dim_alttd: 2,
dim_alttz: 0,
dim_altu: 2,
dim_altz: 0,
dim_apost: String::new(),

splframe: false,
```

### 2. Reader（`crates/h7cad-native-dxf/src/lib.rs`）

紧跟 Tier-2 dim numerics arm 组尾（`$DIMZIN` 之后、`$SPLFRAME` 之前）：

```rust
"$DIMZIN" => doc.header.dimzin = i16v(70),

// Dimension alternate units (DIMALT*).
"$DIMALT" => doc.header.dim_alt = i16v(70),
"$DIMALTD" => doc.header.dim_altd = i16v(70),
"$DIMALTF" => doc.header.dim_altf = f(40),
"$DIMALTRND" => doc.header.dim_altrnd = f(40),
"$DIMALTTD" => doc.header.dim_alttd = i16v(70),
"$DIMALTTZ" => doc.header.dim_alttz = i16v(70),
"$DIMALTU" => doc.header.dim_altu = i16v(70),
"$DIMALTZ" => doc.header.dim_altz = i16v(70),
"$DIMAPOST" => doc.header.dim_apost = sv(1).to_string(),
```

### 3. Writer（`crates/h7cad-native-dxf/src/writer.rs`）

紧跟 `$DIMZIN` pair、`$SPLFRAME` pair 之前：

```rust
w.pair_str(9, "$DIMZIN");
w.pair_i16(70, doc.header.dimzin);

// ── Dimension alternate units (DIMALT*) ───────────────────────────────
w.pair_str(9, "$DIMALT");
w.pair_i16(70, doc.header.dim_alt);

w.pair_str(9, "$DIMALTD");
w.pair_i16(70, doc.header.dim_altd);

w.pair_str(9, "$DIMALTF");
w.pair_f64(40, doc.header.dim_altf);

w.pair_str(9, "$DIMALTRND");
w.pair_f64(40, doc.header.dim_altrnd);

w.pair_str(9, "$DIMALTTD");
w.pair_i16(70, doc.header.dim_alttd);

w.pair_str(9, "$DIMALTTZ");
w.pair_i16(70, doc.header.dim_alttz);

w.pair_str(9, "$DIMALTU");
w.pair_i16(70, doc.header.dim_altu);

w.pair_str(9, "$DIMALTZ");
w.pair_i16(70, doc.header.dim_altz);

w.pair_str(9, "$DIMAPOST");
w.pair_str(1, &doc.header.dim_apost);
```

### 4. 测试（`crates/h7cad-native-dxf/tests/header_dim_alt.rs`）

4 条，沿既有模板。由于 6 个字段共享 code 70 + 2 个共享 code 40，
arm-wiring 风险**是本家族最高的一轮**——特意把 6 个 code-70 字段
的 ground-truth 全部两两不相等，让任何串位都会撞至少两个 assert。

### 5. Ground-truth 值选择

- `dim_alt = 1`（≠ 0） — 开关开
- `dim_altd = 3`（≠ 2） — 小数位 3
- `dim_altf = 2.54`（≠ 25.4） — 非默认换算：大约 cm→in
- `dim_altrnd = 0.5`（≠ 0.0） — 圆整到 0.5
- `dim_alttd = 4`（≠ 2 且 ≠ dim_altd=3） — 公差小数位 4
- `dim_alttz = 12`（bit4 + bit8） — 仅压缩 0-feet / 0-inches
- `dim_altu = 3`（≠ 2 default 且 ≠ dim_alt / dim_alttz） — 工程单位
- `dim_altz = 3`（bit1 + bit2） — 主体压缩前后导零
- `dim_apost = "<> mm"` — 经典 "原值 + mm" 后缀

6 个 code-70 字段的 ground-truth `(1, 3, 4, 12, 3, 3)` —— 其中 3
出现两次（`dim_altd` 和 `dim_altz`）。改成 `(1, 3, 4, 12, 3, 5)`
让全部 6 个互不相等：

- `dim_altz = 5`（bit1 + bit4）

最终 6 个 code-70 值：`1, 3, 4, 12, 3, 5` — 唯一重复只有 dim_altd
= 3 与 dim_alttd = … 等等我上面写的 6 个是 dim_alt/dim_altd/
dim_alttd/dim_alttz/dim_altu/dim_altz 六个 code-70 字段；值是
`1, 3, 4, 12, 3, 5` —— dim_altd=3 与 dim_altu=3 相同，再换
dim_altu = 6 避开重复：**`1, 3, 4, 12, 6, 5`**。

- `dim_altu = 6`（架构单位，与 dim_altd = 3 错开）

最终 6 个 code-70 ground-truth： `1, 3, 4, 12, 6, 5` — 两两互不
相等，arm 串位立即暴露。

测试项：

1. `header_reads_dim_alt_family` — 9 字段精确恢复；顺便对 6 个
   code-70 字段做 `assert_ne!(a, b)` 矩阵（3 × 3 = 9 条 assertion
   覆盖所有 pair）
2. `header_writes_dim_alt_family` — 9 `$VAR` 按 reader arm 顺序
   出现 + `dim_apost` 的 `<>` 占位符 verbatim
3. `header_roundtrip_preserves_dim_alt_family` — read → write →
   read 9 字段 bit-identical；`dim_altf=2.54` 作 shortest
   round-trip 哨兵（非整数有效数字值）
4. `header_legacy_file_without_dim_alt_loads_with_defaults` —
   legacy HEADER 缺省命中 AutoCAD 出厂默认
   (`0 / 2 / 25.4 / 0.0 / 2 / 0 / 2 / 0 / ""`)

## 实施步骤

| 步骤 | 工作内容 | 预估 |
|---|---|---|
| M1 | `DocumentHeader` 扩 9 字段 + Default（新子组） | 5 min |
| M2 | reader 9 arm | 3 min |
| M3 | writer 9 对 pair | 3 min |
| M4 | 新测试文件 4 条（含 6-field arm-串位矩阵） | 15 min |
| M5 | `cargo test -p h7cad-native-dxf` 全绿 | 1 min |
| M6 | `cargo check -p H7CAD`、`-p h7cad-native-facade` | 2 min |
| M7 | `ReadLints` + CHANGELOG "二十九" 条目 | 6 min |

总预算约 35 min；比之前 4 轮平均（~20 min）多 ~75%，与规模扩张
（9 vs 4-6）一致。

## 验收

- `cargo test -p h7cad-native-dxf` **165 → 169**（+4 header_dim_alt）
- `cargo test --bin H7CAD io::native_bridge` 25 / 25 不受影响
- `cargo check -p H7CAD` 零新 warning
- `cargo check -p h7cad-native-facade` 零新 warning
- `ReadLints` 改动的 4 个文件零 lint
- CHANGELOG "2026-04-22（二十九）" 条目存在
- HEADER 覆盖：103 → **112**（~37%）
- 本 plan §状态 "落地完成"

## 风险

| 风险 | 缓解 |
|---|---|
| 6 个字段同 code 70（最严重 arm-wiring 堆叠），任一串位都可能静默通过 | Test 里 6 值两两互不相等 + 3×3=9 assert_ne 矩阵，覆盖所有 pair；任一串位必撞 ≥2 条 assert |
| `dim_altf = 2.54` 这类带 2-3 位有效数字的 f64 在写出路径是否无损 | 二十五轮 `format_f64` shortest round-trip 已保证所有 f64 bit-identical；本轮仅作为烟测哨兵 |
| `dim_apost = "<> mm"` 里的 `<>` 尖括号若经某 escape 路径会出错 | AutoCAD DIM 规范保证 `<>` 是 DIM 文本占位符，在 DXF 文件里 verbatim 存储无转义；测试 `contains("<> mm")` 即可验证 |
| 将 `dim_alt` 存 `i16` 而非 `bool`，与 `fillmode / orthomode` 等历史 bool 形成不一致 | 整个 DIM 家族（`dimtofl`、`dim_alt`、`dim_alttz`…）全部 code 70 / i16，保持**家族内一致性**胜于跨家族一致性；下游 `!= 0` 判真假 |

## 执行顺序

M1 → M2 → M3 → M4 → M5 → M6 → M7 → commit（严格串行）

## 后续工作指向（二十九完成后）

本轮是 plan §9 候选队列里**最后一个 HEADER 中等规模家族**。完成
后 HEADER 覆盖 ~37%，继续按"一天一轮、一轮 4-9 变量"的节奏还能
再跑 20-30 轮，之后 HEADER 扩展会进入长尾阶段（剩余大多为罕用
变量 / R2018 之后新增 / 特定场景 flag）。

合理候选（二十九完成后再评估）：

1. **继续 HEADER 长尾**（每轮规模收缩到 3-5 变量）—— 适合保持
   cadence 但边际价值下降
2. **转向 entity 覆盖**——目前 EntityData 40 变体，AutoCAD 完整
   ~100；补齐 RAY/XLINE/OLE2FRAME/ACAD_PROXY_ENTITY 等
3. **修复 DWG 红灯**（`real_dwg_samples_baseline_m3b`：sample_AC1015.dwg
   仅 26/40 LINE 恢复，body_decode_fail 82 次）——上层 product
   价值最高，facade rollout 前置条件
4. **`format_f64` 精度审计**（rounds 25-28 的意外 by-product 已修
   writer；但 reader 是否容忍 shortest-round-trip 的 tail 值？
   各 DXF header/entity 场景的 f64 robustness 还未被 fuzz）

推荐在二十九完成后**暂停 HEADER 扩展**，写一个 roadmap 文档评估
"继续 HEADER vs 转 entity vs 修 DWG" 三条路径的 ROI，让 owner
决定。

## 状态

- [x] 计划定稿（本文件）
- [x] M1 DocumentHeader 9 字段 + Default（新 `dim_alt_*` / `dim_apost` 子组）
- [x] M2 reader 9 arm
- [x] M3 writer 9 对 pair
- [x] M4 新测试文件 4 条（含 15 条 arm-串位 regression 矩阵）
- [x] M5 `cargo test -p h7cad-native-dxf` 165 → 169 全绿
- [x] M6 `cargo check -p H7CAD` / `-p h7cad-native-facade` 零新 warning；`cargo test --bin H7CAD io::native_bridge` 25 / 25
- [x] M7 CHANGELOG "2026-04-22（二十九）" 落地 + ReadLints 零 lint
- [x] 里程碑：DIM 子系统覆盖 15 → 24（+60%）；plan §9 下一轮候选队列清空
