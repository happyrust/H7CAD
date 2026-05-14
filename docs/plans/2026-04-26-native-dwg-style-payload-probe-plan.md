# Native DWG STYLE Payload Probe 计划（R55-DWG-STYLE-PAYLOAD）

> 起稿：2026-04-26  
> 前置：R54-DWG-STYLE 已证明 `STYLE 0x11` 的 EOF 发生在 main stream，
> handle stream 没推进。

## 1. 目标

对 `STYLE 0x11` 做 payload-specific 入口探测：比较直接读取 text、套用
non-entity common 后读取 text，以及原始 reader 位置变化，确定 STYLE record 的下一步
字段假设。

## 2. 当前事实

- `read_text_style_name` 当前等价于：`parse_ac1015_non_entity_common` 后
  `read_text_ascii`。
- `STYLE 0x11` 在 common 阶段已经 EOF，因此无法到达 style name。
- `LAYER/LTYPE/APPID` 的 table record 形状不能直接推广到 STYLE。

## 3. 执行批次

| 任务 | 优先级 | 预估 |
|---|---:|---:|
| T1 新增 payload probe，记录 STYLE 0x11 direct `read_text_ascii` 结果 | P0 | 0.3 h |
| T2 记录 common-then-text 结果与 reader 消耗 | P0 | 0.3 h |
| T3 根据输出选择下一轮字段假设：direct text、前置 flags、或特殊 common | P1 | 0.3 h |
| T4 更新计划结论 | P1 | 0.2 h |

## 4. 不纳入

- 不实现 STYLE decoder。
- 不改变生产 diagnostics stage。
- 不改变 symbol name maps。

## 5. 验收

```bash
cargo test -p h7cad-native-dwg ac1015_style_payload_probe -- --nocapture
cargo test -p h7cad-native-dwg ac1015_style_common_eof -- --nocapture
```

通过标准：

- probe 输出 direct text 与 common-then-text 两条路径。
- common EOF 仍稳定复现。
- 计划文件记录下一轮具体字段假设。

## 6. 状态

- [x] 计划定稿（2026-04-26）
- [x] T1 direct text probe
- [x] T2 common-then-text probe
- [x] T3 下一轮字段假设
- [x] T4 计划状态更新

## 7. 执行结论

新增 `ac1015_style_payload_probe_reports_direct_text_attempts`。

Probe 结果：

- direct `read_text_ascii` 从 `main_pos=58` 开始可以读出字符串，但结果不是干净
  style name：包含控制字节和 `Arial` 片段，消耗到 `main_pos=180`。
- common-then-text 仍在 common 阶段 EOF：`main_pos=58 -> 460`，`main_remaining=402 -> 0`，
  handle stream 不推进。

这说明 `STYLE 0x11` 的 payload 很可能是：

- 先有 STYLE-specific flags / numeric fields，再有字体名等 text；
- 或 text 不是从当前 body 起点直接开始；
- 不能继续复用 `parse_ac1015_non_entity_common + read_text_ascii`。

下一轮建议建立 STYLE-specific field probe，逐步尝试：

1. 读取候选 flags / xref bits。
2. 搜索或定位 `Arial` 前的字段边界。
3. 再决定是否实现最小 `read_text_style_name` 修复。
