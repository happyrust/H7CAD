# Native DWG STYLE Multi-Handle Boundary 计划（R58-DWG-STYLE-MULTI）

> 起稿：2026-04-26  
> 前置：R57-DWG-STYLE-PREFIX 已确认 `STYLE 0x11` 的 name boundary 为
> `offset=200 -> "Standard"`。

## 1. 目标

在进入生产修复前，扫描真实样本中所有 `type=53 STYLE` handles，确认是否只有一个
STYLE 记录，或是否多个 STYLE 记录共享可推广的 name-boundary 规则。

## 2. 执行批次

| 任务 | 优先级 | 预估 |
|---|---:|---:|
| T1 新增 STYLE handles scan，枚举所有 object type 53 | P0 | 0.3 h |
| T2 对每个 STYLE handle 扫描可打印 text candidates | P0 | 0.4 h |
| T3 判断是否可安全实现 conservative `read_text_style_name` | P1 | 0.3 h |

## 3. 不纳入

- 不改生产代码。
- 不实现 STYLE decoder。
- 不改变 diagnostics。

## 4. 验收

```bash
cargo test -p h7cad-native-dwg ac1015_style_multi_handle_boundary -- --nocapture
cargo test -p h7cad-native-dwg ac1015_style_prefix_field_probe -- --nocapture
```

通过标准：

- 输出所有 STYLE handles。
- 每个 STYLE handle 输出候选 name boundary。
- 计划记录是否可进入生产修复。

## 5. 状态

- [x] 计划定稿（2026-04-26）
- [x] T1 STYLE handles scan
- [x] T2 text candidate scan
- [x] T3 修复可行性判断

## 6. 结论

`sample_AC1015.dwg` 中存在 6 个 `type=53 STYLE` handles：
`0x11`, `0xDC`, `0x27F`, `0x399`, `0x409`, `0x887`。

候选文本边界不统一，不能把 R57 中 `0x11` 的 `offset=200 -> "Standard"` 规则
直接推广到所有 STYLE：

- `0x11`: `offset=200 -> "Standard"`
- `0xDC`: `offset=90 -> "\u{1e}AnnotativeDat"`，另在 `offset=308` 出现 Arial 相关片段
- `0x27F`: `offset=158 -> "ltypeshp.shx"`
- `0x399`: `offset=232 -> "custom_text_style"`
- `0x409`: `offset=208 -> "MyTextStyle"`
- `0x887`: `offset=158 -> "D:\\Albert DC\\Documents\\Visual Studio 2022\\Repos\\ACadSharp\\samples\\sample_base\\test_shape.shx"`

下一步不应实现固定 offset 的 STYLE name reader。更稳妥的 R59 是继续做
STYLE 字段结构探针：比较这 6 个 handles 的 prefix bytes、text candidates 与对象尾部，
先识别 name/font/big-font/file-name 等字段分界，再决定是否只做 conservative skip，还是实现
最小 STYLE decoder。
