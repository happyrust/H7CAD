# H7CAD F5.M4.D — LINE Entity Body Writer 子计划

> **起稿**：2026-05-09
> **父计划**：[`2026-05-09-dwg-m4-object-stream-writer-plan.md`](2026-05-09-dwg-m4-object-stream-writer-plan.md) §2.4
> **里程碑**：F5.M4.D —— LINE entity body writer
> **目标**：让 `read_line_geometry(reader)` 完整还原 writer 写出的 LINE 几何字段
> **不在范围**：其他 21 类 entity body writer（CIRCLE / ARC / ...，归 F5.M5）；entity common header（已落地于 M4.C）；object slice composition（已落地于 M4.B）；`write_dwg` 集成（M4.E）
> **预估**：1–2h

---

## 0. TL;DR

LINE 是 R2000 文件中数量最多的几何 entity（`sample_AC1015.dwg` 82 个 LINE 对比 9 个 CIRCLE / 3 个 ARC），也是 reader 端最早接通的 entity body decoder。因此 native writer 的第一个 entity body writer 选择 LINE，能让"writer→reader 全链路"覆盖最大百分比的实测样本。

本子计划交付：
- `crates/h7cad-native-dwg/src/writer/entity_line.rs::write_line_geometry(LineGeometry, &mut BitWriter)`
- 4–5 个单测覆盖 read/write 对偶（2D / 3D / DD-default / 非平凡 thickness 与 extrusion）

不接 `write_dwg`；那是 M4.E 的工作。

---

## 1. Reader 端字段时序（来源：`crates/h7cad-native-dwg/src/entity_line.rs`）

### 1.1 `read_line_geometry` 顺序

| 序 | reader 调用 | M4.D writer 调用 |
|---|---|---|
| 1 | `read_bit()` → `z_are_zero` (1 bit) | `write_bit(if z_are_zero { 1 } else { 0 })` |
| 2 | `read_raw_f64_le()` → `sx` | `write_raw_f64_le(sx)` |
| 3 | `read_bit_double_with_default(sx)` → `ex` | `write_bit_double_with_default(ex, sx)` |
| 4 | `read_raw_f64_le()` → `sy` | `write_raw_f64_le(sy)` |
| 5 | `read_bit_double_with_default(sy)` → `ey` | `write_bit_double_with_default(ey, sy)` |
| 6 | (if !z_are_zero) `read_raw_f64_le()` → `sz` | (if !z_are_zero) `write_raw_f64_le(sz)` |
| 7 | (if !z_are_zero) `read_bit_double_with_default(sz)` → `ez` | (if !z_are_zero) `write_bit_double_with_default(ez, sz)` |
| 8 | `read_bit_thickness_r2000_plus()` → `thickness` | `write_bit_thickness_r2000_plus(thickness)` |
| 9 | `read_bit_extrusion_r2000_plus()` → `extrusion` | `write_bit_extrusion_r2000_plus(extrusion)` |

### 1.2 z_are_zero 自动检测

Reader 没有显式信号告诉 caller 「应该把哪三个值视为 0」；它依赖 `z_are_zero` 标志位。Writer 的策略：

- `start[2] == 0.0 && end[2] == 0.0` → 写 `z_are_zero = 1`，省去 sz / ez 的 80+ bits（一个 RD = 64 bits + 一个 DD prefix ≥ 2 bits）
- 否则 → 写 `z_are_zero = 0`，并写 sz + ez

> **决策**：Writer 自动选最紧凑表示。Caller 不需要显式控制 `z_are_zero` —— 字段 0 自然导致紧凑路径。

> **风险**：`-0.0` 与 `+0.0`：Rust `f64` 的 `0.0 == -0.0` 返回 true，所以 `-0.0` 也会触发 z_are_zero 紧凑路径。reader 端读出的 `z` 永远是 `+0.0`。这与 IEEE 754 稍有偏离，但符合 AutoCAD 的语义（`-0.0` 在 LINE 几何意义上等同 `+0.0`）。

### 1.3 thickness 与 extrusion 的紧凑编码

| 字段 | 紧凑路径 | 完整路径 |
|---|---|---|
| `thickness` | `0.0` → 1 bit (`1`) | 非 0 → 1 bit (`0`) + BD double |
| `extrusion` | `[0.0, 0.0, 1.0]` (世界 Z) → 1 bit (`1`) | 否则 → 1 bit (`0`) + 3 BD doubles |

`BitWriter::write_bit_thickness_r2000_plus` 与 `write_bit_extrusion_r2000_plus` 已经实现这两条路径（见 F5.M1 的 30 个 BitWriter 单测覆盖）。直接调用即可。

---

## 2. 接口设计

```rust
// crates/h7cad-native-dwg/src/writer/entity_line.rs

use crate::entity_line::LineGeometry;

pub fn write_line_geometry(
    geom: LineGeometry,
    writer: &mut BitWriter,
) -> Result<(), DwgWriteError>;
```

输入类型直接复用 reader 端定义的 `LineGeometry`（`crates/h7cad-native-dwg/src/entity_line.rs::LineGeometry`），保证 reader/writer 视角一致。无需新增类型。

---

## 3. 任务拆解

| ID | 描述 | 文件 |
|---|---|---|
| T1 | 创建 `crates/h7cad-native-dwg/src/writer/entity_line.rs::write_line_geometry`。 | `writer/entity_line.rs` |
| T2 | `writer/mod.rs` 与 `lib.rs` re-export。 | 二处 |
| T3 | 4–5 个单测 roundtrip。 | `writer/entity_line.rs` 内部 `#[cfg(test)] mod tests` |
| T4 | `cargo test -p h7cad-native-dwg --all-targets` 全绿。 | — |
| T5 | `RUSTFLAGS=-Dwarnings cargo check -p h7cad-native-dwg --all-targets` 干净。 | — |
| T6 | 新文件 `rustfmt --edition 2021` + `ReadLints`。 | — |
| T7 | 更新 CHANGELOG / progress / findings / 父子计划 §6 执行记录。 | 四处 |

