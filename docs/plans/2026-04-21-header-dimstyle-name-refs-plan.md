# 开发计划：DXF HEADER 标注样式名引用 2 变量扩充

> 起稿：2026-04-21（第十五轮）  
> 前置：HEADER 已覆盖 61 变量，其中 DIM Tier 1 8 数值变量已扩。本轮把 "当前标注样式名" 和 "标注文字样式名" 两个**字符串引用**变量加进来，闭合 DIM 区块的 name-pointer 部分。

## 动机

第十轮的 DIM Tier 1 plan 里包含尺寸数值默认值（dimtxt / dimasz 等 8 个），但**未包含名称引用**：

- `$DIMSTYLE` (code 2, string) — Current dimension style name
- `$DIMTXSTY` (code 7, string) — Current dimension text style name

这两个变量 AutoCAD DXF 普遍输出。当前 reader 忽略 → 重新打开时 "current dimension style" 复位为空字符串，UI 显示 "Standard" fallback。

## 目标

1. `DocumentHeader` 扩 2 字段：
   - `dimstyle: String`（`$DIMSTYLE`，code 2，default `"Standard"`）
   - `dimtxsty: String`（`$DIMTXSTY`，code 7，default `"Standard"`）
2. Reader 2 arm
3. Writer 2 pair 块（紧跟 DIM Tier 1 之后）
4. 测试：read / write / roundtrip / legacy 默认（4 条）

## 非目标

- 不验证 `$DIMSTYLE` 引用的 DimStyleProperties 在 TABLES.DIMSTYLE 中存在
- 不接入到 DIMENSION 实体的 default style（独立逻辑层）
- 不扩 `$DIMBLK` (default arrow block name) — 独立 scope

## 关键设计

### Model

```rust
// Default dimension / text style references (Tier 1).
/// `$DIMSTYLE` (code 2): current dimension style name.
/// Default `"Standard"`.
pub dimstyle: String,
/// `$DIMTXSTY` (code 7): current dimension text style name.
/// Default `"Standard"`.
pub dimtxsty: String,
```

### Reader

```rust
"$DIMSTYLE" => doc.header.dimstyle = sv(2).to_string(),
"$DIMTXSTY" => doc.header.dimtxsty = sv(7).to_string(),
```

### Writer

紧跟 `$DIMADEC` 之后（DIM Tier 1 末尾）：

```rust
w.pair_str(9, "$DIMSTYLE");
w.pair_str(2, &doc.header.dimstyle);
w.pair_str(9, "$DIMTXSTY");
w.pair_str(7, &doc.header.dimtxsty);
```

### 测试

`tests/header_dimstyle_name_refs.rs`：

- `header_reads_both_name_refs`：自定义 `dimstyle="Architectural"`, `dimtxsty="ArialBold"` → 精确读取
- `header_writes_both_name_refs`：构造 → write → 字符串都在
- `header_roundtrip_preserves_name_refs`
- `header_legacy_file_uses_standard_defaults`

## 实施步骤

M1 (5 min) → M2 (5 min) → M3 (5 min) → M4 (15 min) → M5 validator + CHANGELOG (10 min)

## 验收

- `cargo test -p h7cad-native-dxf` 125 → **129** (+4)
- `cargo test --bin H7CAD io::native_bridge` 20 / 20
- `cargo check -p H7CAD` 零新 warning
- CHANGELOG 条目
