# Phase 2a：TABLE 增强 + Wipeout 验证 + LAYOUT 修复记录

> 起稿：2026-04-24
> 前置：Phase 1（write_dxf_strict）已完成；LAYOUT code 330→340 bug 已修复

---

## 1. TABLE 实体增强

### 1.1 背景

ACAD_TABLE 实体当前缺少 `row_heights` (code 141) 和 `column_widths`
(code 142) 字段。这些在 DXF 规范中是 ACAD_TABLE 的标准属性，
缺少会导致从 AutoCAD 文件读取的表格在 roundtrip 后丢失行高/列宽。

### 1.2 改动

| 文件 | 改动 |
|------|------|
| `h7cad-native-model/src/lib.rs` | `EntityData::Table` 新增 `row_heights: Vec<f64>`, `column_widths: Vec<f64>` |
| `h7cad-native-dxf/src/entity_parsers.rs` | `parse_acad_table` 读取 code 141/142 |
| `h7cad-native-dxf/src/writer.rs` | `write_entity_data` TABLE arm 写出 141/142 |
| `h7cad-native-dxf/tests/table_roundtrip.rs` | 新增 roundtrip 测试 |

### 1.3 兼容性

`EntityData::Table` 增加两个字段是 breaking（match exhaustiveness），
但该类型仅在内部使用，不影响外部 API。所有现有的 `Table { .. }` match
使用通配符 `..` 不受影响。

---

## 2. Wipeout roundtrip 验证

新增 `tests/wipeout_roundtrip.rs`，覆盖：
- 基本 clip_vertices roundtrip
- elevation 保真
- 空 clip_vertices 边界

---

## 3. 验收

- `cargo test -p h7cad-native-dxf` 全绿（+新测试数）
- `cargo check -p H7CAD` 零新 warning
- `cargo check -p h7cad-native-facade` 零新 warning

---

## 4. 状态

- [x] TABLE row_heights/column_widths 实现 — 2026-04-24 完成
- [x] Wipeout roundtrip 测试 — 2026-04-24 完成
- [x] CHANGELOG 更新 — 2026-04-24 完成
