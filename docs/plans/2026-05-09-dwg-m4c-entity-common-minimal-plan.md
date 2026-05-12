# H7CAD F5.M4.C — Common Entity Header Minimal Writer 子计划

> **起稿**：2026-05-09
> **父计划**：[`2026-05-09-dwg-m4-object-stream-writer-plan.md`](2026-05-09-dwg-m4-object-stream-writer-plan.md) §2.3
> **里程碑**：F5.M4.C —— common entity header writer，最简配置
> **目标**：让 `parse_ac1015_entity_common(main_reader, handle_reader, object_handle)` 能完整还原 writer 写出的 main + handle 流字段，且 `Ac1015EntityCommonData` 字段值与 caller 输入精确等价
> **不在范围**：完整字段配置（带 EED / graphic / nolinks=false 链接 / 显式 linetype / 显式 plotstyle）— 留作 M4.C 的后续 PR 或 M5 演进
> **预估**：3–5h（一次性完成；如反推困难，§5 拆 M4.C.1 + M4.C.2 多 PR）

---

## 0. TL;DR

把 native DWG writer 推进到「能把一个最简 entity 的 common header 字段（owner/layer/linetype/color/lineweight/...）写出 main 流 + handle 流双流，并被 `parse_ac1015_entity_common` 还原」。

`parse_ac1015_entity_common` 实际读取 13 个 main 字段 + 至多 7 个 handle 字段，其中绝大多数取最简默认值后可以省略 handle 流条目。通过精心选择**所有 flag 字段的默认值**，最简配置只需写：

- main 流 11 个字段（EED 终止 + 11 个固定字段）
- handle 流 2 个字段（xdictionary NULL + layer 显式）

---

## 1. Reader 端字段时序（来源：`crates/h7cad-native-dwg/src/entity_common.rs`）

### 1.1 main 流（`main_reader`）

| 序 | reader 调用 | M4.C minimal 值 | writer 调用 |
|---|---|---|---|
| 1 | EED loop: `read_bit_short()` 直到 `<= 0`；> 0 时 read_handle + read 字节 size | 写 `bit_short(0)` 终结 | `BitWriter::write_bit_short(0)` |
| 2 | `read_bit()` → `has_graphic` | `0` | `write_bit(0)` |
| 3 | (if has_graphic) `read_raw_u32_le()` + `graphic_size` 字节 | skip（has_graphic = 0） | — |
| 4 | `read_bits(2)` → `entity_mode` | `1` (= ByBlockOwnership 概念，避免 owner 在 handle 流) | `write_bits(0b01, 2)` |
| 5 | `read_bit_long()` → `reactor_count` | `0` | `write_bit_long(0)` |
| 6 | `read_bit()` → `nolinks` | `1` (= true，跳过 prev/next 链接) | `write_bit(1)` |
| 7 | `read_bit_short()` → `color_index` | `0` (ByLayer / ByBlock 视上下文) | `write_bit_short(0)` |
| 8 | `read_bit_double()` → `linetype_scale` | `1.0` | `write_bit_double(1.0)` |
| 9 | `read_bits(2)` → `linetype_flags` | `0` (= ByLayer，linetype handle 不在 handle 流) | `write_bits(0b00, 2)` |
| 10 | `read_bits(2)` → `plotstyle_flags` | `0` (= ByLayer) | `write_bits(0b00, 2)` |
| 11 | `read_bit_short()` → `invisible` | `0` | `write_bit_short(0)` |
| 12 | `read_raw_u8()` → `lineweight_index` | `31` (ByDefault sentinel) | `write_raw_u8(31)` |

### 1.2 handle 流（`handle_reader`）

reader 的实际读取顺序（依赖 main 流字段决定是否读取）：

| 序 | reader 调用 | 触发条件 | M4.C minimal 行为 |
|---|---|---|---|
| 1 | `read_resolved_handle` (owner) | entity_mode == 0 | **不写**（minimal 选 entity_mode = 1） |
| 2 | `read_resolved_handle` × reactor_count | reactor_count > 0 | **不写**（minimal 选 reactor_count = 0） |
| 3 | `consume_optional_handle` (xdictionary) | 总是读 | **必写**：写 NULL handle 引用（code 5 + value 0） |
| 4 | `read_resolved_handle` × 2 (prev + next) | nolinks == false | **不写**（minimal 选 nolinks = 1） |
| 5 | `read_resolved_handle` (layer_handle) | 总是读 | **必写**：layer_handle 显式 |
| 6 | `read_resolved_handle` (linetype_handle) | linetype_flags == 0b11 | **不写**（minimal 选 linetype_flags = 0） |
| 7 | `read_resolved_handle` (plotstyle_handle) | plotstyle_flags == 0b11 | **不写**（minimal 选 plotstyle_flags = 0） |

