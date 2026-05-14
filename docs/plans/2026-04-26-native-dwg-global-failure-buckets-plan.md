# Native DWG Global Failure Buckets 计划（R48-DWG-BUCKETS）

> 起稿：2026-04-26  
> 前置：R47-DWG-DIAG 已清理 supported-family false positives；
> `sample_AC1015.dwg` 的 supported family failure buckets 全部为 0。

## 1. 目标

把 AC1015 diagnostics 中剩余的 global failure buckets 转成可审计的代表 handle 清单，
为下一轮真正提升恢复数量选择对象类型或对象流问题。

## 2. 当前事实

`real_dwg_samples_baseline_m3b` 当前输出：

- recovered entities: 84
- supported family failure buckets: 全部为 0
- global failure buckets 仍存在：
  - `slice_miss`
  - `header_fail`
  - `handle_mismatch`
  - `common_decode_fail`
  - `unsupported_type`

因此下一步不能再使用已恢复的 `LINE/POINT/CIRCLE/ARC/LWPOLYLINE` 代表 handle；
必须从 global failures 中重新抽样。

## 3. 执行批次

| 任务 | 优先级 | 预估 |
|---|---:|---:|
| T1 新增 global failure bucket representatives 测试，按 kind 打印前几个 handle/type/stage | P0 | 0.4 h |
| T2 断言抽样对象不归属 supported family，避免回退到 false-positive 路线 | P0 | 0.3 h |
| T3 从输出中选择下一轮候选：优先 `unsupported_type` 或 `common_decode_fail`，跳过单纯 slice/header 噪声 | P1 | 0.3 h |
| T4 更新计划状态和候选清单 | P1 | 0.2 h |

## 4. 不纳入

- 不新增对象类型 decoder。
- 不改变 AC1015 recovery lower bound。
- 不修复 slice map 或 handle map。
- 不清理全 crate 格式化漂移。

## 5. 验收

```bash
cargo test -p h7cad-native-dwg ac1015_global_failure_bucket_representatives -- --nocapture
cargo test -p h7cad-native-dwg real_dwg_samples_baseline_m3b -- --nocapture
```

通过标准：

- 输出稳定列出剩余 global bucket 的代表 handle。
- 代表样本不再归因到已恢复 supported family。
- baseline 仍保持 84 entities，supported family failure buckets 仍为 0。

## 6. 状态

- [x] 计划定稿（2026-04-26）
- [x] T1 global bucket representatives 测试
- [x] T2 no supported-family 断言
- [x] T3 下一轮候选选择
- [x] T4 计划状态更新

## 7. 执行结论

新增 `ac1015_global_failure_bucket_representatives`，确认剩余 diagnostics failures
均未归属到已恢复 supported families，且 `body_decode_fail` 维持为 0。

当前代表样本：

| Kind | Count | Representatives |
|---|---:|---|
| `slice_miss` | 212 | `0x1781`, `0x1782`, `0x1783`, `0x1784`, `0x1787` |
| `header_fail` | 166 | `0x1785`, `0x1786`, `0x183B`, `0x183C`, `0x183E` |
| `handle_mismatch` | 2 | `0x185B(type=51:LAYER)`, `0x1862(type=256:?)` |
| `common_decode_fail` | 236 | `0x1(type=48:BLOCK_CONTROL)`, `0x2(type=50:LAYER_CONTROL)`, `0x3(type=52:STYLE_CONTROL)`, `0x5(type=56:LTYPE_CONTROL)`, `0x6(type=60:VIEW_CONTROL)` |
| `unsupported_type` | 254 | `0xE(type=500:CUSTOM_CLASS)`, `0x17(type=42:DICTIONARY)`, `0x18(type=73:?)`, `0x1A(type=42:DICTIONARY)`, `0x20(type=4:?)` |

下一轮建议优先做 `unsupported_type` 的对象族分流，而不是继续几何实体路径：

- `type=42 DICTIONARY` 可作为最小对象 decoder / skip contract。
- `type=500 CUSTOM_CLASS` 可作为 class metadata 解析入口或明确 skip。
- `common_decode_fail` 的 `*_CONTROL` 更像 table/object common 解析路径，不应走 entity common decoder。
