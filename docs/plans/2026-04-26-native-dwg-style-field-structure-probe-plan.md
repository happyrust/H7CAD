# Native DWG STYLE Field Structure Probe 计划（R59-DWG-STYLE-FIELDS）

> 起稿：2026-04-26  
> 前置：R58-DWG-STYLE-MULTI 已确认 `sample_AC1015.dwg` 中有 6 个
> `type=53 STYLE` handles，且 text/name boundary 不统一。

## 1. 目标

在进入生产修复前，横向比较所有 STYLE 对象的字段结构线索：body 起点、前缀字节、
候选文本边界、候选文本后的剩余 bits。目标是判断下一步应实现 conservative skip，
还是可以安全实现最小 STYLE decoder。

## 2. 执行批次

| 任务 | 优先级 | 预估 |
|---|---:|---:|
| T1 新增 STYLE field structure probe | P0 | 0.4 h |
| T2 打印每个 STYLE 的 prefix bytes 与 text candidates | P0 | 0.4 h |
| T3 对已知样本名做 conservative assertions | P1 | 0.2 h |
| T4 根据输出判断下一步生产策略 | P1 | 0.2 h |

## 3. 不纳入

- 不改生产代码。
- 不实现 STYLE decoder。
- 不改 diagnostics bucket。

## 4. 验收

```bash
cargo test -p h7cad-native-dwg ac1015_style_field_structure_probe -- --nocapture
```

通过标准：

- 输出所有 6 个 STYLE handles。
- 输出每个 STYLE 的 body start、main bits、prefix bytes、text candidates。
- 至少确认 `Standard`、`custom_text_style`、`MyTextStyle` 三个样本名仍可被定位。

## 5. 状态

- [x] 计划定稿（2026-04-26）
- [x] T1 STYLE field structure probe
- [x] T2 prefix/text/tail 输出
- [x] T3 conservative assertions
- [x] T4 生产策略判断

## 6. 结论

R59 probe 确认 `STYLE` 不是单一固定边界结构：

- `0x11`: name `Standard` at `offset=200`，tail font `arial.ttf` at `offset=368`
- `0xDC`: embedded/font fragment at `offset=308`，style-like name `Annotative` at `offset=450`
- `0x27F`: only clear text `ltypeshp.shx` at `offset=158`
- `0x399`: name `custom_text_style` at `offset=232`，tail font `consola.ttf` at `offset=472`
- `0x409`: name `MyTextStyle` at `offset=208`，tail font `arial.ttf` at `offset=400`
- `0x887`: clear text is a shape file path ending `test_shape.shx` at `offset=158`

生产策略判断：

- 目前不实现 STYLE name reader；规则不足以区分 name/font/shape path，强行读取会把
  SHX 文件名误当 style name。
- 下一步可以做更保守的 `STYLE` skip contract：对 object type `53` 直接归入
  `UnsupportedType/style_record_skip`，避免继续把 STYLE 归为 `CommonDecodeFail`。
- 若未来要真正解码 STYLE，需要先引入字段级格式依据，而不是从 text scan 反推固定 offset。