---

## 4. 测试矩阵

| 测试 | 路径 | 验证 |
|---|---|---|
| `write_line_geometry_round_trips_2d_synthesis` | z_are_zero = 1 + 紧凑 thickness/extrusion | 与 reader 端 `line_geometry_round_trips_2d_synthesis` 镜像；start=(1,2,0)，end=(4,5,0) |
| `write_line_geometry_round_trips_3d_with_z` | z_are_zero = 0 + 显式 sz/ez | reader 端 `line_geometry_reads_z_when_not_zero_flag` 的对偶 |
| `write_line_geometry_uses_dd_default_when_end_equals_start` | z_are_zero = 1 + ex == sx → DD prefix `00` | reader 端 `line_geometry_uses_start_coordinate_default_for_bit_double_prefix_zero` 的对偶 |
| `write_line_geometry_round_trips_nontrivial_thickness_and_extrusion` | thickness != 0 + extrusion ≠ [0,0,1] | reader 端 `line_geometry_decodes_nontrivial_thickness_and_extrusion` 的对偶 |
| `write_line_geometry_negative_zero_z_collapses_to_z_are_zero_path` | start[2] = -0.0, end[2] = +0.0 → 紧凑路径 | 锁定 §1.2 的 `-0.0` 处理决策 |

---

## 5. 验收门

- 5 个新单测全绿；既存 native-dwg 测试零回归 → **lib 235→240（+5）= 240 + 95 = 335 全绿**（数量级，实际等 M4.C 落地后再算）。
- `cargo check --workspace --all-targets`：通过；零新增 warning。
- `RUSTFLAGS=-Dwarnings cargo check -p h7cad-native-dwg --all-targets`：干净。
- 新文件 `rustfmt --edition 2021 --check` 通过；`ReadLints` 零错误。
- facade `dwg_runtime_save_is_unavailable` 测试**保持锁定**（M4.D 仍然不接 entity 入 `write_dwg`）。

---

## 6. 风险与退路

| 风险 | 触发条件 | 退路 |
|---|---|---|
| `read_bit_double_with_default` 的 DD prefix 选择算法不一致 | 测试 3 红：reader 期望 `0b00`，writer 输出 `0b11` | 检查 `BitWriter::write_bit_double_with_default` 的实现是否优先选最紧凑 prefix；如必要在 writer 加 `prefer_default_prefix` flag |
| z_are_zero 边界（`-0.0` vs `+0.0`） | 测试 5 红 | 改为字面比较 bit 模式（`f64::to_bits(z) == 0`），允许 `-0.0` 走紧凑 |
| `BitWriter::write_bit_thickness_r2000_plus` / `write_bit_extrusion_r2000_plus` 不存在 | T1 编译失败 | F5.M1 已实现这两个方法（CHANGELOG 列出过），如缺失补齐；不应触发 |

---

## 7. 时间表

| 段 | 工作 | 估时 |
|---|---|---|
| T1 | 编码 `write_line_geometry` | 15 min |
| T2 | re-export | 5 min |
| T3 | 5 个单测 | 30 min |
| T4–T6 | 验证 + fmt + lint | 10 min |
| T7 | 文档 | 20 min |
| 缓冲 | DD prefix / -0.0 调试 | 30 min |
| **合计** | | **1.5–2 h** |

---

## 8. 立即可执行的第一批命令

```powershell
# 起步
cargo test -p h7cad-native-dwg --lib writer::entity_line -- --nocapture

# 收尾
cargo test -p h7cad-native-dwg --all-targets
$env:RUSTFLAGS='-Dwarnings'; cargo check -p h7cad-native-dwg --all-targets; Remove-Item Env:RUSTFLAGS
rustfmt --edition 2021 --check crates\h7cad-native-dwg\src\writer\entity_line.rs
```

---

## 9. 与父计划的关系

| 父计划条目 | 关系 |
|---|---|
| `2026-05-09-dwg-m4-object-stream-writer-plan.md` §2.4 | 本子计划是其展开；落地后父计划 §2.4 标记完成 |
| §2.5 M4.E `write_dwg` 集成 | 直接消费本计划：M4.E 调 `write_ac1015_entity_common_minimal` (M4.C) → `write_line_geometry` (本计划) → `compose_ac1015_object_slice` (M4.B) |

---

## 10. 执行记录（落地后填充）

| 日期 | 任务 | 简述 | 验证 |
|---|---|---|---|
| 2026-05-09 | T1+T2 | `crates/h7cad-native-dwg/src/writer/entity_line.rs::write_line_geometry` 创建；`writer/mod.rs` 与 `lib.rs` re-export | 编译通过 |
| 2026-05-09 | T3 | 5 个单测：2D 紧凑 / 3D 显式 / DD-default `0b00` / 非平凡 thickness+extrusion / `-0.0` z 边界 | 5/5 首次运行就全绿；DD-default 单测精确预测 135 bits |
| 2026-05-09 | T4–T6 | `cargo test -p h7cad-native-dwg --all-targets`：lib 240 + 95 = 335 全绿；`RUSTFLAGS=-Dwarnings cargo check` 干净；`rustfmt + ReadLints` 通过 | M4.D 收口 |
| 2026-05-09 | T7 | CHANGELOG / progress / findings / 父子计划 §6 同步 | 文档齐全 |

---

*起草者：H7CAD agent；2026-05-09。*
