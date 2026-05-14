# Native DWG DICTIONARY Skip Contract 计划（R50-DWG-DICT-SKIP）

> 起稿：2026-04-26  
> 前置：R49-DWG-DICT 已证明 `type=42 DICTIONARY` 可解析 non-entity common，
> 但 diagnostics 仍只显示 generic `body_dispatch`。

## 1. 目标

给 AC1015 `DICTIONARY` 建立最小对象路径契约：生产 diagnostics 在遇到 `type=42`
时先消费 non-entity common，再把剩余 body 作为未实现 dictionary payload 保守跳过。
本轮不解析 dictionary entries，只把 failure stage 从 generic `body_dispatch` 细化到
`object_body_skip`。

## 2. 设计

- 增加 `DICTIONARY_OBJECT_TYPE = 42`。
- 在 body dispatch 前识别 DICTIONARY，调用 `parse_ac1015_non_entity_common`。
- common 成功后仍返回 `UnsupportedType`，但 diagnostics stage 记录为 `object_body_skip`。
- common 失败时仍按 `CommonDecodeFail` 处理，避免把坏对象误标为已安全跳过。

## 3. 执行批次

| 任务 | 优先级 | 预估 |
|---|---:|---:|
| T1 更新 DICTIONARY focused test，期望 stage 为 `object_body_skip` | P0 | 0.2 h |
| T2 实现 DICTIONARY non-entity common skip contract | P0 | 0.5 h |
| T3 更新 global bucket representatives 输出/计划结论 | P1 | 0.2 h |
| T4 跑 focused tests 与 baseline | P1 | 0.3 h |

## 4. 不纳入

- 不从 `unsupported_type` 计数中移除 DICTIONARY。
- 不解析 dictionary entries / reactors / key-value pairs。
- 不处理 `CUSTOM_CLASS` 或 `*_CONTROL`。
- 不改变 recovered entity lower bound。

## 5. 验收

```bash
cargo test -p h7cad-native-dwg ac1015_dictionary -- --nocapture
cargo test -p h7cad-native-dwg ac1015_global_failure_bucket_representatives -- --nocapture
cargo test -p h7cad-native-dwg real_dwg_samples_baseline_m3b -- --nocapture
```

通过标准：

- `0x17/0x1A` 仍是 `unsupported_type`，但 stage 变为 `object_body_skip`。
- non-entity common probe 仍能解析 owner `0xC`。
- supported family failure buckets 继续为 0。

## 6. 状态

- [x] 计划定稿（2026-04-26）
- [x] T1 DICTIONARY stage RED test
- [x] T2 skip contract 实现
- [x] T3 输出/计划更新
- [x] T4 focused verification

## 7. 执行结论

本轮实现了最小 DICTIONARY object skip contract：

- `type=42 DICTIONARY` 进入 entity-body dispatch 前先消费 `parse_ac1015_non_entity_common`。
- common 成功后仍返回 `UnsupportedType`，但 diagnostics stage 改为 `object_body_skip`。
- common 失败时仍会保留 `CommonDecodeFail`，避免把坏对象误标成安全跳过。

验证结果：

- `0x17/0x1A` focused test 通过，owner 仍为 `0xC`，stage 变为 `object_body_skip`。
- global bucket 中 `common_decode_fail` 从 236 降为 193。
- global bucket 中 `unsupported_type` 从 254 升为 297，其中 DICTIONARY 代表现在显示
  `stage=object_body_skip`。
- AC1015 real baseline 仍恢复 84 entities，supported family failure buckets 仍全为 0。

下一轮可继续对 `CUSTOM_CLASS(type=500)` 建立类似的 class/object metadata skip contract，
或转向 `*_CONTROL` 对象，把它们从 entity common decoder 迁移到 table/object common 路线。
