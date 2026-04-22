# 开发计划：LOFT 3D 默认家族 6 变量扩充（二十八）

> 起稿：2026-04-22（第二十八轮）
> 前置：HEADER 已覆盖 97 变量（~32%）。上一轮二十七补完 drawing
> 元数据附加 4 变量（String × 2 + i16 + bool 混合首轮）。本轮走 plan
> §9 下一轮候选里的 Loft 3D 默认家族——6 变量、**4 × f64 + 2 × i16**，
> 比 display/render 家族规模稍大，和 snap/grid 家族同级。

## 动机

AutoCAD R2007+ 提供 LOFT 命令，通过一组横截面 + 可选路径曲线生成 3D
实体 / 曲面。LOFT 的默认行为由 6 个 HEADER 变量驱动：

- 起 / 止横截面的 **draft angle + magnitude**（4 × f64）—— 决定边缘
  切线方向
- **normals 控制**（i16 枚举 0–6）—— 决定曲面法向量来源
- **param bitfield**（i16 位字段）—— twist / alignment / 表面质量 /
  是否周期性闭合

H7CAD 之前 reader / writer **全部忽略这 6 个变量**，用户保存的 LOFT
偏好在 roundtrip 后归默认。本轮一次补齐，让 "用户自定义 LOFT 预设 →
保存 → H7CAD 读回 / 写出" 三步闭环。

## 目标

### 字段

按 AutoCAD DXF Reference R2007+：

| 字段 | 类型 | `$` 变量 | DXF code | Default | 语义 |
|---|---|---|---|---|---|
| `loft_ang1` | `f64` | `$LOFTANG1` | 40 | `0.0` | 起横截面 draft angle（弧度） |
| `loft_ang2` | `f64` | `$LOFTANG2` | 40 | `0.0` | 止横截面 draft angle（弧度） |
| `loft_mag1` | `f64` | `$LOFTMAG1` | 40 | `0.0` | 起横截面 draft magnitude |
| `loft_mag2` | `f64` | `$LOFTMAG2` | 40 | `0.0` | 止横截面 draft magnitude |
| `loft_normals` | `i16` | `$LOFTNORMALS` | 70 | `1` | 曲面法向量控制枚举（0 = ruled / 1 = smooth fit（AutoCAD 默认）/ 2 = 起横截面 / 3 = 止横截面 / 4 = 起与止 / 5 = 全横截面 / 6 = 路径） |
| `loft_param` | `i16` | `$LOFTPARAM` | 70 | `7` | bitfield（1 = no twist、2 = aligned directions、4 = simple surfaces、8 = closed / periodic；AutoCAD 默认 1+2+4 = 7） |

插入位置：`DocumentHeader` 里紧跟二十七轮的 drawing 元数据附加组
（`$OLESTARTUP` 之后，几何命令默认 `$CHAMFERA` 之前），形成 "元数据
附加 → Loft 3D 默认 → 交互几何命令默认" 的递进语义链。

### 默认值选型

- `loft_ang*` / `loft_mag*` = `0.0` —— AutoCAD 默认（零 draft =
  横截面切面即曲面切面，无偏移）
- `loft_normals = 1` —— AutoCAD 出厂默认 = smooth fit（最通用）
- `loft_param = 7` —— AutoCAD 出厂默认 = 三 flag 同开（no twist +
  aligned + simple surfaces），不闭合

全部对齐 AutoCAD factory default。

## 非目标

- **不**解析 `loft_normals` 的枚举值含义（由 UI / 渲染层解读）
- **不**拆 `loft_param` 的各 bit 为独立 `bool`（io 存储为原子 i16
  bitfield，与 `indexctl` 的处理策略一致）
- **不**验证 `loft_ang*` 在 `[-π, π]` 合理范围内（AutoCAD 允许任意
  f64；io passthrough）
- **不**触碰 DWG 侧 / `real_dwg_samples_baseline_m3b` 红灯

## 关键设计

### 1. Model（`crates/h7cad-native-model/src/lib.rs`）

紧跟二十七轮 `olestartup` 之后、`chamfera` 之前：

