# 开发计划：DXF HEADER `$CHAMMODE` 扩充（二十一）

> 起稿：2026-04-22（第二十一轮）  
> 前置：HEADER 已覆盖 70 变量（~23%）。本轮补 **1 个 code 70 整数**：
> 交互 chamfer 输入模式开关 `$CHAMMODE`，联动上一轮刚落地的 `$CHAMFERA/B/C/D`。
> 覆盖推到 71。

## 动机

上一轮（二十）落地了 chamfer 四个距离（A/B/C/D）与 fillet 半径，但漏了
一个关键的**模式选择开关** —— `$CHAMMODE`：

- 0 = **Distance-Distance**（用 `$CHAMFERA` / `$CHAMFERB`）
- 1 = **Length-Angle**（用 `$CHAMFERC` 长度 + `$CHAMFERD` 角度）

没有 `$CHAMMODE`，reader 读 AutoCAD .dxf 再 roundtrip 写回后，交互 chamfer
默认模式会从用户本来设置的 Length-Angle 静默回退成 Distance-Distance。

本轮是上一轮的**功能闭环补丁**：一个 code 70 整数，插在 chamfer 家族内部，
保持 AutoCAD HEADER 官方输出顺序。

## 目标字段

| 字段 | 类型 | `$` 变量 | DXF code | Default | 语义 |
|---|---|---|---|---|---|
| `chammode` | `i16` | `$CHAMMODE` | 70 | `0` | chamfer 输入模式：0=两距离，1=长度+角度 |

选择 `i16` 而非 `bool`：与 `$CMLJUST` / `$ATTMODE` 等同族 code 70 整数保持
一致，并为 AutoCAD 可能未来扩展留出空间（目前官方只定义 0/1 两态）。

## 非目标

- **不**把 `$CHAMMODE` 绑死成 `bool`——保留 `i16` 透传（官方定义未来可扩）
- **不**联动 chamfer 命令运行时逻辑（io 层只管 header 值本身，不参与
  命令交互）
- **不**对 `$CHAMFERC/D` 做角度单位转换（上一轮已决定纯 f64 透传）

## 关键设计

### 1. Model（`crates/h7cad-native-model/src/lib.rs`）

在 `DocumentHeader` 的 "Interactive geometry command defaults" 分组中，
插入 `chammode` 字段，位置：紧接 `chamferd` 之后、`filletrad` 之前
（遵循 AutoCAD HEADER 官方输出顺序 —— chamfer 家族内部排序）：

```rust
/// `$CHAMFERD` (code 40): chamfer angle (distance-angle mode).
/// Stored as AutoCAD stores it (raw f64 passthrough). Default 0.0.
pub chamferd: f64,
/// `$CHAMMODE` (code 70): interactive chamfer input mode.
/// 0 = distance-distance (uses `$CHAMFERA` / `$CHAMFERB`).
/// 1 = length-angle (uses `$CHAMFERC` / `$CHAMFERD`).
/// Stored as `i16` (not `bool`) to leave room for future AutoCAD
/// tri-state extensions; current spec defines 0 / 1 only.
/// Default 0.
pub chammode: i16,
/// `$FILLETRAD` (code 40): default fillet radius. Default 0.0.
pub filletrad: f64,
```

`Default::default` 追加 `chammode: 0`。

### 2. Reader（`crates/h7cad-native-dxf/src/lib.rs`）

在 `"$CHAMFERD" => ...` 之后、`"$FILLETRAD" => ...` 之前追加：

```rust
"$CHAMFERD" => doc.header.chamferd = f(40),
"$CHAMMODE" => doc.header.chammode = i16v(70),
"$FILLETRAD" => doc.header.filletrad = f(40),
```

复用现有 `i16v(70)` helper。

### 3. Writer（`crates/h7cad-native-dxf/src/writer.rs`）

在 `$CHAMFERD` pair 之后、`$FILLETRAD` pair 之前插入：

```rust
w.pair_str(9, "$CHAMFERD");
w.pair_f64(40, doc.header.chamferd);

w.pair_str(9, "$CHAMMODE");
w.pair_i16(70, doc.header.chammode);

w.pair_str(9, "$FILLETRAD");
w.pair_f64(40, doc.header.filletrad);
```

### 4. 测试（`crates/h7cad-native-dxf/tests/header_chammode.rs`）

4 条，与上一轮风格一致：

- `header_reads_chammode`：构造 `$CHAMMODE=1` → 读后 `doc.header.chammode == 1`
- `header_writes_chammode`：构造 `chammode=1` → write → 字符串包含 `$CHAMMODE`
  且紧接其后的 `70` 对值为 `1`
- `header_roundtrip_preserves_chammode`：read(chammode=1) → write → read → 仍为 1
- `header_legacy_file_without_chammode_loads_with_zero`：缺省 → `chammode == 0`

## 实施步骤

| 步骤 | 工作内容 | 预估 |
|---|---|---|
| M1 | `DocumentHeader` + `Default::default` 扩 1 字段 | 3 min |
| M2 | reader 1 个 match arm | 1 min |
| M3 | writer 1 对 pair | 1 min |
| M4 | 新测试文件 4 条 | 8 min |
| M5 | `cargo test -p h7cad-native-dxf` + `cargo check -p H7CAD` + `ReadLints` + CHANGELOG | 6 min |

## 验收

- `cargo test -p h7cad-native-dxf` 133 → **137** (+4)
- `cargo test --bin H7CAD io::native_bridge` 25 / 25 不受影响
- `cargo check -p H7CAD` 零新 warning
- `ReadLints` 改动的 3 个源文件 + 新测试文件零 lint
- CHANGELOG "2026-04-22（二十一）" 条目存在
- HEADER 覆盖：70 → **71**

## 风险

| 风险 | 缓解 |
|---|---|
| `$CHAMMODE` 在某些 AutoCAD 版本存储为 `$CHAMMODE` vs `$CHAMMETHOD` 命名差异 | 只匹配 `$CHAMMODE`（官方主流名）；如后续发现 `$CHAMMETHOD` 别名再补 |
| `i16` vs `bool` 类型选择分歧 | 注释明确说明"预留扩展空间"，且与同族 code 70 整数保持一致 |
| writer 顺序插错导致 bridge snapshot drift | 插在 chamfer 家族内部（D 与 FILLETRAD 之间），符合 AutoCAD 官方顺序；ReadLints 会抓出任何字段偏移错误 |

## 执行顺序

M1 → M2 → M3 → M4 → M5（严格串行）
