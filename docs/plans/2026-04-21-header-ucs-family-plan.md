# 开发计划：DXF HEADER UCS 家族 5 变量扩充

> 起稿：2026-04-21（第八轮）  
> 前置：
> - `docs/plans/2026-04-21-header-drawing-vars-plan.md`（15 绘图环境变量）
> - `docs/plans/2026-04-21-header-timestamps-plan.md`（4 时间戳变量）
> 本轮继续扩 HEADER 覆盖面，补齐 UCS（User Coordinate System）家族 5 变量，覆盖任何 AutoCAD 图纸的"当前 UCS 定义"。

## 动机

当前 DXF HEADER 覆盖 34 个变量（15 原有 + 15 绘图 + 4 时间戳），但缺失 UCS 家族：

| `$` 变量 | DXF code | 含义 |
|---|---|---|
| `$UCSBASE` | 2 (string) | Name of UCS that defines origin/orientation of orthographic UCS settings |
| `$UCSNAME` | 2 (string) | Name of current UCS |
| `$UCSORG` | 10/20/30 | UCS origin point (WCS) |
| `$UCSXDIR` | 10/20/30 | UCS X-axis direction (WCS) |
| `$UCSYDIR` | 10/20/30 | UCS Y-axis direction (WCS) |

真实 AutoCAD DXF 普遍携带这 5 个变量（即使 UCS 等同 WCS）。读 AutoCAD .dxf 写回后 UCS 设置全丢失，用户重新打开时当前 UCS 被复位到 WCS。

本轮给 HEADER 扩 5 个字段，让 UCS 设置完整 round-trip。**不动** TABLES.UCS 表（这是独立的 UCS-by-name 字典，未来单独处理）。

## 目标

1. `DocumentHeader` 扩 5 字段：
   - `ucsbase: String`（`$UCSBASE`，code 2，default `""`）
   - `ucsname: String`（`$UCSNAME`，code 2，default `""`）
   - `ucsorg: [f64; 3]`（`$UCSORG`，code 10/20/30，default `[0, 0, 0]`）
   - `ucsxdir: [f64; 3]`（`$UCSXDIR`，code 10/20/30，default `[1, 0, 0]`）
   - `ucsydir: [f64; 3]`（`$UCSYDIR`，code 10/20/30，default `[0, 1, 0]`）
2. Reader `read_header_section` match 加 5 arm
3. Writer `write_header` 加 5 对 pair 块
4. 测试：read / write / roundtrip / legacy 回落默认，至少 4 条

## 非目标

- 不动 TABLES.UCS 表（当前只存 name→handle，需要补 origin / xdir / ydir 属性；独立 scope）
- 不处理 `$UCSORGTOP / $UCSORGLEFT / …` 6 个 orthographic UCS origins（Aliased to `$UCSBASE` 的子设置）
- 不验证 xdir / ydir 正交（这是 UI / 命令层的责任，不是 reader / writer 的）
- 不做 WCS→UCS 坐标变换帮手（已有 `$UCSORG` / `$UCSXDIR` / `$UCSYDIR` 三元组，上层可自行构建矩阵）

## 关键设计

### 1. Model

`DocumentHeader`（`crates/h7cad-native-model/src/lib.rs`）在 timestamp 4 字段之后、`handseed` 之前插入：

```rust
// UCS (User Coordinate System) metadata.
/// `$UCSBASE` (code 2): name of UCS defining origin/orientation of
/// orthographic UCS settings. Default empty.
pub ucsbase: String,
/// `$UCSNAME` (code 2): name of the current UCS. Default empty
/// (i.e. current UCS equals WCS).
pub ucsname: String,
/// `$UCSORG` (codes 10/20/30): UCS origin point in WCS coordinates.
pub ucsorg: [f64; 3],
/// `$UCSXDIR` (codes 10/20/30): UCS X-axis direction in WCS.
pub ucsxdir: [f64; 3],
/// `$UCSYDIR` (codes 10/20/30): UCS Y-axis direction in WCS.
pub ucsydir: [f64; 3],
```