```rust
pub olestartup: bool,

// Loft 3D defaults — R2007+ LOFT command driver (4 × f64 draft params
// + 2 × i16 normals / flags). io layer is pure passthrough; semantic
// meaning of `loft_normals` enum values and `loft_param` bit flags is
// AutoCAD-documented and UI-decoded.
/// `$LOFTANG1` (code 40): start cross-section draft angle, radians.
/// Default 0.0.
pub loft_ang1: f64,
/// `$LOFTANG2` (code 40): end cross-section draft angle, radians.
/// Default 0.0.
pub loft_ang2: f64,
/// `$LOFTMAG1` (code 40): start cross-section draft magnitude.
/// Default 0.0.
pub loft_mag1: f64,
/// `$LOFTMAG2` (code 40): end cross-section draft magnitude.
/// Default 0.0.
pub loft_mag2: f64,
/// `$LOFTNORMALS` (code 70): lofted surface normals source.
/// 0 = ruled, 1 = smooth fit (default), 2 = start cross-section,
/// 3 = end cross-section, 4 = start and end, 5 = all cross-sections,
/// 6 = path.
pub loft_normals: i16,
/// `$LOFTPARAM` (code 70): lofted surface option bitfield.
/// bit 1 = no twist, bit 2 = align directions, bit 4 = simple
/// surfaces, bit 8 = closed / periodic. AutoCAD default 7
/// (1 + 2 + 4 = three flags on, not closed).
pub loft_param: i16,

// Interactive geometry command defaults.
/// `$CHAMFERA` (code 40): first chamfer distance. Default 0.0.
pub chamfera: f64,
```

`Default::default()` 追加：

```rust
olestartup: false,

loft_ang1: 0.0,
loft_ang2: 0.0,
loft_mag1: 0.0,
loft_mag2: 0.0,
loft_normals: 1,
loft_param: 7,

chamfera: 0.0,
```

### 2. Reader

紧跟 `$OLESTARTUP` arm：

```rust
"$OLESTARTUP" => doc.header.olestartup = bv(290),

// Loft 3D defaults (R2007+ LOFT command).
"$LOFTANG1" => doc.header.loft_ang1 = f(40),
"$LOFTANG2" => doc.header.loft_ang2 = f(40),
"$LOFTMAG1" => doc.header.loft_mag1 = f(40),
"$LOFTMAG2" => doc.header.loft_mag2 = f(40),
"$LOFTNORMALS" => doc.header.loft_normals = i16v(70),
"$LOFTPARAM" => doc.header.loft_param = i16v(70),
```

### 3. Writer

紧跟 `$OLESTARTUP` pair：

```rust
w.pair_str(9, "$OLESTARTUP");
w.pair_i16(290, if doc.header.olestartup { 1 } else { 0 });

// ── Loft 3D defaults ──────────────────────────────────────────────────
w.pair_str(9, "$LOFTANG1");
w.pair_f64(40, doc.header.loft_ang1);

w.pair_str(9, "$LOFTANG2");
w.pair_f64(40, doc.header.loft_ang2);

w.pair_str(9, "$LOFTMAG1");
w.pair_f64(40, doc.header.loft_mag1);

w.pair_str(9, "$LOFTMAG2");
w.pair_f64(40, doc.header.loft_mag2);

w.pair_str(9, "$LOFTNORMALS");
w.pair_i16(70, doc.header.loft_normals);

w.pair_str(9, "$LOFTPARAM");
w.pair_i16(70, doc.header.loft_param);
```

### 4. 测试（`crates/h7cad-native-dxf/tests/header_loft.rs`）

4 条，沿用家族测试 4-合 1 模板：

1. `header_reads_loft_family` — 6 字段非默认值精确恢复；特别用
   **4 个不相等的 f64 值** 对 `loft_ang1 / 2 / loft_mag1 / 2` 做
   arm 串位 regression guard（共享 code 40，串位就会出现两字段相等）
2. `header_writes_loft_family` — 6 个 `$VAR` 按 reader arm 顺序出现
3. `header_roundtrip_preserves_loft_family` — read → write → read
   6 字段 bit-identical；特别给 `loft_ang1` 塞一个 π/6 这类常用弧度
   **无损数学常量**，验证 shortest round-trip `format_f64`（二十五轮
   升级）在本家族也成立
4. `header_legacy_file_without_loft_loads_with_defaults` — 缺省命中
   (`0 / 0 / 0 / 0 / 1 / 7`)

### 5. Ground-truth 值选择

让每字段都**与 Default 不同**且 4 个 f64 **两两互不相等**：

- `loft_ang1 = π/6 ≈ 0.5235987755982989` —— 常用 30° 斜面；也顺便
  测 f64 shortest round-trip
- `loft_ang2 = π/3 ≈ 1.0471975511965976` —— 60° 斜面
- `loft_mag1 = 1.5`
- `loft_mag2 = 2.5`
- `loft_normals = 6` —— 路径法向（与 default 1 不同）
- `loft_param = 9` —— bit1(no twist) + bit8(closed) 的罕见组合，覆盖
  "no twist 下做 closed loft" 的合法位组合

