# Native DWG CUSTOM_CLASS Skip Contract 计划（R51-DWG-CUSTOM）

> 起稿：2026-04-26  
> 前置：R50-DWG-DICT-SKIP 已把 `type=42 DICTIONARY` 从 generic
> `body_dispatch` 推进到 `object_body_skip`；global representatives 中仍有
> `type=500 CUSTOM_CLASS`。

## 1. 目标

为 AC1015 custom class object（`object_type >= 500`）建立最小 skip contract：
先消费 non-entity common，成功后仍保守返回 `UnsupportedType`，但 diagnostics stage
细化为 `custom_class_body_skip`。

## 2. 当前候选

来自 `ac1015_global_failure_bucket_representatives`：

| Handle | Object type | Current kind | Stage |
|---|---:|---|---|
| `0xE` | `500:CUSTOM_CLASS` | `unsupported_type` | `body_dispatch` |

## 3. 执行批次

| 任务 | 优先级 | 预估 |
|---|---:|---:|
| T1 新增 focused test，锁定 `0xE` 是 custom class object | P0 | 0.2 h |
| T2 验证 `0xE` 可通过 non-entity common，并将期望 stage 改为 `custom_class_body_skip` | P0 | 0.3 h |
| T3 在生产 diagnostics 路径接入 `object_type >= 500` 的 non-entity common skip contract | P0 | 0.4 h |
| T4 跑 focused tests 与 AC1015 baseline | P1 | 0.3 h |

## 4. 不纳入

- 不解析 Classes section 与 custom class number 的映射。
- 不解析 custom class payload。
- 不改变 recovered entity lower bound。
- 不处理 `*_CONTROL` common decode path。

## 5. 验收

```bash
cargo test -p h7cad-native-dwg ac1015_custom_class -- --nocapture
cargo test -p h7cad-native-dwg ac1015_global_failure_bucket_representatives -- --nocapture
cargo test -p h7cad-native-dwg real_dwg_samples_baseline_m3b -- --nocapture
```

通过标准：

- `0xE` 仍是 `UnsupportedType`，但 stage 变为 `custom_class_body_skip`。
- non-entity common probe 可输出 owner/remaining bits。
- supported family failure buckets 继续为 0。

## 6. 状态

- [x] 计划定稿（2026-04-26）
- [x] T1 focused custom class test
- [x] T2 RED stage assertion
- [x] T3 skip contract 实现
- [x] T4 focused verification

## 7. 执行结论

本轮实现了 custom class object 的最小 skip contract：

- `object_type >= 500` 进入 entity-body dispatch 前先消费 `parse_ac1015_non_entity_common`。
- common 成功后仍返回 `UnsupportedType`，但 diagnostics stage 改为
  `custom_class_body_skip`。
- common 失败仍保留 `CommonDecodeFail`。

Focused probe 确认：

| Handle | Type | Owner | Main position | Main remaining | Handle position | Handle remaining |
|---|---:|---:|---:|---:|---:|---:|
| `0xE` | 500 | `0xC` | 78 | 94 | 212 | 36 |

验证结果：

- `ac1015_custom_class` 通过，`0xE` stage 变为 `custom_class_body_skip`。
- global bucket 中 `common_decode_fail` 从 193 降为 177。
- global bucket 中 `unsupported_type` 从 297 升为 313，代表样本包含
  `0xE(type=500,stage=custom_class_body_skip)` 与 `0xF(type=501,stage=custom_class_body_skip)`。
- AC1015 real baseline 仍恢复 84 entities，supported family failure buckets 仍全为 0。

下一轮建议转向 `*_CONTROL` 对象：它们现在仍是 `common_decode_fail`，前几项为
`BLOCK_CONTROL / LAYER_CONTROL / STYLE_CONTROL / LTYPE_CONTROL / VIEW_CONTROL`，
应从 entity common decoder 迁移到 table/object common 路线。
