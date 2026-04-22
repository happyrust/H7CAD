# 开发计划：DXF HEADER Chamfer / Fillet / 3D 默认值 7 变量扩充

> 起稿：2026-04-22（第二十轮）  
> 前置：HEADER 已覆盖 63 变量（~21%）。本轮再补 7 个 code 40 f64 常用默认量：  
> Chamfer 四距离 + Fillet 半径 + 当前 Elevation / Thickness。覆盖推到 70。

## 动机

AutoCAD 的 `CHAMFER` / `FILLET` 命令分别使用 `$CHAMFERA/B/C/D` 与
`$FILLETRAD` 作为交互式倒角 / 圆角的默认距离；`$ELEVATION` / `$THICKNESS`
则是 2.5D 建模场景下新建实体默认挂载的 Z 值与厚度。这 7 个变量全都是
code 40 f64，读写最简单、行为语义稳定，而 H7CAD 当前 reader 完全忽略、
writer 不输出，读 AutoCAD .dxf 写回后这 7 个设置丢失。

## 目标 7 变量

| 字段 | 类型 | `$` 变量 | DXF code | Default |
|---|---|---|---|---|
| `chamfera` | f64 | `$CHAMFERA` | 40 | 0.0 |
| `chamferb` | f64 | `$CHAMFERB` | 40 | 0.0 |
| `chamferc` | f64 | `$CHAMFERC` | 40 | 0.0 |
| `chamferd` | f64 | `$CHAMFERD` | 40 | 0.0 |
| `filletrad` | f64 | `$FILLETRAD` | 40 | 0.0 |
| `elevation` | f64 | `$ELEVATION` | 40 | 0.0 |
| `thickness` | f64 | `$THICKNESS` | 40 | 0.0 |

`$CHAMFERA/B` 为 Distance-Distance 模式的两距离，`$CHAMFERC/D` 为
Distance-Angle 模式的长度和角度（角度仍按 f64 弧度或度数由上层解读）。
`$ELEVATION` 仅影响新建实体的默认 Z 值（既有实体不迁移）。
`$THICKNESS` 影响 LINE/CIRCLE/ARC/TEXT 等实体新建时默认的 extrusion
厚度（entity-level `thickness` 字段已独立存在于 `EntityData` 结构里，
不与 header 字段冲突——reader/writer 仅透传 header 默认值本身）。

## 非目标

- **不**把 HEADER 的 `elevation` / `thickness` 自动注入到新建实体（那是
  CAD 命令层责任，不在 io 层）
- **不**对 `$CHAMFERD` 做弧度↔度数归一化（纯透传 f64，与 AutoCAD 原始
  存储保持一致）
- **不**扩 `$CHAMMODE`（code 70 tri-state）——下一轮联动 chamfer mode
  时再加，避免本轮 mix 数值型 / 模式型变量
- **不**动 entity-level 的 `thickness` / `elevation` 字段（已存在且独立）

## 关键设计

### 1. Model（`crates/h7cad-native-model/src/lib.rs`）

`DocumentHeader` 新增字段块（插在 `xedit` 之后、`handseed` 之前）：

```rust
// Interactive geometry command defaults.
/// `$CHAMFERA` (code 40): first chamfer distance. Default 0.0.
pub chamfera: f64,
/// `$CHAMFERB` (code 40): second chamfer distance. Default 0.0.
pub chamferb: f64,
/// `$CHAMFERC` (code 40): chamfer length (distance-angle mode).
/// Default 0.0.
pub chamferc: f64,
/// `$CHAMFERD` (code 40): chamfer angle (distance-angle mode).
/// Stored as AutoCAD stores it (raw f64 passthrough). Default 0.0.
pub chamferd: f64,
/// `$FILLETRAD` (code 40): default fillet radius. Default 0.0.
pub filletrad: f64,

// 2.5-D default attachment for freshly-created entities.
/// `$ELEVATION` (code 40): default Z value for new entities in the
/// current UCS. Default 0.0.
pub elevation: f64,
/// `$THICKNESS` (code 40): default extrusion thickness for new
/// entities (LINE / CIRCLE / ARC / TEXT). Default 0.0. Independent
/// of entity-level `thickness` — this is the per-drawing default,
/// each entity carries its own override.
pub thickness: f64,
```

