# Native DWG Table Record Skip Contract 计划（R53-DWG-TABLE）

> 起稿：2026-04-26  
> 前置：R52-DWG-CONTROL 已把 control objects 推进到 `control_object_skip`；
> global `common_decode_fail` 的前几项变为 table record objects。

## 1. 目标

为 AC1015 table record objects 建立最小 skip contract：遇到 `LAYER`、`STYLE`、
`APPID`、`LTYPE` 等表记录对象时，先消费 non-entity common，然后保守返回
`UnsupportedType`，diagnostics stage 记录为 `table_record_skip`。

## 2. 当前候选

来自 `ac1015_global_failure_bucket_representatives`：

| Handle | Object type | Current kind | Stage |
|---|---:|---|---|
| `0x10` | `51:LAYER` | `common_decode_fail` | `common_entity_decode` |
| `0x11` | `53:STYLE` | `common_decode_fail` | `common_entity_decode` |
| `0x12` | `67:APPID` | `common_decode_fail` | `common_entity_decode` |
| `0x14` | `57:LTYPE` | `common_decode_fail` | `common_entity_decode` |
| `0x15` | `57:LTYPE` | `common_decode_fail` | `common_entity_decode` |

## 3. 执行批次

| 任务 | 优先级 | 预估 |
|---|---:|---:|
| T1 新增 focused test，锁定 table record object 的 type 与 non-entity common probe | P0 | 0.4 h |
| T2 RED 断言这些对象 stage 应为 `table_record_skip` | P0 | 0.2 h |
| T3 在生产 diagnostics 路径接入 table record skip contract | P0 | 0.5 h |
| T4 跑 focused tests 与 AC1015 baseline | P1 | 0.3 h |

## 4. 不纳入

- 不解析 table record 的名称 / flags / payload。
- 不替代 `collect_symbol_name_maps` 的专用轻量解析。
- 不改变 recovered entity lower bound。

## 5. 验收

```bash
cargo test -p h7cad-native-dwg ac1015_table_record -- --nocapture
cargo test -p h7cad-native-dwg ac1015_global_failure_bucket_representatives -- --nocapture
cargo test -p h7cad-native-dwg real_dwg_samples_baseline_m3b -- --nocapture
```

通过标准：

- 前五个 table record objects 从 `common_decode_fail` 推进到
  `unsupported_type/table_record_skip`。
- supported family failure buckets 继续为 0。
- AC1015 real baseline 仍恢复 84 entities。

## 6. 状态

- [x] 计划定稿（2026-04-26）
- [x] T1 focused table record test
- [x] T2 RED stage assertion
- [x] T3 skip contract 实现
- [x] T4 focused verification

## 7. 执行结论

本轮实现了 table record object 的最小 skip contract，但保留了异构记录差异：

- `LAYER / APPID / LTYPE` 等可通过 `parse_ac1015_non_entity_common` 的 table records，
  现在会推进到 `UnsupportedType`，stage 为 `table_record_skip`。
- `STYLE 0x11` 当前 non-entity common probe 仍 EOF，因此保留为
  `CommonDecodeFail / common_entity_decode`，不强行标成安全 skip。

Focused probe 结果：

| Handle | Type | Probe result |
|---|---:|---|
| `0x10` | 51 `LAYER` | ok，owner `0x2` |
| `0x11` | 53 `STYLE` | `UnexpectedEof { context: "bit" }` |
| `0x12` | 67 `APPID` | ok，owner `0x9` |
| `0x14` | 57 `LTYPE` | ok，owner `0x5` |
| `0x15` | 57 `LTYPE` | ok，owner `0x5` |

验证结果：

- `ac1015_table_record` 通过。
- global bucket 中 `common_decode_fail` 从 168 降为 106。
- global bucket 中 `unsupported_type` 从 322 升为 384。
- AC1015 real baseline 仍恢复 84 entities，supported family failure buckets 仍全为 0。

下一轮建议聚焦 `STYLE 0x11` 的 non-entity common EOF，或转向剩余 custom class
`type=502/505` 这类仍停在 `common_entity_decode` 的对象。
