# Native DWG LWPOLYLINE Body Decode 计划（R46-DWG-LWP）

> 起稿：2026-04-26  
> 前置：`sample_AC1015.dwg` 真实样本可用；`real_dwg_samples_baseline_m3b`
> 当前已经恢复至少 15 个 LWPOLYLINE，但代表失败 handle 仍停在
> `entity_body_decode`。

## 1. 目标

把 AC1015 LWPOLYLINE 的下一批 `body_decode_fail` 转成可解释、可修复的字段级差异，
优先修复一个最小根因，并在成功后提高 `real_dwg_samples_baseline_m3b` 的
LWPOLYLINE lower bound。

## 2. 当前代表失败

来自：

```bash
cargo test -p h7cad-native-dwg ac1015_representative_geometric_failure_handles -- --nocapture
```

| Family | Failure kind | Stage | Representative handles |
|---|---|---|---|
| LWPOLYLINE | `body_decode_fail` | `entity_body_decode` | `0x2E2`, `0x2E3`, `0x2E4` |

这些 handle 已经越过 object header 和 common entity decode，说明下一步应聚焦
`crates/h7cad-native-dwg/src/entity_lwpolyline.rs::read_lwpolyline_geometry` 的字段顺序、
flag 语义、点坐标编码或宽度/bulge/vertex id 计数处理。

## 3. 执行批次

| 任务 | 优先级 | 预估 |
|---|---:|---:|
| T1 新增 focused diagnostic test，锁定 `0x2E2/0x2E3/0x2E4` 都停在 `entity_body_decode`，避免后续误判成 common/header 问题 | P0 | 0.2 h |
| T2 增加 LWPOLYLINE body probe helper，逐字段记录 flag、count、坐标读取位置、失败字段和剩余 bit 数 | P0 | 0.6 h |
| T3 对比一个已恢复 LWPOLYLINE 和一个失败 LWPOLYLINE 的 body trace，形成单一根因假设 | P0 | 0.5 h |
| T4 按 TDD 修复一个最小字段规则，例如 flag bit、vertex id 计数或 DD 坐标默认值读取差异 | P1 | 0.8 h |
| T5 提升 `real_dwg_samples_baseline_m3b` 的 LWPOLYLINE lower bound，并保留代表失败清单更新 | P1 | 0.3 h |

## 4. 不纳入

- 不碰 DWG writer。
- 不切换默认 DWG backend。
- 不同时修 LINE/POINT/CIRCLE/ARC。
- 不把 synthetic fixture 的成功视为真实对象流修复。

## 5. 验收

```bash
cargo test -p h7cad-native-dwg ac1015_lwpolyline -- --nocapture
cargo test -p h7cad-native-dwg real_dwg_samples_baseline_m3b -- --nocapture
cargo test -p h7cad-native-dwg ac1015_representative_geometric_failure_handles -- --nocapture
```

通过标准：

- focused diagnostic test 能稳定定位 LWPOLYLINE 代表 handle 的失败阶段；
- 修复后 `read_dwg(sample_AC1015.dwg)` 的 LWPOLYLINE 恢复数增加；
- lower bound 只在真实样本恢复数提升后上调；
- 其它已恢复 family 的 baseline 不下降。

## 6. 状态

- [x] 计划定稿（2026-04-26）
- [x] T1 focused diagnostic test（`0x2E2/0x2E3/0x2E4`：common decode `ok`，first missing record 为 `EntityBodyDecode`）
- [x] T2 LWPOLYLINE body probe helper（字段级 probe 可完整读完 `0x2E2/0x2E3/0x2E4`）
- [x] T3 recovered vs failing trace 对比（结论：代表 handle 不是 body parser 失败，而是 diagnostics trace false positive）
- [x] T4 最小字段规则修复（改为 diagnostics 修复：common probe 使用 cloned reader，避免二次消耗 body reader）
- [x] T5 baseline lower bound 评估（恢复数仍为 15；本轮清理 false failure bucket，不上调 lower bound）

## 7. 执行结论

字段级 probe 证明 `0x2E2`、`0x2E3`、`0x2E4` 的 LWPOLYLINE body 都能完整读取：

- `0x2E2`: `flag=0`, `num_pts=5`，5 个点全部读完。
- `0x2E3`: `flag=512`, `num_pts=7`，7 个点全部读完。
- `0x2E4`: `flag=528`, `num_pts=7`, `num_bulges=7`，点和 bulge 全部读完。

根因不是 `entity_lwpolyline.rs` 字段规则，而是 diagnostics 辅助路径在
`probe_ac1015_entity_common` 后复用同一个 reader 调 `try_decode_entity_body_with_reason`，
导致 common 字段被二次消费，误报为 `EntityBodyDecode`。修复后：

- `trace_ac1015_targeted_failure_before_fallback` 对三条 LWPOLYLINE handle 返回 no missing record。
- `collect_ac1015_recovery_diagnostics` 不再把已恢复的 supported families 重新归因成 failure。
- `real_dwg_samples_baseline_m3b` 仍恢复 84 个实体，其中 LWPOLYLINE 为 15；diagnostics 的
  supported family failure bucket 全部为 0。
