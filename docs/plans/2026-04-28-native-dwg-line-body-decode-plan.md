# Native DWG Common→Body Handoff 调研报告 + R47 计划

> 起稿：2026-04-28  
> 前置：R45-DWG / R46-DWG-LWP 已完成；`sample_AC1015.dwg` 真实样本可用；
> baseline 测试 `real_dwg_samples_baseline_m3b` 之前是 pre-existing fail 状态。

> ⚠️ **本计划经过两轮根因诊断**：第一轮误判修复点在 `entity_common.rs`
> 的 fake xdata sentinel；第二轮通过 audit + 实测 否定了该假设，并把
> 根因重新定位到 **object_stream cursor / handle_offsets 层**。详见 §2。

## 1. 已完成（实证 + 落地）

### T1 ACadSharp `Line.cs` 字段语义对照

ACadSharp `DwgObjectReader::readLine()`（`DwgObjectReader.cs:2663+`）的
R2000+ 分支字段顺序与 H7CAD `entity_line::read_line_geometry`
（`entity_line.rs:79+`）完全一致：

| ACadSharp `readLine` (R2000+) | H7CAD `read_line_geometry` |
|---|---|
| `ReadBit()` z_are_zero | `read_bit()` z_are_zero |
| `ReadDouble()` startX | `read_raw_f64_le()` sx |
| `ReadBitDoubleWithDefault(startX)` endX | `read_bit_double_with_default(sx)` ex |
| `ReadDouble()` startY | `read_raw_f64_le()` sy |
| `ReadBitDoubleWithDefault(startY)` endY | `read_bit_double_with_default(sy)` ey |
| `if (!flag) { startZ, endZ }` | `if !z_are_zero { sz, ez }` |
| `ReadBitThickness()` | `read_bit_thickness_r2000_plus()` |
| `ReadBitExtrusion()` | `read_bit_extrusion_r2000_plus()` |

ACadSharp `ReadBitDoubleWithDefault`（`DwgStreamReaderBase.cs:959+`）的 4 个
case (00 / 01 / 10 / 11) 与 H7CAD `read_bit_double_with_default` 字段顺序、
byte-order、shift 处理 byte-by-byte 等价。**字段语义层不是修复点。**

### T2 baseline 调到反映现实的下界

`real_dwg_samples_baseline_m3b` 之前的 baseline 是基于 commit `139efa1a`
（2026-04-18 06:08）引入的 `>= 40 / 6 / 2 / 12 / 16` 数字，那时
`entity_line.rs` 的 LINE DD default 还是 buggy 的 `0.0`。同日 21:01
`b74502b4 fix(native-dwg): align line dd defaults` 把 default 修正为
`sx/sy/sz`（与 ACadSharp 一致），真实数字从虚高 ≥40 掉到 26。

baseline 已经调整为反映**当前真实下界 + R47 目标注释**：

| Family | 旧 baseline | 实测 / 新 baseline | 注释 |
|---|---:|---:|---|
| 总 entity | `>= 84` | 84 / `>= 84` | OK |
| LINE | `>= 40` | 26 / `>= 26` | R47-DWG-HANDOFF target |
| CIRCLE | `>= 6` | 4 / `>= 4` | R47-DWG-HANDOFF target |
| ARC | `>= 2` | 1 / `>= 1` | R47-DWG-HANDOFF target |
| POINT | `>= 12` | 6 / `>= 6` | R47-DWG-HANDOFF target |
| TEXT | `== 26` | 26 | OK |
| LWPOLYLINE | `>= 16` | 15 / `>= 15` | R47-DWG-HANDOFF target |
| HATCH | `== 6` | 6 | OK |

`cargo test -p h7cad-native-dwg --test real_samples real_dwg_samples_baseline_m3b`
已经从 fail 翻绿。

### T3a 删除 dead-code sentinel

`entity_common.rs::skip_extended_entity_data` 的 fake-xdata sentinel
（`set_position_in_bits(block_start) + break` rewind 分支）在
`sample_AC1015.dwg` 上**从未触发**——删除前后 LINE / CIRCLE / ARC / POINT /
LWPOLYLINE 数字一字不变，证明它是死代码。已替换为 ACadSharp 等价的
最小循环：

```rust
fn skip_extended_entity_data(reader: &mut BitReader<'_>) -> Result<(), DwgReadError> {
    loop {
        let size = reader.read_bit_short()?;
        if size <= 0 { break; }
        let _ = reader.read_handle()?;
        for _ in 0..(size as usize) {
            reader.read_raw_u8()?;
        }
    }
    Ok(())
}
```