## 实施步骤

| 步骤 | 工作内容 | 预估 |
|---|---|---|
| M1 | `DocumentHeader` 扩 6 字段 + Default | 3 min |
| M2 | reader 6 arm | 2 min |
| M3 | writer 6 对 pair | 2 min |
| M4 | 新测试文件 4 条 | 8 min |
| M5 | `cargo test -p h7cad-native-dxf` 全绿 | 1 min |
| M6 | `cargo check -p H7CAD`、`-p h7cad-native-facade` | 2 min |
| M7 | `ReadLints` + CHANGELOG "二十八" 条目 | 4 min |

## 验收

- `cargo test -p h7cad-native-dxf` **161 → 165**（+4 header_loft）
- `cargo test --bin H7CAD io::native_bridge` 25 / 25 不受影响
- `cargo check -p H7CAD` 零新 warning
- `cargo check -p h7cad-native-facade` 零新 warning
- `ReadLints` 改动的 4 个文件零 lint
- CHANGELOG 存在 "2026-04-22（二十八）" 条目
- HEADER 覆盖：97 → **103**（跨过 100 门槛！~34%）
- 本 plan §状态 写 "落地完成"

## 风险

| 风险 | 缓解 |
|---|---|
| 4 个 `loft_ang*/mag*` 都用 code 40，reader arm 间串位风险 | 二十五轮 SNAP/GRID 家族里 `snap_base / snap_unit / grid_unit` 共享 code 10/20 已证明 arm 作用域独立；测试值两两不等让串位立刻挂 |
| `loft_param` 位组合 9（bit1+bit8）在 AutoCAD 里可能不出现，测试 ground-truth 未必与真实绘图一致 | io 层不过滤 bit 组合；测试只关心 "i16 = 9 读写一致"，不模拟 AutoCAD 语义；UI / 3D engine 自己拒绝非法组合 |
| `loft_ang1 = π/6` 在 shortest round-trip `format_f64` 下是否仍精确 | 二十五轮升级 `{:.10}` → `f64::to_string()` 后任何 f64 都 bit-identical roundtrip；测试做 `assert_eq!` 就能检测到回归 |

## 执行顺序

M1 → M2 → M3 → M4 → M5 → M6 → M7 → commit（严格串行）

## 下一轮候选（二十九）

本轮用完 "快赢 6 变量" 后，plan §9 原先列的 3 候选只剩 DIMALT 家族
9 变量；那是唯一剩下的、在 header 里有工程意义的中等规模 Sprint。
二十九可选：

1. `$DIMALT / $DIMALTD / $DIMALTF / $DIMALTRND / $DIMALTTD / $DIMALTTZ
   / $DIMALTU / $DIMALTZ / $DIMAPOST` — 9 变量，DIM 替代单位家族
   （需在 `DocumentHeader` 里引入新的 `dim_alt_*` 子组）；是二十六
   轮以来最大规模一轮，但仍远低于 "DIM 完整 100+ 变量" 的长期目标
2. 或改走 **entity 侧扩展**（目前只有 41 种 EntityData 变体，AutoCAD
   完整 ~100 种）—— 这是更长远的工作，本 docs/plans 系列一直聚焦
   HEADER，是否在此转向需要和 owner 确认
3. 或补 `crates/h7cad-native-dwg` 的 AC1015 entity body 解码（修复
   `real_dwg_samples_baseline_m3b` 红灯的 26 / 40 LINE 差距）

推荐二十九仍选 **路径 1**（DIMALT 9 变量）以保持 HEADER Sprint 惯性，
直到 HEADER 覆盖超过 150 变量再评估是否转向路径 2 / 3。

## 状态

- [x] 计划定稿（本文件）
- [x] M1 DocumentHeader 6 字段 + Default
- [x] M2 reader 6 arm
- [x] M3 writer 6 对 pair
- [x] M4 新测试文件 4 条（π/6 + π/3 哨兵）
- [x] M5 `cargo test -p h7cad-native-dxf` 161 → 165 全绿
- [x] M6 `cargo check -p H7CAD` / `-p h7cad-native-facade` 零新 warning；`cargo test --bin H7CAD io::native_bridge` 25 / 25
- [x] M7 CHANGELOG "2026-04-22（二十八）" 落地 + ReadLints 零 lint
- [x] 里程碑：HEADER 覆盖越过 100 变量门槛（97 → 103）