**最终最简 handle 流 = NULL xdictionary + layer_handle。两个写入。**

### 1.3 Handle reference 编码

`read_handle_relative` 解码：

| code | 解释 | minimal writer 选择 |
|---|---|---|
| 0x0..=0x5 | `value` 即绝对 handle | **使用 code 5 + value** |
| 0x6 | reference + 1 | 不用 |
| 0x8 | reference - 1 | 不用 |
| 0xA | reference + value | 不用 |
| 0xC | reference - value | 不用 |
| 其他 | 解为 0 | 不用 |

`BitWriter::write_handle(code, value)` 已存在，直接用：
- NULL handle reference：`write_handle(0x5, 0)` → 解码 = 0 = `Handle::NULL`
- 显式绝对 handle：`write_handle(0x5, h.value())` → 解码 = h.value()

---

## 2. 接口设计

### 2.1 公共接口

```rust
// crates/h7cad-native-dwg/src/writer/entity_common.rs

/// Minimal-config common entity header inputs. The fields exposed
/// here are the ones a caller plausibly wants to vary at the F5.M4.C
/// milestone; everything else (EED, graphic, reactors, prev/next
/// links, explicit linetype/plotstyle) is hard-coded to its
/// "absent" value at this milestone and added incrementally in
/// M5/M6 as entity types demand them.
#[derive(Debug, Clone, Copy)]
pub struct EntityCommonMinimal {
    /// Owner block record handle. M4.C uses entity_mode = 1 to
    /// signal "owner not encoded inline"; the reader returns
    /// `Handle::NULL` for `owner_handle` in that case. Round-trip
    /// tests therefore verify the *encoded* layer + linetype
    /// handles, not the owner.
    pub owner_block_handle: Handle,
    pub layer_handle: Handle,
    pub color_index: i16,
    pub linetype_scale: f64,
    pub lineweight: i16,
    pub invisible: bool,
}

pub fn write_ac1015_entity_common_minimal(
    minimal: EntityCommonMinimal,
    main_writer: &mut BitWriter,
    handle_writer: &mut BitWriter,
) -> Result<(), DwgWriteError>;
```

### 2.2 `lineweight` 输入归一化

`lineweight` 字段在 reader 端是 `i16`（解码后），但 wire format 是 `u8` index 经 `dwg_lineweight_from_index` 查表得 i16。Writer 需要反查表（或直接写 ByDefault index 31，对应 i16 = -3）。

最简配置：硬编码 `lineweight_index = 31`（写 raw u8 31）。`EntityCommonMinimal::lineweight` 字段在 M4.C 阶段**不实际生效**（仅作 future-proof 占位）；reader 读出 `lineweight = -3`，与 caller 输入无关。

> **决策**：M4.C minimal 版本不做 `i16 → u8 index` 反查，避免引入 `dwg_lineweight_from_index` 反函数（一个一对多映射；表里 `28/29/30/31` 都映射到负值）。后续 PR 加 `write_ac1015_entity_common_full` 时再做反查表。

同理 `color_index`、`linetype_scale`、`invisible`：M4.C minimal 直接透传 caller 的 `EntityCommonMinimal` 字段；它们都是无歧义的一对一映射。

### 2.3 hard-coded minimal 配置

writer 内部对以下字段使用固定值，不暴露给 caller：

```text
EED:               bit_short(0) 立即终止
has_graphic:       0
entity_mode:       0b01 (跳过 handle 流的 owner)
reactor_count:     0
nolinks:           1 (跳过 prev/next 链接)
linetype_flags:    0b00 (ByLayer; 不写 linetype handle)
plotstyle_flags:   0b00 (ByLayer; 不写 plotstyle handle)
lineweight_index:  31 (ByDefault; 反映为 i16 -3)
xdictionary:       NULL handle reference (code 5 + value 0)
```

---

## 3. 任务拆解

### 3.1 准备

