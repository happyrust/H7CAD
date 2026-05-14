# Native DWG DICTIONARY Object 计划（R49-DWG-DICT）

> 起稿：2026-04-26  
> 前置：R48-DWG-BUCKETS 已确认剩余 failures 不再是 supported geometry；
> `unsupported_type` 的代表样本包含 `type=42 DICTIONARY`。

## 1. 目标

把 AC1015 `DICTIONARY` 从 “unsupported entity body dispatch” 转成明确的非实体对象候选，
先验证它的 object header 和 non-entity common 是否能稳定读取，再决定是否新增最小对象
decoder 或显式 skip contract。

## 2. 当前候选

来自 `ac1015_global_failure_bucket_representatives`：

| Handle | Object type | Current kind | Stage |
|---|---:|---|---|
| `0x17` | `42:DICTIONARY` | `unsupported_type` | `body_dispatch` |
| `0x1A` | `42:DICTIONARY` | `unsupported_type` | `body_dispatch` |

## 3. 执行批次

| 任务 | 优先级 | 预估 |
|---|---:|---:|
| T1 新增 focused test，锁定 `0x17/0x1A` 的 header object type 为 `42` | P0 | 0.2 h |
| T2 在 focused test 中尝试 `parse_ac1015_non_entity_common`，验证 DICTIONARY 是否可以越过 common object preamble | P0 | 0.4 h |
| T3 若 non-entity common 可读，记录 owner/remaining bits，为下一轮最小 dictionary decoder 提供入口 | P1 | 0.3 h |
| T4 跑 global bucket baseline，确认本轮没有破坏 diagnostics 清单 | P1 | 0.2 h |

## 4. 不纳入

- 不实现完整 dictionary entries 解析。
- 不提升 recovered entity lower bound。
- 不把 DICTIONARY 塞进 supported entity family。
- 不处理 `CUSTOM_CLASS`，它另起一轮。

## 5. 验收

```bash
cargo test -p h7cad-native-dwg ac1015_dictionary -- --nocapture
cargo test -p h7cad-native-dwg ac1015_global_failure_bucket_representatives -- --nocapture
```

通过标准：

- focused test 能稳定定位 `0x17/0x1A` 为 `type=42 DICTIONARY`。
- 如果 non-entity common 可读，测试输出 owner 和剩余 bit 位置。
- diagnostics 仍把 DICTIONARY 视为 unsupported object candidate，而不是 supported-family failure。

## 6. 状态

- [x] 计划定稿（2026-04-26）
- [x] T1 DICTIONARY header focused test
- [x] T2 non-entity common probe
- [x] T3 记录 owner / remaining bits
- [x] T4 focused verification

## 7. 执行结论

新增 `ac1015_dictionary_objects_parse_non_entity_common_preamble`，确认：

- `0x17` 与 `0x1A` 的 object header 都是 `type=42 DICTIONARY`。
- 两者都能通过 `parse_ac1015_non_entity_common`。
- 两者 owner 都解析为 `0xC`。
- diagnostics 仍将它们记录为 `unsupported_type` / `body_dispatch`，且 `family=None`。

当前 probe 输出：

| Handle | Owner | Main position | Main remaining | Handle position | Handle remaining |
|---|---:|---:|---:|---:|---:|
| `0x17` | `0xC` | 70 | 110 | 220 | 20 |
| `0x1A` | `0xC` | 70 | 316 | 426 | 78 |

下一轮可以做最小 DICTIONARY decoder / skip contract：先消费 non-entity common，再按保守方式
跳过剩余 object body，同时把 diagnostics stage 从 generic `body_dispatch` 改成更明确的
`dictionary_body_decode` 或 `object_body_skip`。
