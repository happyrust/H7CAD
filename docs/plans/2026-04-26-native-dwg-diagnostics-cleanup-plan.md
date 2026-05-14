# Native DWG Diagnostics Cleanup 计划（R47-DWG-DIAG）

> 起稿：2026-04-26  
> 前置：R46-DWG-LWP 已证明 LWPOLYLINE 代表 `body_decode_fail`
> 是 diagnostics false positive；supported family failure buckets 已清零。

## 1. 目标

把 AC1015 real-sample diagnostics 从“代表 handle 必须继续失败”的旧假设，收敛为
“已恢复 supported families 不再被二次归因”的新契约。下一步开发应先确保测试面不再
鼓励 false positive，然后再从剩余 global buckets 中选择真正未恢复对象。

## 2. 当前事实

- `sample_AC1015.dwg` 当前恢复 84 个实体。
- `LINE / CIRCLE / ARC / POINT / TEXT / LWPOLYLINE / HATCH` 已恢复对象不应再被记录为
  failure。
- 旧测试仍假设 `LINE/POINT` 代表 handle 会出现在 diagnostics failure surface：
  `0x2C7 / 0x2CF / 0x517 / 0x28E / 0x298 / 0x299`。
- 实际 targeted trace 已显示这些 handle 的 `stage_before_fallback=None`、
  `first_missing_record=None`、`common_probe_stage=ok`。

## 3. 执行批次

| 任务 | 优先级 | 预估 |
|---|---:|---:|
| T1 跑 `ac1015_line` focused tests，确认仍失败的旧假设测试 | P0 | 0.2 h |
| T2 更新 LINE/POINT diagnostics tests：代表 handle 不再要求存在于 failure surface | P0 | 0.5 h |
| T3 保留字段级 probe/audit 测试作为几何语义研究资料，但把 diagnostics surface 断言改成 no false failure | P0 | 0.4 h |
| T4 跑 focused tests，确认 `ac1015_line` 与 LWPOLYLINE diagnostics 同时通过 | P0 | 0.3 h |
| T5 更新本计划状态，记录剩余 global bucket 后续选择策略 | P1 | 0.2 h |

## 4. 不纳入

- 不提升 AC1015 recovery lower bound。
- 不改 LINE/POINT 几何字段语义。
- 不扩大默认 DWG backend。
- 不处理 workspace 级格式化漂移。

## 5. 验收

```bash
cargo test -p h7cad-native-dwg ac1015_line -- --nocapture
cargo test -p h7cad-native-dwg ac1015_lwpolyline -- --nocapture
cargo test -p h7cad-native-dwg real_dwg_samples_baseline_m3b -- --nocapture
```

通过标准：

- 旧 LINE/POINT diagnostics surface 测试不再要求 false positive。
- targeted trace 对已恢复代表 handle 明确返回 no missing record。
- `real_dwg_samples_baseline_m3b` 仍保持 84 entities baseline。

## 6. 状态

- [x] 计划定稿（2026-04-26）
- [x] T1 focused failure 确认（`ac1015_line` 旧 diagnostics surface 断言失败）
- [x] T2 LINE/POINT diagnostics stale assertion 更新
- [x] T3 保留 probe/audit、改写 diagnostics contract
- [x] T4 focused verification（`ac1015_line` / `ac1015_lwpolyline` / `real_dwg_samples_baseline_m3b` 通过）
- [x] T5 计划状态更新

## 7. 执行结论

本轮没有新增实体恢复数量，而是清理上一轮 diagnostics 修复后暴露出来的陈旧测试契约：

- `LINE/POINT` 代表 handle `0x2C7 / 0x2CF / 0x517 / 0x28E / 0x298 / 0x299`
  不再要求出现在 diagnostics failure surface。
- targeted trace 对这些已恢复代表 handle 返回 `stage_before_fallback=None`、
  `first_missing_record=None`、`common_probe_stage=ok`。
- `real_dwg_samples_baseline_m3b` 继续保持 84 entities，且 supported family failure buckets 全部为 0。

下一轮若继续提升恢复数量，应从 `failure_counts` 的 global buckets 重新抽样未归属对象，
而不是复用已恢复 supported-family handle。
