# DIM 箭头 / 符号家族 6 变量扩充

> 起稿：2026-04-24
> 前置：DIMALT 9 变量完成（112 vars），本轮扩充到 118

---

## 1. 新增字段

| 字段 | 类型 | `$` 变量 | DXF code | Default | 语义 |
|---|---|---|---|---|---|
| `dim_blk` | `String` | `$DIMBLK` | 1 | `""` | 全局箭头块名（空=标准实心箭头） |
| `dim_blk1` | `String` | `$DIMBLK1` | 1 | `""` | 第一箭头块名（覆盖 dim_blk） |
| `dim_blk2` | `String` | `$DIMBLK2` | 1 | `""` | 第二箭头块名（覆盖 dim_blk） |
| `dim_ldrblk` | `String` | `$DIMLDRBLK` | 1 | `""` | 引线箭头块名 |
| `dim_arcsym` | `i16` | `$DIMARCSYM` | 70 | `0` | 弧长符号显示模式（0 文字前 / 1 文字上方 / 2 不显示） |
| `dim_jogang` | `f64` | `$DIMJOGANG` | 40 | `0.7854` | 折弯标注折角（弧度，π/4 默认） |

## 2. 改动

| 文件 | 改动 |
|------|------|
| `h7cad-native-model/src/lib.rs` | `DocumentHeader` 新增 6 字段 |
| `h7cad-native-dxf/src/lib.rs` | reader 新增 6 arm |
| `h7cad-native-dxf/src/writer.rs` | writer 新增 6 pair |
| `h7cad-native-dxf/tests/header_dim_arrow.rs` | 4 条测试 |

## 3. 验收

- `cargo test -p h7cad-native-dxf` +4 全绿
- `cargo check -p H7CAD` 零新 warning
- HEADER 覆盖：112 → 118