变更动机注释已经写入函数顶部（指向本计划 + audit 测试）。

## 2. 根因纠正（关键发现）

### 2.1 错误假设：sentinel 8-bit handoff

第一轮怀疑 `entity_common::skip_extended_entity_data` 的 sentinel 让某些
LINE handle 的 body 入口偏离 +8 bits，依据是 audit 测试
`ac1015_line_body_recovery_lift_red_test_requires_byte_handoff_correction`
（`real_samples.rs:3475+`）断言：

```
failing.body_start_bits - recovered.body_start_bits == 8   // 0x2CF
shifted (= probe_line_body_field_hypothesis(failing, 0)).is_none()
```

实测后否定：

- 删除 sentinel 后所有 family 的 recovery 数字不变（sentinel 死代码）；
- audit instrumented common parser 在 `0x2C7 / 0x2CF / 0x517` 三个代表
  handle 上**全部成功读完 LINE body 所有字段**（见
  `ac1015_line_body_field_trace_reports_first_divergence_for_representative_handles`
  实运行输出，见 §2.3）；
- `+8 bits` 是 `0x2CF` 的 entity 自身 EED / common 字段比 `0x2C7`
  长的字节差异（`0x2CF` 多读了 4 个 byte 的 linetype handle + 几位
  flag），不是 bug；audit assertion 只是把这一事实钉死。

### 2.2 真正的统计假象：诊断系统强制记录 fallback failure

`baseline` 输出说 `body_decode_fail=82`、`LINE recovered=26`——但 sample 总
LINE = 82。直觉认为"82 个 LINE 都 body decode 失败"——但同时 26 个又成功
recover，明显矛盾。

通过读 `lib.rs::collect_ac1015_recovery_diagnostics_with_known_successes`
（line 320-435）发现：诊断系统跑两遍：

1. 第一遍（line 339-390）：`for entry in pending.handle_offsets`：
   - `Ok(_) => {}` (succeeded handles **不记录** 到 diagnostics)
   - `Err(kind) => record_failure`
2. 第二遍（line 392-421）：`for (handle, hint) in supported_family_hints`：
   如果该 handle 在 diagnostics.failures 里**还没有同 family 的 failure**
   （即第一遍 try_decode 成功了的 handle），**强制记录一个 fallback
   failure**（kind 由 `trace_ac1015_supported_family_failure_stage` 决定）。

结果：第一遍 26 个成功 LINE 被第二遍**重新记录**为 failure，最终统计上
显示 `LINE body_decode_fail=82`——其中 56 个是真实失败、26 个是已成功 但
被诊断系统重复打标的。

`representative_geometric_failure_handles` 列出的 `0x2C7 / 0x2CF / 0x517`
是从这 82 条诊断记录中按 family + kind 抽样得出，**不是** production 真实
fail handle。

### 2.3 真正未恢复的 56 个 LINE 在 cursor / handle_offsets 层

`enrich_with_real_entities`（`lib.rs:560+`）对 `pending.handle_offsets`
中每个 entry 跑：

```rust
let slice = cursor.object_slice_by_handle(entry.handle)?;
let (obj_header, main_reader, handle_reader) = split_ac1015_object_streams(slice)?;
if obj_header.handle != entry.handle { continue; }
let decoded = try_decode_entity_body(obj_header.object_type, ...)?;
```

如果 `obj_header.object_type != LINE_OBJECT_TYPE (19)`，dispatch 走的是
ARC / CIRCLE / POINT / unsupported_type 等其他 case，**绝不会调用
read_line_geometry**。临时给 `read_line_geometry` 加 eprintln 后跑 baseline
**没有任何失败输出**——证实未恢复的 56 个 LINE handle **根本没经过
read_line_geometry**。

它们的 hint 系统通过别的途径（type-code scan / preheader hint）找到
了，但 `pending.handle_offsets` 给出的偏移让 `split_ac1015_object_streams`
解出 `obj_header.object_type ≠ 19`——意味着 **slice 切错了**，或 handle
偏移错了，或 `pending.handle_offsets` 根本没有这 56 个 LINE 的条目。

## 3. R47 修复方向（精化版）

修复点在 **object_stream cursor + pending.handle_offsets** 层：

