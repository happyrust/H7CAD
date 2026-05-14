# ColorPolicy 三态 + SVG 导出对话框补齐（四十轮）

> **起稿**：2026-04-25（第四十轮）
> **前置**：R39（三十九轮）Gradient HATCH 的 `linearGradient` + PDF strip-fill 落地，
> 但 **P1 T5**（GUI 对话框接入 `gradient_hatches`）明确延到下一轮；与此同时
> R37/R38 把 `SvgExportOptions` 扩到 16 字段，而 GUI 对话框只暴露 9 bool + 3
> numeric，外加 R38 的 `--options JSON` 让 CLI 自由度远超 GUI。
> **目标**：（1）把 ODA 的 `ColorPolicy` **三态**（Full / Monochrome / Grayscale）
> 完整映射进 H7CAD，关掉 `bool monochrome` ↔ ODA enum 的语义鸿沟；
> （2）把对话框剩余 4 个字段（`gradient_hatches` / `font_family` /
> `image_url_prefix` / `image_base`）全部对接，让 GUI 能力与 CLI `--options`
> JSON 完全对齐；（3）**零破坏 JSON 向后兼容**：所有 pre-R40 的 `--options`
> 文件必须一字节不变地继续工作。

---

## 1. 现状

### 1.1 ODA 参考实现（OdSvgExportEx.cpp, L436）

```cpp
// "ColorPolicy" - 0 = Colors are unchanged, 1 = Drawing is converted to
// monochrome, 2 = Drawing is converted to grayscale
dev->properties()->putAt("ColorPolicy",
    OdRxVariantValue((OdInt32)(useMonochrome ? 1 : 0)));
```

ODA CLI 只暴露 `--keep-colors` / `--monochrome` 两种（映射 `ColorPolicy=0` /
`ColorPolicy=1`），`ColorPolicy=2`（灰度）官方示例从未驱动过——但 device
API 本身就支持。

### 1.2 H7CAD R39 现状

```rust
pub struct SvgExportOptions {
    pub monochrome: bool,  // 仅 bool — 缺 Grayscale 档位
    // ...
}
```

GUI 对话框 `src/ui/svg_export_dialog.rs` 暴露：
- `Monochrome` toggle（bool）
- `TextAsGeometry`, `IncludeHatches`, `UseBlockDefs`, `IncludeImages`,
  `EmbedImages`, `NativeCurves`, `NativeSplines`, `NativeDimensionText`
- 3 个 numeric（font size scale / min stroke width / lineweight scale）

**缺失**：
- `color_policy` 三态（Full / Mono / Gray）
- `gradient_hatches`（R39 P1 遗留）
- `font_family`（字符串）
- `image_url_prefix`（字符串）
- `image_base`（`Option<PathBuf>`，需异步目录选择器）

### 1.3 约束

`src/cli.rs` 的 JSON 反序列化用 `#[derive(serde::Deserialize)]` +
`#[serde(default)]` 覆盖 `SvgExportOptions`——**任何 pre-R40 JSON 必须继续
parse 成功**。R38 的三条集成测试（`cli_options_json_produces_valid_pdf` /
`cli_options_missing_file_exits_one` / `cli_options_malformed_json_exits_one`）
是硬性回归屏障。

---

## 2. 范围

| 任务 | 优先级 | 预估 |
|------|-------|------|
| T1 `ColorPolicy` enum + `SvgExportOptions` 新增 `color_policy` 字段（保留 legacy `monochrome` bool）+ `effective_color_policy()` + `apply_color_policy()` helper | P0 | 0.5 h |
| T2 `emit_wires` + `emit_hatches` + `emit_gradient_hatch_svg` + `resolve_entity_stroke` + `resolve_entity_fill` 改走 helper，新增 Grayscale 分支（BT.601 luminance） | P0 | 0.4 h |
| T3 `SvgExportDialogField` 增加 `ColorPolicyFull` / `ColorPolicyMono` / `ColorPolicyGray` / `GradientHatches` / `FontFamily` / `ImageUrlPrefix` / `ImageBasePick` / `ImageBaseClear` | P0 | 0.3 h |
| T4 `apply_toggle` + `view_window` 对接新字段；`image_base` 走 rfd 异步目录选择器 + 文本显示路径；app state 加 `svg_export_font_family_buf` / `svg_export_image_url_prefix_buf` + 新 `Message` variant `SvgExportDialogImageBasePicked` | P0 | 0.5 h |
| T5 单元测试：`effective_color_policy` 5 组合 + `apply_color_policy` 三模式 + 端到端 Full/Mono/Gray fill 采样 + legacy/new JSON 反序列化 | P0 | 0.5 h |
| T6 plan 定稿 + CHANGELOG + `docs/svg_export.md` 字段表扩充 | P0 | 0.3 h |

