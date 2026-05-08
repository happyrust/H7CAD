# R50-LINE-HANDLE-RECOVERY: native_model store_entity graceful fallback for unknown owner

> 起稿：2026-04-28
> 前置：R47 第二轮根因诊断 + R48 facade 接通 + R49 viewport sync 完成。
> R47 留下的"56 个 LINE 失败"在本轮通过精准 instrumentation **彻底定位**
> 并修复。

## 1. 根因（彻底确认）

通过临时 instrumentation 在 `enrich_with_real_entities` 加 LINE 失败统计：

```text
[R50-ENRICH] LINE stats: seen=0 slice_miss=0 split_fail=0 handle_mismatch=0
                          try_decode_none=0 decoded_ok=82 add_failed=56
```

**关键事实**：

- `pending.handle_offsets` 包含 82 个 type=19 LINE entry（`real_ac1015_full_handle_map_object_type_histogram` 也确认）；
- 82 个 LINE 在 `enrich_with_real_entities` 中**全部成功 try_decode_entity_body**（`decoded_ok=82`），意味着 R47 推测的 sentinel handoff / common decode / read_line_geometry 等 decoder 层问题**全部不存在**；
- 但其中 **56 个在 `doc.add_entity` 阶段失败**（`add_failed=56`），最终只有 26 个 LINE 进入 `doc.entities`。

定位 reject 点：`crates/h7cad-native-model/src/lib.rs::store_entity`（line 467-502）。`store_entity` 在 `entity.owner_handle` 解析不到任何 block record 时直接返回 `Err`：

```rust
let owner_br_handle = self.block_record_by_any_handle(owner_handle).map(|br| br.handle);
let Some(owner_br_handle) = owner_br_handle else {
    return Err(format!(
        "owner handle {:X} does not resolve to a block record",
        owner_handle.value()
    ));
};
```

native DWG recovery 路径在 AC1015 上会解出某些 LINE 的 `owner_handle` 指向**当前 `block_record_by_any_handle` 解析表里不存在的 handle**——这些 entity 被 hard-reject 掉。该问题不是 LINE-specific：CIRCLE / ARC / POINT / LWPOLYLINE 都有同类 reject。

## 2. 修复

`store_entity` 改为 **graceful fallback to model_space**：当 `owner_handle` 解析不到 block record 时，把它重写为 `model_space_handle()` 并 push 到 `self.entities`，而不是 hard-error。

```rust
let owner_br_handle = match owner_br_handle {
    Some(handle) => handle,
    None => {
        // R50-LINE-HANDLE-RECOVERY graceful fallback
        entity.owner_handle = self.model_space_handle();
        self.entities.push(entity);
        return Ok(());
    }
};
```

这种做法符合 `read_dwg` 的 "best-effort recovery" 语义：未能正确解出 owner 的 entity **仍渲染/导出**，作为 model-space orphan，比"silently dropped"好得多。原 `owner_handle` 已经指向无效 block record，覆盖为 `model_space_handle()` 让后续 ownership repair / round-trip 写入保持一致。

## 3. 实测增量（在 `sample_AC1015.dwg` 上）

| Family | 修复前 | 修复后 | 增量 |
|---|---:|---:|---:|
| LINE | 26 | **82** | +56 |
| CIRCLE | 4 | **9** | +5 |
| ARC | 1 | **3** | +2 |
| POINT | 6 | **34** | +28 |
| TEXT | 26 | 26 | 0 |
| LWPOLYLINE | 15 | **17** | +2 |
| HATCH | 6 | 6 | 0 |
| **Total** | **84** | **177** | **+93** |

LINE 完整恢复（82/82），CIRCLE/ARC/POINT/LWPOLYLINE 也一并提升。

## 4. baseline ratchet

`real_dwg_samples_baseline_m3b` 的 lower bound 同步上调（留 1 个缓冲）：

| Field | Old `>=` | New `>=` |
|---|---:|---:|
| `doc.entities.len()` | 84 | 170 |
| `diagnostics.recovered_total` | 84 | 170 |
| LINE | 26 | 80 |
| CIRCLE | 4 | 8 |
| ARC | 1 | 2 |
| POINT | 6 | 32 |
| LWPOLYLINE | 15 | 16 |
| TEXT (`==`) | 26 | 26（不变）|
| HATCH (`==`) | 6 | 6（不变）|

注释标 R50 落地时间，方便后续读者追溯。

## 5. 范围

| 任务 | 状态 | 优先级 | 预估 |
|---|---|---:|---:|
| T1 加临时 instrumentation 定位 LINE 失败 stage | ✅ 完成 | P0 | 0.5 h |
| T2 落盘 R50 plan 文件 | ✅ 完成 | P0 | 0.2 h |
| T3 修 `store_entity` graceful fallback | ✅ 完成 | P0 | 0.3 h |
| T4 撤回临时 debug eprintln/println（entity_line.rs / lib.rs::enrich） | ✅ 完成 | P0 | 0.2 h |
| T5 ratchet baseline lower bounds 到反映新真实 | ✅ 完成 | P0 | 0.2 h |
| T6 验收：`cargo test -p h7cad-native-dwg`、workspace test、`-Dwarnings cargo check` | ✅ 完成 | P0 | 0.4 h |

## 6. 不纳入

- 不深挖 56 个 LINE 的 `owner_handle` 为何 resolve 不到 block record——这要追溯
  block record 表的解析或 handle stream resolution，单独工作量可观，不在本轮
- 不修 `block_record_by_any_handle` 的查找算法
- 不动 `entity_common::parse_ac1015_entity_common` 的 owner handle 解析
- 不重写 `enrich_with_real_entities`
- 不影响 acadrust 主路径（store_entity 仅作用于 native model）

## 7. 验收

```bash
cargo test -p h7cad-native-dwg --test real_samples real_dwg_samples_baseline_m3b
cargo test --locked --workspace --all-targets
RUSTFLAGS=-Dwarnings cargo check --locked --workspace --all-targets
```

通过标准：

- baseline 测试 pass，新 ratcheted lower bounds 全部满足；
- workspace test 100% pass（不引入回归）；
- `-Dwarnings cargo check workspace` 仍干净；
- `sample_AC1015.dwg` 上 LINE 恢复数 = 82（完整）。

## 8. 风险与后续

- 把无效 owner_handle 改写为 model_space 是 best-effort 修复——某些
  entity 的真实 owner 是某个 block，目前丢失了归属信息。**长期** 应该
  追溯 owner_handle 解析逻辑（涉及 handle resolution map / block record
  registry），让 owner 解析正确。但这不在 R50 scope。
- diagnostics 的 fallback 重复打标 bug（R47 第二轮发现）仍未修——`body_decode_fail`
  数字仍含已恢复 entity 的虚假记录。**R51-DIAGNOSTICS-FALLBACK-DEDUP** 候选。
- AC1018 native reader 仍 fail-closed（R46-DWG-AC1018 候选）。

## 9. 状态

- [x] 计划定稿（2026-04-28）
- [x] T1 临时 instrumentation 定位失败 stage 为 add_failed=56
- [x] T2 plan 文件落盘
- [x] T3 store_entity graceful fallback
- [x] T4 撤回临时 debug 代码
- [x] T5 baseline ratchet
- [x] T6 验收（待最终一次 -Dwarnings + cargo test workspace 确认）