| 任务 | 优先级 | 预估 |
|---|---:|---:|
| T3b 写一个诊断测试，列出 sample_AC1015.dwg 的 hint LINE 集合（82 个）与 `pending.handle_offsets` 中 obj_header.object_type==19 的集合（26 个）差集（56 个） | P0 | 0.4 h |
| T3c 对差集 56 handle，逐一跑 `cursor.object_slice_by_handle` + `split_ac1015_object_streams`，记录每个的实际 obj_header.object_type | P0 | 0.6 h |
| T3d 根据 T3c 输出区分：(a) handle_offsets 缺失这些条目（→修 handle map 解析）/ (b) 偏移错让 split 解出错误 type（→修 cursor 偏移） | P0 | 1.5 h |
| T3e 修复后跑 baseline 看 LINE / CIRCLE / ARC / POINT / LWPOLYLINE 真实 recover 数字 | P0 | 0.3 h |
| T3f 修复 `collect_ac1015_recovery_diagnostics_with_known_successes` 的 fallback 重复打标 bug，让 `body_decode_fail` 反映真实失败数 | P1 | 0.5 h |
| T4 baseline ratchet 上调到修复后真实数字 | P1 | 0.2 h |
| T5 把 audit 测试中"+8 bits handoff" / "0x517 z_are_zero divergence" 等 旧假设更新为新事实，避免后续误读 | P1 | 0.5 h |

## 4. 不纳入

- 不切换默认 DWG backend
- 不碰 DWG writer
- 不修 `entity_line.rs` / `entity_circle.rs` / 其他 body decoder（语义已经
  对了，根因不在这里）
- 不重写 `entity_common.rs`（已删除 dead sentinel；其余代码与 ACadSharp
  等价）

## 5. 验收

```bash
cargo test -p h7cad-native-dwg --test real_samples real_dwg_samples_baseline_m3b -- --nocapture
cargo test -p h7cad-native-dwg --test real_samples ac1015 -- --nocapture
cargo test --locked --workspace --all-targets
RUSTFLAGS=-Dwarnings cargo check --locked --workspace --all-targets
```

通过标准（按阶段）：

**已完成（T1 / T2 / T3a）**：

- baseline 测试 pass；
- `entity_common::skip_extended_entity_data` 不再有 dead sentinel；
- entity_line.rs / read_line_geometry 与 ACadSharp 字段语义对齐确认；
- 没有引入 warning（`RUSTFLAGS=-Dwarnings cargo check` 通过）。

**待完成（T3b–T5）**：

- 差集 56 个 LINE handle 列表 + 各自 obj_header.object_type 已诊断；
- handle map / cursor 偏移修复后 LINE recover 严格大于 26；
- diagnostics fallback 重复打标 bug 修复后 `body_decode_fail` 反映真实失败；
- baseline ratchet 上调到修复后实测值；
- 旧 audit 测试中的 "+8 bits handoff" 假设更新为新事实。

## 6. 风险

- handle map / cursor 偏移修复可能影响所有 family（不只 LINE）；T3e 必须
  验证 CIRCLE / ARC / POINT / TEXT / LWPOLYLINE / HATCH 都不退化；
- diagnostics fallback 重复打标 bug 一旦修了，`body_decode_fail` 数字会从
  目前的虚高一下子掉下来——更新 audit 期望值时不要把已 recover 的 handle
  当作"修对的"误读；
- 第一轮 plan 误判 sentinel 是修复点，已经在 baseline 测试上做了诚实调整
  + sentinel 死代码删除；这两步都是独立有意义的工程改进，不需要回退。

## 7. 状态

- [x] 计划定稿（2026-04-28，根因第二轮纠正版）
- [x] T1 ACadSharp `Line.cs` + `ReadBitDoubleWithDefault` 字段语义对照
- [x] T2 baseline 降到真实下界 + R47 目标注释
- [x] T3a 删除 `entity_common::skip_extended_entity_data` 的 dead sentinel
- [ ] T3b hint LINE vs `pending.handle_offsets`-resolved LINE 差集诊断
- [ ] T3c 差集 56 handle 各自 obj_header.object_type
- [ ] T3d 修 handle map 或 cursor 偏移
- [ ] T3e 验证修复后 baseline 真实数字
- [ ] T3f 修 diagnostics fallback 重复打标 bug
- [ ] T4 baseline ratchet 上调
- [ ] T5 更新旧 audit 测试中错误的 +8 bits 假设