`Default` 追加上述 7 个 `0.0`。

### 2. Reader（`crates/h7cad-native-dxf/src/lib.rs`）

在 `"$XEDIT" => ...` 行之后追加：

```rust
// Interactive geometry command defaults.
"$CHAMFERA" => doc.header.chamfera = f(40),
"$CHAMFERB" => doc.header.chamferb = f(40),
"$CHAMFERC" => doc.header.chamferc = f(40),
"$CHAMFERD" => doc.header.chamferd = f(40),
"$FILLETRAD" => doc.header.filletrad = f(40),

// 2.5-D default attachment.
"$ELEVATION" => doc.header.elevation = f(40),
"$THICKNESS" => doc.header.thickness = f(40),
```

全部走现有 `f(40)` helper。

### 3. Writer（`crates/h7cad-native-dxf/src/writer.rs`）

在 `$XEDIT` pair 之后、`$PDMODE` pair 之前插入：

```rust
// ── Interactive geometry command defaults ─────────────────────────────
w.pair_str(9, "$CHAMFERA");
w.pair_f64(40, doc.header.chamfera);

w.pair_str(9, "$CHAMFERB");
w.pair_f64(40, doc.header.chamferb);

w.pair_str(9, "$CHAMFERC");
w.pair_f64(40, doc.header.chamferc);

w.pair_str(9, "$CHAMFERD");
w.pair_f64(40, doc.header.chamferd);

w.pair_str(9, "$FILLETRAD");
w.pair_f64(40, doc.header.filletrad);

// ── 2.5-D default attachment ──────────────────────────────────────────
w.pair_str(9, "$ELEVATION");
w.pair_f64(40, doc.header.elevation);

w.pair_str(9, "$THICKNESS");
w.pair_f64(40, doc.header.thickness);
```

### 4. 测试（`crates/h7cad-native-dxf/tests/header_geom_defaults.rs`）

4 条覆盖 read / write / roundtrip / legacy：

- `header_reads_all_7_geom_default_vars`：非默认值全部精确读入
- `header_writes_all_7_geom_default_vars`：构造 → write → 7 个 `$VAR` 字符串都在
- `header_roundtrip_preserves_all_7_geom_default_vars`：read → write → read 全字段保持
- `header_legacy_file_without_geom_defaults_loads_with_zeros`：缺省 → 全部 0.0

## 实施步骤

| 步骤 | 工作内容 | 预估 |
|---|---|---|
| M1 | `DocumentHeader` + `Default::default` 扩 7 字段 | 5 min |
| M2 | reader 7 个 match arm | 3 min |
| M3 | writer 7 对 pair | 3 min |
| M4 | 新测试文件 4 条 | 10 min |
| M5 | `cargo test -p h7cad-native-dxf` + `cargo check -p H7CAD` + `ReadLints` + CHANGELOG | 10 min |

## 验收

- `cargo test -p h7cad-native-dxf` 129 → **133** (+4)
- `cargo test --bin H7CAD io::native_bridge` 25 / 25 不受影响
- `cargo check -p H7CAD` 零新 warning
- `ReadLints` 改动的 3 个源文件零 lint
- CHANGELOG "2026-04-22（二十）" 条目存在

## 风险

| 风险 | 缓解 |
|---|---|
| `$CHAMFERD` 角度单位（rad vs deg）在不同 AutoCAD 版本差异 | 纯 f64 透传，不转换；上层若需展示按 `$AUNITS` 解读 |
| `$ELEVATION` 与 `$THICKNESS` 与 entity-level 同名字段混淆 | doc comment 明确标注是 "per-drawing default"；字段就挂在 `DocumentHeader` 上，编译器作用域天然隔离 |
| 现有 `native_bridge` 可能依赖字段 stable layout | 新字段追加在 `xedit` 之后 / `handseed` 之前，不改变前序字段偏移 |

## 执行顺序

M1 → M2 → M3 → M4 → M5（严格串行；任一步红则就地修，不跳步）