`Default` 填 `""` / `""` / `[0, 0, 0]` / `[1, 0, 0]` / `[0, 1, 0]`（WCS 对齐）。

### 2. Reader

```rust
"$UCSBASE" => doc.header.ucsbase = sv(2).to_string(),
"$UCSNAME" => doc.header.ucsname = sv(2).to_string(),
"$UCSORG" => doc.header.ucsorg = [f(10), f(20), f(30)],
"$UCSXDIR" => doc.header.ucsxdir = [f(10), f(20), f(30)],
"$UCSYDIR" => doc.header.ucsydir = [f(10), f(20), f(30)],
```

### 3. Writer

在 `$PSLTSCALE` 之后、timestamps 之前输出（AutoCAD 惯例顺序）：

```rust
w.pair_str(9, "$UCSBASE");
w.pair_str(2, &doc.header.ucsbase);

w.pair_str(9, "$UCSNAME");
w.pair_str(2, &doc.header.ucsname);

w.pair_str(9, "$UCSORG");
w.point3d(10, doc.header.ucsorg);

w.pair_str(9, "$UCSXDIR");
w.point3d(10, doc.header.ucsxdir);

w.pair_str(9, "$UCSYDIR");
w.point3d(10, doc.header.ucsydir);
```

### 4. 测试

新建 `crates/h7cad-native-dxf/tests/header_ucs_family.rs`：

- `header_reads_all_5_ucs_vars`：手写含 5 `$UCS*` 的 HEADER → 精确读取（`ucsorg=[10,20,30]`, `ucsxdir=[0, -1, 0]`, `ucsydir=[1, 0, 0]`, `ucsname="RotatedUCS"`, `ucsbase="TOP"`）
- `header_writes_all_5_ucs_vars`：构造 doc 填 5 字段 → write → 扫 text 找 `$UCS*` + 对应 code
- `header_roundtrip_preserves_all_5_ucs_vars`：read → write → read，容忍 1e-9
- `header_legacy_file_without_ucs_fields_loads_with_defaults`：legacy HEADER 无 `$UCS*` → 5 字段走默认（空字符串 + WCS）

## 实施步骤

### M1 — model（5 min）

`DocumentHeader` 加 5 字段 + Default。

### M2 — reader（10 min）

`read_header_section` 加 5 arm。

### M3 — writer（10 min）

`write_header` 加 5 块 pair（`$UCS*` 顺序紧跟 `$PSLTSCALE`，timestamp 保持在后）。

### M4 — 测试（20 min）

`tests/header_ucs_family.rs`，4 条。

### M5 — validator + CHANGELOG（10 min）

- `cargo test -p h7cad-native-dxf` 105 → **109** (+4)
- `cargo test --bin H7CAD io::native_bridge` 无回归
- CHANGELOG "2026-04-21（八）"

## 风险与缓解

| 风险 | 缓解 |
|---|---|
| `$UCSORG[xy]DIR` 正交性失效 | reader / writer 不校验；只透传，UI 责任 |
| UCSBASE / UCSNAME 引用的 name 对应的 TABLES.UCS 条目缺失 | 仅 HEADER 变量存 name 字符串，TABLES.UCS 解引用留给消费层 |
| default xdir / ydir 与 WCS Z-Up 约定 | xdir = `[1, 0, 0]`, ydir = `[0, 1, 0]` → Z-axis = `[0, 0, 1]`（右手系），匹配 AutoCAD 默认 |

## 验收

- `cargo test -p h7cad-native-dxf` ≥ **109**
- `cargo test --bin H7CAD io::native_bridge` 20 / 20
- `cargo check -p H7CAD` 零新 warning
- CHANGELOG 条目

## 执行顺序

M1 → M2 → M3 → M4 → M5（严格串行，每步过 compile）
