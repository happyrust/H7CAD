# Native DWG STYLE Bit Offset Scan 计划（R56-DWG-STYLE-OFFSET）

> 起稿：2026-04-26  
> 前置：R55-DWG-STYLE-PAYLOAD 已证明 direct text 能读到包含 `Arial` 的混合字符串，
> 但字段边界不对。

## 1. 目标

在 `STYLE 0x11` main stream 的局部 bit 范围内扫描 `read_text_ascii` 候选，
寻找能读出包含 `Arial` 的更合理 bit offset，为 STYLE-specific 字段顺序提供依据。

## 2. 执行批次

| 任务 | 优先级 | 预估 |
|---|---:|---:|
| T1 新增 bit-offset scan probe，枚举 `main_pos=58..220` 的 text 候选 | P0 | 0.4 h |
| T2 输出包含 `Arial` 或可打印 ASCII 的候选 offset | P0 | 0.3 h |
| T3 根据候选选择下一轮 STYLE 字段假设 | P1 | 0.3 h |

## 3. 不纳入

- 不实现 STYLE decoder。
- 不改变 `read_text_style_name`。
- 不改变 diagnostics stage。

## 4. 验收

```bash
cargo test -p h7cad-native-dwg ac1015_style_bit_offset_scan -- --nocapture
cargo test -p h7cad-native-dwg ac1015_style_payload_probe -- --nocapture
```

通过标准：

- probe 能稳定输出候选 offsets。
- 至少保留 direct-text baseline offset `58`。
- 计划记录下一轮最有希望的字段边界假设。

## 5. 状态

- [x] 计划定稿（2026-04-26）
- [x] T1 bit-offset scan probe
- [x] T2 候选 offset 输出
- [x] T3 下一轮字段假设

## 6. 执行结论

新增 `ac1015_style_bit_offset_scan_reports_arial_candidates`，在 `STYLE 0x11`
main stream 中扫描 text 候选。

关键候选：

| Offset | End | Text |
|---:|---:|---|
| 58 | 180 | mixed string，包含 `Arial` |
| 74 | 228 | mixed string，包含 `Arial` |
| 90 | 148 | `\\u{1e}Aria` |
| 200 | 282 | `Standard` |

结论：

- `offset=200` 可以读出干净的 `Standard`，很可能是 STYLE name 字段。
- `Arial` 出现在更早候选中，说明字体字段或 raw font bytes 位于 name 之前或附近，但
  当前 text 边界仍不对。
- 下一轮应围绕 `offset 58 -> 200` 之间的 142 bits 建立字段 probe，目标是解释
  style flags / font family / shape file / bigfont 等前置字段。
