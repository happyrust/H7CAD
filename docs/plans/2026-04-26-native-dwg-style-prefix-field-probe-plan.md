# Native DWG STYLE Prefix Field Probe 计划（R57-DWG-STYLE-PREFIX）

> 起稿：2026-04-26  
> 前置：R56-DWG-STYLE-OFFSET 已确认 `STYLE 0x11` 的 `offset=200`
> 可读出 `Standard`，而 body 起点 `58` 到 name 起点之间有 142 bits 前置数据。

## 1. 目标

把 `STYLE 0x11` 的前置 142 bits 固化成可审计 raw/field probe，为下一轮实现
STYLE-specific reader 做准备。

## 2. 执行批次

| 任务 | 优先级 | 预估 |
|---|---:|---:|
| T1 新增 prefix probe，导出 `58..200` 的 raw bytes / tail bits | P0 | 0.3 h |
| T2 记录 `offset=200` name read 结果与结束位置 | P0 | 0.2 h |
| T3 记录 `offset=90` 的 Arial-adjacent candidate | P1 | 0.2 h |
| T4 更新计划结论和下一轮实现假设 | P1 | 0.2 h |

## 3. 不纳入

- 不实现 `read_text_style_name` 修复。
- 不改变 diagnostics。
- 不解析 STYLE 完整 payload。

## 4. 验收

```bash
cargo test -p h7cad-native-dwg ac1015_style_prefix_field_probe -- --nocapture
cargo test -p h7cad-native-dwg ac1015_style_bit_offset_scan -- --nocapture
```

通过标准：

- prefix probe 稳定输出 `58..200` 的 raw bytes。
- `offset=200` 仍能读出 `Standard`。
- 计划记录下一轮最小 reader 假设。

## 5. 状态

- [x] 计划定稿（2026-04-26）
- [x] T1 prefix raw probe
- [x] T2 name boundary probe
- [x] T3 Arial-adjacent probe
- [x] T4 计划状态更新

## 6. 执行结论

新增 `ac1015_style_prefix_field_probe_reports_standard_boundary`。

Probe 输出：

- prefix range: `58..200`，共 142 bits。
- prefix bytes:
  `43 94 44 80 01 40 07 90 5C 9A 58 5B 11 C8 80 00 00`，tail 6 bits = `10`。
- `offset=200` 读出 `Standard`，结束于 bit `282`。
- `offset=90` 读出 `\\u{1e}Aria`，结束于 bit `148`。

下一轮最小实现假设：

- 对 `STYLE_OBJECT_TYPE` 先不要调用 generic `parse_ac1015_non_entity_common`。
- 先建立一个 conservative style-name reader，定位到已验证的 name boundary 后读取 `Standard`。
- 在实现前还需确认其他 STYLE handles 是否共享同类 boundary；如果不共享，只保留 probe，
  不进入生产路径。
