# R51-DIAGNOSTICS-FALLBACK-DEDUP: drop synthetic BodyDecodeFail injected by fallback path

> 起稿：2026-04-28
> 前置：R47 调研、R48 facade 接通、R49 viewport sync、R50 store_entity graceful fallback。
> R47 第二轮 + R50 instrumentation 留下的"`body_decode_fail` 虚高、把已恢复
> entity 重复打标"问题，本轮**彻底闭环**。

## 1. 根因（已闭环）

`crates/h7cad-native-dwg/src/lib.rs::collect_ac1015_recovery_diagnostics_with_known_successes`
（line 320–454）由两段构成：

1. **主循环**：遍历 `pending.handle_offsets`，对每个 handle 调用
   `try_decode_entity_body_with_reason`。失败时调用
   `record_failure(BodyDecodeFail, …)`；**Ok 分支什么都不记录**。
2. **fallback 循环**：遍历 `supported_family_hints`，对每个有 `family`
   的 hint，如果 `diagnostics.failures` 里**没有**该 (handle, family)
   组合的失败记录，就 `record_failure(BodyDecodeFail, …)` 合成一条。

**bug**：fallback 循环用 `has_family_failure` 作为唯一去重 guard，
**误把"主循环成功处理（Ok 分支）"和"fallback 应该补打标"混为一谈**。
后果是 sample_AC1015.dwg 上：

- 主循环成功 try_decode 的 82 个 LINE 全部通过 fallback 被合成虚假
  `BodyDecodeFail` 记录；
- `failure_counts[BodyDecodeFail]` 被吹高到包含已 recover entity；
- 用户看到 `LINE recovered=82 vs body_decode_fail=82` 的诡异对照。

R47 第二轮 instrumentation 已确认 "decoded_ok=82" 在 enrich 阶段，
R50 plan 末尾把这个 dedup 修复列为 R51 候选。

## 2. 修复

`collect_ac1015_recovery_diagnostics_with_known_successes` 在 fallback
循环开始前先建立 `processed_in_main_loop: BTreeSet<Handle>`（即所有
`pending.handle_offsets.iter().map(|e| e.handle)`），fallback 循环遇到
已在该集合中的 handle 时直接 `continue`：

```rust
let processed_in_main_loop: std::collections::BTreeSet<Handle> = pending
    .handle_offsets
    .iter()
    .map(|entry| entry.handle)
    .collect();
for (handle, hint) in supported_family_hints.iter() {
    let Some(family) = hint.family else { continue; };
    if processed_in_main_loop.contains(handle) {
        // Main loop already saw this handle; either it failed (recorded
        // above) or it succeeded silently — either way the fallback must
        // not invent a synthetic failure record.
        continue;
    }
    // … existing fallback synthesis …
}
```

语义：fallback 路径只为**主循环根本没遍历过**的 handle（即出现在
`supported_family_hints` 但不在 `pending.handle_offsets` 里的——
preheader hint 派生而来）合成失败记录，避免与主循环职责重叠。

## 3. 4 个回归测试反向化

R51 修复揭示了 4 个旧测试**正在测 buggy 行为**——它们的预期值依赖
fallback 重复打标。R51 修复后那些虚假记录消失，旧测试 fail。**正确做法
是把这 4 个测试反向化**：原断言全部翻转，让它们成为 R51 修复的回归
evidence。删除这些测试会丢失对未来 buggy 行为再现的检测；反向化既保留
检测能力，又把测试信号校准到真实数据。