- [ ] **T0** 浏览 ACadSharp `DwgObjectReader.cs` `readCommonEntityData`（`vendor_tmp` 中没有 ACadSharp 源码，回查 GitHub `https://github.com/DomCR/ACadSharp/blob/master/src/ACadSharp/IO/DWG/DwgObjectReader.cs`）确认 minimal 默认值是否被 reader 接受。**已通过本文档 §1 完成**：H7CAD reader 自身的解析路径 `parse_ac1015_entity_common_after_extended_data` 是事实上的 source of truth；任何 ACadSharp 行为差异由 H7CAD reader 单测捕获。

### 3.2 实现（vertical slice 单 PR）

- [ ] **T1** 创建 `crates/h7cad-native-dwg/src/writer/entity_common.rs`，导出 `EntityCommonMinimal` + `write_ac1015_entity_common_minimal`。
- [ ] **T2** 在 `writer/mod.rs` 与 `lib.rs` re-export。
- [ ] **T3** 单测在 `entity_common.rs` 内部模块：roundtrip via `parse_ac1015_entity_common`。
- [ ] **T4** `cargo test -p h7cad-native-dwg --all-targets` 全绿。
- [ ] **T5** `RUSTFLAGS=-Dwarnings cargo check -p h7cad-native-dwg --all-targets` 干净。
- [ ] **T6** 新文件 `rustfmt --edition 2021` + `ReadLints` 通过。
- [ ] **T7** 更新 CHANGELOG / progress.md / findings.md / 父子计划 §6 执行记录。

### 3.3 测试矩阵

| 测试 | 验证内容 |
|---|---|
| `entity_common_minimal_round_trips_layer_color_linetype_scale` | 经典字段（layer = 0x10、color = 7、linetype_scale = 0.5、invisible = false）→ writer → split → reader → 字段等价。reader 返回的 `owner_handle == Handle::NULL`、`linetype_handle == Handle::NULL`（按 minimal 设计），不与 caller 输入比较 |
| `entity_common_minimal_writes_null_xdictionary_first_in_handle_stream` | 检查 handle 流第一个 handle 是 NULL（xdictionary slot），第二个是 layer_handle。锁定 reader 时序的 caller 端假设 |
| `entity_common_minimal_lineweight_decodes_to_by_default` | reader 读出的 `lineweight == -3`（ByDefault 哨兵），与 caller 的 `lineweight` 字段无关 — 这是 M4.C minimal 的 *known limitation*，由测试显式锁定 |
| `entity_common_minimal_invisible_flag_round_trips` | invisible = true 与 false 两种 case 都通过 reader |
| `entity_common_minimal_round_trips_negative_color_index` | color_index = -7（图层 negative override）→ reader → 等价。BitShort 编码路径覆盖 |

---

## 4. 验收门

- 5 个新单测全绿；既存 325 个 native-dwg 测试零回归 → **lib 235 + 既存 95 = 330 全绿**。
- `cargo check --workspace --all-targets`：通过；零新增 warning。
- `RUSTFLAGS=-Dwarnings cargo check -p h7cad-native-dwg --all-targets`：干净。
- 新文件 `rustfmt --edition 2021 --check` 通过；`ReadLints` 零错误。
- facade `dwg_runtime_save_is_unavailable` 测试**保持锁定**（M4.C 仍然不接 entity 入 `write_dwg`）。
- 本文档 §3 的所有 task 勾上完成。

---

## 5. 风险与退路

| 风险 | 触发条件 | 退路 |
|---|---|---|
| reader 端某些 minimal 默认值实际上不被接受 | T3 第一个测试红 | 回查 `entity_common.rs` 内部 `safe_count` / `consume_optional_handle` 等帮助函数；如必要把 minimal 配置切换到 `entity_mode = 0b10` 或 `0b11`（reader 注释暗示这两个值也常见） |
| handle 流 NULL xdictionary 触发 reader 的 `consume_optional_handle` 异常 | T3 第二个测试红 | reader 函数会把 `Handle::NULL` 视为 None 返回，应该不报错。如真报错，改写 `write_handle(0x5, 0)` → `write_handle(0x4, 0)`（不同 code 都解码为 0） |
| reader 读 reactor handles 时尝试相对解码导致 wrap_around | T3 第三个测试红 | minimal 选 reactor_count = 0，绕过该路径。如果 reactor_count > 0 case 必须支持，留作 M4.C.2 |
| `lineweight = -3` 带来 caller 困惑 | M5 entity 实际想保真 lineweight | M4.C 限制为 minimal；M5 引入 `write_ac1015_entity_common_full` 时再加 `lineweight_index_from_value` 反查表 |
| ACadSharp 写出文件用 H7CAD M4.C reader 时 fail | 第三方互通 | M4.C 不承诺 ACadSharp 互通；按主计划 §10 DoD 落到 M5/M6 |

