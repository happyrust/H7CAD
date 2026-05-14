# Native DWG STYLE Common EOF 计划（R54-DWG-STYLE）

> 起稿：2026-04-26  
> 前置：R53-DWG-TABLE 已让可解析 table records 进入 `table_record_skip`；
> `STYLE 0x11` 仍停在 `CommonDecodeFail / common_entity_decode`。

## 1. 目标

把 `STYLE 0x11` 的 non-entity common EOF 固化成可审计的字段级诊断，避免继续把所有
table records 当作同一 common 形状处理。

## 2. 当前事实

- `LAYER 0x10` 可解析 non-entity common，owner=`0x2`。
- `APPID 0x12` 可解析 non-entity common，owner=`0x9`。
- `LTYPE 0x14/0x15` 可解析 non-entity common，owner=`0x5`。
- `STYLE 0x11` 在 `parse_ac1015_non_entity_common` 中 EOF，仍保留
  `CommonDecodeFail / common_entity_decode`。

## 3. 执行批次

| 任务 | 优先级 | 预估 |
|---|---:|---:|
| T1 新增 STYLE focused probe，记录 header、main/handle 起点、EOF 前位置 | P0 | 0.4 h |
| T2 与成功的 LAYER 0x10 做对照，确认是否是 handle stream 不足或 main stream 字段差异 | P0 | 0.4 h |
| T3 保持生产行为不变：STYLE 继续 `CommonDecodeFail`，直到字段形状明确 | P1 | 0.1 h |
| T4 更新计划结论和下一轮候选 | P1 | 0.2 h |

## 4. 不纳入

- 不强行把 STYLE 标成 `table_record_skip`。
- 不解析 STYLE name / font / flags。
- 不改变 recovered entity lower bound。

## 5. 验收

```bash
cargo test -p h7cad-native-dwg ac1015_style_common_eof -- --nocapture
cargo test -p h7cad-native-dwg ac1015_table_record -- --nocapture
```

通过标准：

- focused probe 稳定复现 `STYLE 0x11` 的 EOF。
- 测试输出足够定位 EOF 发生时的 reader 位置。
- `ac1015_table_record` 仍通过，确认 R53 contract 没有回退。

## 6. 状态

- [x] 计划定稿（2026-04-26）
- [x] T1 STYLE focused probe
- [x] T2 LAYER 对照
- [x] T3 保持生产行为
- [x] T4 计划状态更新

## 7. 执行结论

新增 `ac1015_style_common_eof_probe_reports_reader_positions`，本轮只诊断、不改生产路径。

对照结果：

| Handle | Type | Result | Main before | Main after | Handle before | Handle after |
|---|---:|---|---:|---:|---:|---:|
| `0x10` | 51 `LAYER` | ok owner=`0x2` | pos 58 / rem 62 | pos 62 / rem 58 | pos 120 / rem 72 | pos 152 / rem 40 |
| `0x11` | 53 `STYLE` | `UnexpectedEof { context: "bit" }` | pos 58 / rem 402 | pos 460 / rem 0 | pos 460 / rem 36 | pos 460 / rem 36 |

结论：

- `STYLE 0x11` 的 EOF 发生在 main stream，被读到末尾；handle stream 没有推进。
- 这不像普通 non-entity common 的 owner/reactor/xdictionary 形状。
- 生产行为保持不变：`STYLE 0x11` 继续 `CommonDecodeFail / common_entity_decode`。

下一轮应针对 STYLE record payload 建立专用 probe，而不是继续扩大 generic table skip。