| Test | 旧断言（修复前 buggy 行为） | 新断言（修复后真实状态） |
|---|---|---|
| `real_dwg_samples_baseline_m3b` line 349–358 | `failure_counts_by_family.contains_key("LINE") ‖ … ‖ contains_key("HATCH")`，要求至少一个 supported family 有失败 bucket | 改为 `!diagnostics.failures.is_empty()`：诊断仍能捕获 unsupported_type / slice_miss 等真实失败，但不再要求 supported family 必须有失败 bucket |
| `ac1015_recovery_diagnostics_attribute_supported_families_from_preheader_hints` | 每个 LINE/POINT/CIRCLE/ARC/LWPOLYLINE family 必须有 `BodyDecodeFail` bucket > 0 | 反向：每个 family 的 `BodyDecodeFail` bucket **必须 == 0**（R47/R50 修复后主循环全成功，R51 修复后 fallback 不再合成虚假记录） |
| `ac1015_line_point_post_common_body_audit_reports_representative_failure_stage` | representative LINE handles `0x2C7/0x2CF/0x517` 与 POINT handles `0x28E/0x298/0x299` 必须在 `diagnostics.failures` 里且 kind==`BodyDecodeFail` | 反向：这些 representative handles **已被 R47/R50 成功 recover**，不再出现在 `failures` 里；同时验证它们出现在 `doc.entities` 里 |
| `ac1015_line_point_blocked_handles_real_decode_path_advances_after_selective_fix` | stuck LINE `0x99E/0x9CD/0x9D4` 与 POINT `0x298/0x29A` 必须在 `failures` 里且 kind ∈ {`CommonDecodeFail`, `BodyDecodeFail`} | 反向：这些 stuck handles **已被 R47/R50 成功 recover**，不再出现在 `failures` 里 |

## 4. 验收

```bash
cargo test -p h7cad-native-dwg --test real_samples
cargo test --locked --workspace --all-targets
RUSTFLAGS=-Dwarnings cargo check --locked --workspace --all-targets
```

通过标准：

- `real_dwg_samples_baseline_m3b` 的 failure bucket 输出显示
  `family=LINE … body_decode_fail=0`（不再 inflate）；全局
  `body_decode_fail=0`（未支持的 family 走 unsupported_type bucket）；
- 4 个反向化测试 pass，作为 R51 dedup 修复的明确回归 evidence；
- workspace test 100% pass，无回归；
- `-Dwarnings cargo check workspace` 干净。

## 5. 范围

| 任务 | 状态 | 优先级 | 预估 |
|---|---|---:|---:|
| T1 落 R51 plan 文件 | ✅ 完成 | P0 | 0.2 h |
| T2 lib.rs dedup 修复（已存在 working tree） | ✅ 完成 | P0 | 0 h |
| T3 反向化 baseline_m3b 第 349–358 行 assertion | ✅ 完成 | P0 | 0.2 h |
| T4 反向化 attribute_from_preheader_hints 测试 | ✅ 完成 | P0 | 0.2 h |
| T5 反向化 post_common_body_audit 测试 | ✅ 完成 | P0 | 0.3 h |
| T6 反向化 blocked_handles_real_decode_path 测试 | ✅ 完成 | P0 | 0.3 h |
| T7 跑双重门验收 | ✅ 完成 | P0 | 0.4 h |

## 6. 不纳入

- 不重写 fallback 循环的合成逻辑——`processed_in_main_loop` skip 已经是
  最小修复；fallback 仍在 hint-only handle（不在 `pending.handle_offsets`）
  上有意义。
- 不动 `try_decode_entity_body_with_reason` 主循环的 record_failure 路径。
- 不动 `enrich_with_real_entities` 的 store_entity 调用（R50 已修）。
- 不修 AC1018 native reader（R46 候选）。
- 不深挖未恢复 entity 的真实 `owner_handle` 解析逻辑（R50 plan §8 长期项）。

## 7. 风险

- 反向化把"原本期待 supported family 有 BodyDecodeFail"翻成"必须 == 0"。
  如果未来某个 family 解码退化，本测试会先 fail——这正是反向化想要的效果。
  但要记住：sample_AC1015.dwg 上 supported family **当前实际 0 失败**，
  不是 R51 修复"hide"的，而是 R47/R50 修复后主循环 try_decode 真的全部成功。

## 8. 状态

- [x] 计划定稿（2026-04-28）
- [x] T2 lib.rs dedup 修复
- [x] T3 baseline_m3b 反向化
- [x] T4 attribute_from_preheader_hints 反向化
- [x] T5 post_common_body_audit 反向化
- [x] T6 blocked_handles_real_decode_path 反向化
- [x] T7 双重门验收（cargo test workspace 46 binary 0 failed；RUSTFLAGS=-Dwarnings cargo check workspace 2.55s ok）