### 5.1 拆 PR 退路

如反推某字段默认值卡 1 小时以上，把 M4.C 拆成：
- **M4.C.1**：仅写 `bit_short(0)` 终结 EED + `bit(0)` graphic + `bits(0b01, 2)` entity_mode + `bit_long(0)` reactor + handle 流 xdictionary NULL + layer。删除其余字段；reader 报错位置即为下一片入口。
- **M4.C.2**：增量补全 nolinks/color/linetype_scale/linetype_flags/plotstyle_flags/invisible/lineweight。
- 每个 PR 都跑一次 `parse_ac1015_entity_common` roundtrip，记录最早失败字段。

---

## 6. 时间表

| 段 | 工作 | 估时 |
|---|---|---|
| T0 | 已通过本文档 §1 整理完成 | 0 |
| T1 | 编码 `EntityCommonMinimal` + writer 函数 | 30 min |
| T2 | re-export | 5 min |
| T3 | 5 个单测 + 调试 minimal 默认值 | 60–120 min |
| T4–T6 | 验证 + fmt + lint | 15 min |
| T7 | 文档 | 30 min |
| 缓冲 | 反推默认值卡壳 | 60 min |
| **合计** | | **2.5–4.5 h** |

---

## 7. 与父计划的关系

| 父子计划条目 | 关系 |
|---|---|
| `2026-05-09-dwg-m4-object-stream-writer-plan.md` §2.3 | 本子计划是其展开，落地后父子计划 §2.3 标记完成 |
| §2.4 M4.D LINE body writer | 接上本计划：M4.D 调用 `write_ac1015_entity_common_minimal` 后再写 LINE 几何字段；两个 BitWriter 喂给 M4.B compose |
| §2.5 M4.E `write_dwg` 集成 | 最简 entity = `write_ac1015_entity_common_minimal` + `write_line_geometry`；handle_offsets 由 cursor 累加得 |
| §2.6 退路 | 共享，本文档 §5 进一步细化 |

---

## 8. 立即可执行的第一批命令

```powershell
# T0 已用本文档替代

# T1 起步
# (创建 src/writer/entity_common.rs)

# T3 红绿循环
cargo test -p h7cad-native-dwg --lib writer::entity_common -- --nocapture

# 收尾
cargo test -p h7cad-native-dwg --all-targets
$env:RUSTFLAGS='-Dwarnings'; cargo check -p h7cad-native-dwg --all-targets; Remove-Item Env:RUSTFLAGS
rustfmt --edition 2021 --check crates\h7cad-native-dwg\src\writer\entity_common.rs
```

---

## 9. 执行记录（落地后填充）

| 日期 | 子任务 | 简述 | 验证 |
|---|---|---|---|
| 2026-05-09 | T1+T2 | `crates/h7cad-native-dwg/src/writer/entity_common.rs` 创建：`EntityCommonMinimal` + `write_ac1015_entity_common_minimal`；`writer/mod.rs` 与 `lib.rs` re-export | 编译通过 |
| 2026-05-09 | T3 | 5 个单测：典型字段 / invisible 双 case / 负 color_index / lineweight 锁定 ByDefault / `u64::MAX` layer handle | 5/5 首次运行就全绿（无 RED） |
| 2026-05-09 | T4–T6 | 跑 `cargo test -p h7cad-native-dwg --all-targets`：lib 235 + read_headers 53 + real_samples 38 + roundtrip_minimal 4 = 330 全绿；`RUSTFLAGS=-Dwarnings cargo check` 干净；`rustfmt + ReadLints` 通过 | M4.C 收口 |
| 2026-05-09 | T7 | CHANGELOG / progress.md / findings.md 同步 | 文档齐全 |

---

*起草者：H7CAD agent；2026-05-09。如范围调整请先改本文件再改代码。*