**不纳入**：
- ODA 三种坐标模式（`SVG_WORLD_COORDINATES` / `SVG_WHOLE_DRAWING`）留到 R41/R42
- Paper Space Layout 切换 + 多视口合成 — R42
- OLE 对象 + Rational NURBS + SVG `<pattern>` 真实 hatch — R43

---

## 3. 设计

### 3.1 `ColorPolicy` enum + 双字段策略

```rust
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ColorPolicy {
    Full,
    #[default]
    Monochrome,
    Grayscale,
}
```

`SvgExportOptions` **双字段并存**：
- `color_policy: ColorPolicy`（新，主字段）
- `monochrome: bool`（保留，legacy — 仅用于旧 JSON 反序列化 + 旧调用代码）

**优先级契约**（由 `effective_color_policy(opts)` 统一裁决）：

| `color_policy` | `monochrome` | effective |
|----------------|--------------|-----------|
| `Full` | _不关心_ | `Full` |
| `Grayscale` | _不关心_ | `Grayscale` |
| `Monochrome` (default) | `true` | `Monochrome` |
| `Monochrome` (default) | `false` | **`Full`** |

含义：`color_policy` 只要不是默认值（Monochrome），新字段就胜出；否则按
legacy bool 推断。这样所有 8 种组合都有确定语义，并且：

- 旧 JSON `{"monochrome": false}` → effective `Full` ✓ 与 R39 语义一致
- 旧 JSON `{"monochrome": true}` → effective `Monochrome` ✓ 与 R39 语义一致
- 新 JSON `{"color_policy": "grayscale"}` → effective `Grayscale` ✓ R40 新能力
- 混合 JSON `{"color_policy": "full", "monochrome": true}` → effective `Full` ✓
  （新字段明确表态，不理会 legacy bool）

### 3.2 Grayscale 映射算法（BT.601）

```rust
fn apply_color_policy(p: ColorPolicy, r: f32, g: f32, b: f32) -> (f32, f32, f32) {
    match p {
        ColorPolicy::Full => (r, g, b),
        ColorPolicy::Monochrome => (0.0, 0.0, 0.0),
        ColorPolicy::Grayscale => {
            let y = 0.299 * r + 0.587 * g + 0.114 * b;
            (y, y, y)
        }
    }
}
```

选 BT.601 而非 BT.709 的原因：工程图纸主要打印黑白硬拷贝，BT.601 对纯色
的亮度排序（红 76 / 绿 150 / 蓝 29）与旧 AutoCAD / PlotStyle CTB 的灰度
映射更接近，视觉上"R39 单色"→"R40 灰度"切换是光滑渐变而非突变。

### 3.3 Gradient HATCH 三态渲染

```rust
let (stop0, stop1) = match effective_color_policy(options) {
    ColorPolicy::Monochrome => {
        // 保持梯度方向可辨识：pure black → light gray，不塌陷两端
        ("rgb(0,0,0)", "rgb(200,200,200)")
    }
    policy => {
        let (r0, g0, b0) = apply_color_policy(policy, hatch.color[..3]);
        let (r1, g1, b1) = apply_color_policy(policy, color2[..3]);
        (...)
    }
};
```

Monochrome 的"黑→浅灰"兜底是 R39 既有行为，R40 保留；Full 是原色；
Grayscale 对两 stop 各走 luminance，得到同方向但灰度压缩的 ramp。

### 3.4 GUI 对话框三态 pill

布局：

```
Color & Strokes
  Color policy    [● Full ] [  Mono ] [  Gray ]   ← 三选一 pill
  Min stroke width (mm)    [0.1    ]
  Line-weight scale        [0.2646 ]
Text
  [ ] Tessellate text
  [ ] Native dimension text
  Font size scale          [0.8    ]
  Font family              [SimSun, 宋体        ]   ← R40 new
Geometry
  [x] Include hatch fills
  [x] Gradient hatches (linearGradient)             ← R40 new (R39 P1 遗留)
  [x] Deduplicate blocks via <defs>
  [x] Emit native curves
  [x] Emit native splines
Images
  [x] Include raster images
  [x] Embed images as base64 data URI
  Image URL prefix         [./                  ]   ← R40 new
  Image base dir           (not set)  [Browse…] [Clear]   ← R40 new
```

pill active 状态由 `effective_color_policy(opts)` 决定，保证切换瞬间
UI 立即反映最终行为。

`image_base` 通过 **`Message::SvgExportDialogImageBasePicked`** 回调接
rfd 异步目录选择结果（复用 `io::pick_workspace_folder()`）。Clear 按钮走
同步路径直接清 `Option<PathBuf>` 为 `None`。

### 3.5 `apply_toggle` 分发

