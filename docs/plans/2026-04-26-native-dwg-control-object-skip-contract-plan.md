# Native DWG CONTROL Object Skip Contract 计划（R52-DWG-CONTROL）

> 起稿：2026-04-26  
> 前置：R51-DWG-CUSTOM 已把 custom class objects 推进到
> `custom_class_body_skip`；global `common_decode_fail` 仍以 `*_CONTROL` 对象开头。

## 1. 目标

为 AC1015 table/control objects 建立最小 skip contract：遇到 `BLOCK_CONTROL`、
`LAYER_CONTROL`、`STYLE_CONTROL`、`LTYPE_CONTROL`、`VIEW_CONTROL` 等控制对象时，
先消费 non-entity common，然后保守返回 `UnsupportedType`，diagnostics stage 记录为
`control_object_skip`。

## 2. 当前候选

来自 `ac1015_global_failure_bucket_representatives`：

| Handle | Object type | Current kind | Stage |
|---|---:|---|---|
| `0x1` | `48:BLOCK_CONTROL` | `common_decode_fail` | `common_entity_decode` |
| `0x2` | `50:LAYER_CONTROL` | `common_decode_fail` | `common_entity_decode` |
| `0x3` | `52:STYLE_CONTROL` | `common_decode_fail` | `common_entity_decode` |
| `0x5` | `56:LTYPE_CONTROL` | `common_decode_fail` | `common_entity_decode` |
| `0x6` | `60:VIEW_CONTROL` | `common_decode_fail` | `common_entity_decode` |

## 3. 执行批次

| 任务 | 优先级 | 预估 |
|---|---:|---:|
| T1 新增 focused test，锁定前五个 control object 的 type 与 non-entity common probe | P0 | 0.4 h |
| T2 RED 断言这些对象 stage 应为 `control_object_skip` | P0 | 0.2 h |
| T3 在生产 diagnostics 路径接入 control object skip contract | P0 | 0.5 h |
| T4 跑 focused tests 与 AC1015 baseline | P1 | 0.3 h |

## 4. 不纳入

- 不解析 control object 的 owned entries。
- 不解析 table records。
- 不改变 recovered entity lower bound。
- 不处理 remaining unknown object types。

## 5. 验收

```bash
cargo test -p h7cad-native-dwg ac1015_control_object -- --nocapture
cargo test -p h7cad-native-dwg ac1015_global_failure_bucket_representatives -- --nocapture
cargo test -p h7cad-native-dwg real_dwg_samples_baseline_m3b -- --nocapture
```

通过标准：

- 前五个 control objects 从 `common_decode_fail` 推进到 `unsupported_type/control_object_skip`。
- supported family failure buckets 继续为 0。
- AC1015 real baseline 仍恢复 84 entities。

## 6. 状态

- [x] 计划定稿（2026-04-26）
- [x] T1 focused control object test
- [x] T2 RED stage assertion
- [x] T3 skip contract 实现
- [x] T4 focused verification

## 7. 执行结论

本轮实现了 control object 的最小 skip contract：

- `BLOCK_CONTROL / LAYER_CONTROL / STYLE_CONTROL / LTYPE_CONTROL / VIEW_CONTROL`
  等 control object 进入 entity-body dispatch 前先消费 `parse_ac1015_non_entity_common`。
- common 成功后仍返回 `UnsupportedType`，但 diagnostics stage 改为 `control_object_skip`。
- common 失败仍保留 `CommonDecodeFail`。

Focused probe 确认：

| Handle | Type | Owner | Main position | Main remaining | Handle position | Handle remaining |
|---|---:|---:|---:|---:|---:|---:|
| `0x1` | 48 | `0x0` | 62 | 10 | 88 | 632 |
| `0x2` | 50 | `0x0` | 62 | 10 | 104 | 496 |
| `0x3` | 52 | `0x0` | 62 | 10 | 88 | 128 |
| `0x5` | 56 | `0x0` | 62 | 10 | 88 | 176 |
| `0x6` | 60 | `0x0` | 62 | 10 | 88 | 112 |

验证结果：

- `ac1015_control_object` 通过。
- global bucket 中 `common_decode_fail` 从 177 降为 168。
- global bucket 中 `unsupported_type` 从 313 升为 322，前五个代表现在都是
  `stage=control_object_skip` 的 control objects。
- AC1015 real baseline 仍恢复 84 entities，supported family failure buckets 仍全为 0。

下一轮建议处理 table record objects：global `common_decode_fail` 的前几项已经变为
`LAYER / STYLE / APPID / LTYPE`，它们应沿用 object/table common 路线，而不是 entity common。