同步分支处理所有 bool / pill / clear；异步 `ImageBasePick` 在 `app::update`
截获后派发 `Task::perform(pick_workspace_folder, SvgExportDialogImageBasePicked)`。
文本输入 `FontFamily` / `ImageUrlPrefix` 是 edit 路径而非 toggle 路径——
`apply_toggle` 对这两项是 no-op，由 `Message::SvgExportDialogEdit` 走
string buffer，`Commit` 时 flush 到 `opts`。

---

## 4. 测试

### 4.1 单元测试（`src/io/svg_export.rs`）

- `effective_color_policy_defaults_to_monochrome`
- `effective_color_policy_new_field_wins_when_non_default`（Full / Grayscale 两情况）
- `effective_color_policy_falls_back_to_legacy_monochrome_when_unset`（true/false 两情况）
- `effective_color_policy_conflict_new_field_overrides_legacy`
- `apply_color_policy_full_is_identity`
- `apply_color_policy_monochrome_collapses_to_black`
- `apply_color_policy_grayscale_uses_bt601_luminance`（纯红 76 / 纯绿 150 / 纯蓝 29 / 白 255 / 黑 0）
- `end_to_end_full_mode_emits_source_hatch_color`（`<polygon fill="rgb(255,0,0)"`）
- `end_to_end_grayscale_mode_maps_red_to_bt601_luminance`（`<polygon fill="rgb(76,76,76)"`）
- `end_to_end_monochrome_mode_forces_black_fill`
- `gradient_stops_in_grayscale_become_luminance_ramp`（red→blue stops 变 rgb(76,...) / rgb(29,...)）
- `legacy_json_monochrome_false_deserialises_to_full_effective_policy`
- `legacy_json_monochrome_true_stays_monochrome`
- `new_json_color_policy_grayscale_sets_enum`
- `new_json_color_policy_full_overrides_legacy_monochrome_true`
- `new_json_all_three_color_policy_variants_roundtrip`

### 4.2 对话框测试（`src/ui/svg_export_dialog.rs`）

既有三条保留：
- `toggle_flips_each_boolean_field` — 扩展 10 → 10 + `GradientHatches`
- `numeric_fields_do_not_flip_booleans` — 扩展 3 → 3 + `FontFamily` / `ImageUrlPrefix`
- `double_toggle_returns_to_original`

R40 新增：
- `color_policy_pills_select_exactly_one_mode_at_a_time` — 三态互斥
- `legacy_monochrome_toggle_keeps_color_policy_in_sync` — 遗留 bool 切换后 enum 必须跟随
- `image_base_clear_resets_field_to_none`
- `image_base_pick_is_a_noop_in_apply_toggle`（保证异步只走 update 分支）

### 4.3 CLI / 集成

不新增——既有 R38 的 `cli_options_json_produces_valid_pdf` / `cli_options_malformed_json_exits_one` 作为 JSON parse 契约的端到端 canary。

---

## 5. 验收

```bash
cargo check -p H7CAD                      # 零新 warning
cargo test --bin H7CAD io::svg_export     # 72 → 88 全绿
cargo test --bin H7CAD ui::svg_export_dialog  # 3 → 7 全绿
cargo test --bin H7CAD                    # 426 → 450 全绿
cargo test --test cli_batch_export        # 10 / 10 不变
```

手动：
- 启动 H7CAD，`SVGEXPORTDIALOG` 打开对话框，三态 pill 切换，Font family
  填 `"Arial"`，Image base 用 Browse 选个目录，点 Export…
  产出的 SVG 应反映这些设置
- CLI：`echo '{"color_policy":"grayscale"}' > gray.json` →
  `h7cad.exe drawing.dxf --export-svg out.svg --options gray.json` →
  SVG 颜色应走灰度 ramp

---

## 6. 状态

- [x] 计划定稿（2026-04-25）
- [x] T1 `ColorPolicy` enum + `effective_color_policy` + `apply_color_policy`
- [x] T2 `emit_*` 全量改走 helper；Grayscale BT.601 分支
- [x] T3 `SvgExportDialogField` 扩展 8 新 variant
- [x] T4 GUI 对话框接入全部字段 + rfd 异步目录选择
- [x] T5 单元测试 bin 426 → 450 全绿，CLI 10 / 10 不变
- [x] T6 plan + CHANGELOG + docs/svg_export.md

---

## 7. 下轮方向（R41 预告）

- **R41**：Layer 分组 `<g id="layer_LNAME">` + SVGZ 压缩（.svgz 输出）
- **R42**：Paper Space Layout + 多视口合成（`setupActiveLayoutViews` parity）
- **R43**：OLE 对象 + Rational NURBS + SVG `<pattern>` 真实 hatch
