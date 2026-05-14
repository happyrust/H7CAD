# PDF ColorPolicy 三态镜像 + GUI 对话框三态 pill（R41-PDF）

> **起稿**：2026-04-25（第四十一轮 · PDF 镜像支线）
> **前置**：R40（四十轮）`ColorPolicy { Full, Monochrome, Grayscale }` 已经完整
> 落地到 SVG 端（`src/io/svg_export.rs` L48-L59 + L268-L297），GUI 对话框三态 pill
> 也已上线（`src/ui/svg_export_dialog.rs`）。R40 收尾时显式登记：
> > "PDF 对话框镜像三态（PDF 当前仍然 `monochrome: bool`）— **下轮对称扩展**"
>
> **目标**：把 SVG 已经做完的 `ColorPolicy` 三态完整镜像到 PDF 端，关掉
> `bool monochrome` ↔ ODA `ColorPolicy=0/1/2` 的语义鸿沟；GUI 对话框 PDF 侧也
> 长出三态 pill；向后保证所有 pre-R41 PDF JSON `--options` 一字节不变继续工作。
>
> **正交性**：本轮只动 PDF 路径与共享 `ColorPolicy` 模块抽取，**不动 R40 SVG
> 已有契约**（SVG 通过 `pub use` 重导出共享模块，外部签名零变化）。与并行可能
> 在做的 R41-SVG（`group_by_layer` / `compress_gzip` 字段已登记）完全文件级
> 解耦。

---

## 1. 现状

### 1.1 R40 已经做的（SVG 端）

```rust
// src/io/svg_export.rs L48
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ColorPolicy {
    Full,
    #[default]
    Monochrome,
    Grayscale,
}

// L268 — 双字段裁决器
pub(crate) fn effective_color_policy(opts: &SvgExportOptions) -> ColorPolicy { ... }

// L288 — 三态颜色映射（BT.601 luminance）
pub(crate) fn apply_color_policy(policy: ColorPolicy, r: f32, g: f32, b: f32) -> (f32, f32, f32) {
    match policy {
        ColorPolicy::Full => (r, g, b),
        ColorPolicy::Monochrome => (0.0, 0.0, 0.0),
        ColorPolicy::Grayscale => {
            let y = 0.299 * r + 0.587 * g + 0.114 * b;
            (y, y, y)
        }
    }
}
```

### 1.2 PDF 端缺口

```rust
// src/io/pdf_export.rs L37
pub struct PdfExportOptions {
    pub monochrome: bool,                  // 仅 bool — 缺 Grayscale 档位
    // ...
}
```

`pdf_export.rs` 内 **7 处**直接读 `options.monochrome` 决定颜色（L429, L607,
L663, L729, L1234, L1818, L2384 测试），全部走 `if options.monochrome { 黑 }
else { 原色 }` 二态分支。Gradient strip-fill 在 L2801 的回归测试只对比
`monochrome=true/false` 两路径字节差异，缺第三档。

`PdfExportDialogField` enum（`src/app/mod.rs` L278）只有 `Monochrome` 单 toggle，
对话框 `src/ui/pdf_export_dialog.rs` L134 一个 `toggle("Monochrome (force black)", ...)`。

`docs/svg_export.md` 存在并已经在 R40 扩充字段表；`docs/pdf_export.md` **不
存在**。

### 1.3 约束（硬性回归屏障）

`src/cli.rs` 的 JSON 反序列化 `#[derive(serde::Deserialize)]` + `#[serde(default)]`
覆盖 `PdfExportOptions`——R38 集成测试 `cli_options_json_produces_valid_pdf` /
`cli_options_missing_file_exits_one` / `cli_options_malformed_json_exits_one`
是硬性回归屏障。

`src/io/pdf_export.rs` L2384 `fixture_pdf_options_monochrome_forces_black_strokes`
也是契约屏障——pre-R41 行为必须字节级一致。

---

## 2. 范围

| 任务 | 优先级 | 预估 |
|------|-------|------|
| T1 抽出 `src/io/color_policy.rs` 模块（`ColorPolicy` enum + `apply_color_policy`），SVG 端改 `pub use crate::io::color_policy::*` 重导出 | P0 | 0.4 h |
| T2 `PdfExportOptions` 新增 `color_policy: ColorPolicy` 字段（保留 legacy `monochrome: bool`）+ `effective_color_policy_pdf()` helper | P0 | 0.3 h |
| T3 改写 7 处 `if options.monochrome` 分支统一走 helper；新增 Grayscale 分支（BT.601 luminance） | P0 | 0.5 h |
| T4 Gradient HATCH PDF strip-fill 三态适配（已有 mono 灰度 ramp、Full 原色 ramp，新增 Grayscale 两 stop 各走 luminance） | P0 | 0.3 h |
| T5 `PdfExportDialogField` 增加 `ColorPolicyFull` / `ColorPolicyMono` / `ColorPolicyGray` 三 variant；`apply_toggle` + `view_window` 接入；保留 legacy `Monochrome` toggle 行为镜像 | P0 | 0.4 h |
| T6 单元测试：effective + apply 三态 + 端到端 PDF 字节差异 + JSON 兼容（R38 三条不变 + 新四条）+ 对话框 pill 互斥 | P0 | 0.5 h |
| T7 `docs/pdf_export.md` 新建（与 `docs/svg_export.md` 对称的字段表 + ColorPolicy 三态 + JSON 示例）+ CHANGELOG R41-PDF 章节 | P0 | 0.3 h |
| T8 跑 `cargo test --locked --workspace --all-targets` + `RUSTFLAGS=-Dwarnings cargo check --locked --workspace --all-targets` | P0 | 0.2 h |

**不纳入**：
- ODA SVG 三种坐标模式（`SVG_WORLD_COORDINATES` / `SVG_WHOLE_DRAWING`）— R42
- Paper Space Layout + 多视口合成（`setupActiveLayoutViews` parity）— R42
- OLE 对象 / Rational NURBS / SVG `<pattern>` 真实 hatch — R43
- SVG R41 `group_by_layer` + `compress_gzip` 收口（CHANGELOG 章节 / docs / 测试）— 另一支
- PDF Shading Pattern（`sh` 操作符 + `/Pattern` resource）— R39 不在本轮里已说明
- DWG 输入批处理 CLI 路径（h7cad-native-dwg fallback 已通过 GUI 路径，CLI 仍只稳定 DXF）

---

## 3. 设计

### 3.1 共享 `ColorPolicy` 模块（T1）

新建 `src/io/color_policy.rs`：

```rust
//! Three-way color policy mirroring ODA's `ColorPolicy` device property.
//!
//! Shared by SVG export (R40) and PDF export (R41) so the two pipelines
//! agree byte-for-byte on what `Full` / `Monochrome` / `Grayscale` mean.

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ColorPolicy {
    Full,
    #[default]
    Monochrome,
    Grayscale,
}

/// Map an RGB triplet through the resolved policy.
///
/// * `Full` returns the input unchanged.
/// * `Monochrome` collapses everything to black.
/// * `Grayscale` projects to BT.601 luminance `0.299R + 0.587G + 0.114B`.
pub fn apply_color_policy(policy: ColorPolicy, r: f32, g: f32, b: f32) -> (f32, f32, f32) {
    match policy {
        ColorPolicy::Full => (r, g, b),
        ColorPolicy::Monochrome => (0.0, 0.0, 0.0),
        ColorPolicy::Grayscale => {
            let y = 0.299 * r + 0.587 * g + 0.114 * b;
            (y, y, y)
        }
    }
}
```

**注册**：`src/io/mod.rs` 加 `pub mod color_policy;`。

**SVG 端重导出**（保持外部 callers 零修改）：

```rust
// src/io/svg_export.rs — 删除原 enum 与 apply 函数，改为
pub use crate::io::color_policy::{ColorPolicy, apply_color_policy};
// effective_color_policy(opts: &SvgExportOptions) 留在 svg_export.rs 不动
```

R40 单元测试（`apply_color_policy_full_is_identity` 等 6 条）继续在
`svg_export.rs` 的 `mod tests` 跑——`apply_color_policy` 通过 `pub use` 引入，
测试代码不动。R40 `effective_color_policy_*` 4 条也继续不动。

### 3.2 PDF 双字段策略（T2）

```rust
// src/io/pdf_export.rs
use crate::io::color_policy::{apply_color_policy, ColorPolicy};

pub struct PdfExportOptions {
    pub color_policy: ColorPolicy,    // R41 主字段
    pub monochrome: bool,             // legacy — 仅 JSON / GUI 兼容
    // ... existing ...
}

impl Default for PdfExportOptions {
    fn default() -> Self {
        Self {
            color_policy: ColorPolicy::Monochrome,  // 与 R40 SVG 默认对齐
            monochrome: true,                       // 与 pre-R41 默认对齐
            // ... existing ...
        }
    }
}

/// Same裁决契约 as SVG。
pub(crate) fn effective_color_policy_pdf(opts: &PdfExportOptions) -> ColorPolicy {
    match opts.color_policy {
        ColorPolicy::Monochrome => {
            if opts.monochrome {
                ColorPolicy::Monochrome
            } else {
                ColorPolicy::Full
            }
        }
        other => other,
    }
}
```

**优先级矩阵**（与 R40 SVG 完全一致，便于后续若用宏统一）：

| `color_policy` | `monochrome` | effective |
|----------------|--------------|-----------|
| `Full` | _不关心_ | `Full` |
| `Grayscale` | _不关心_ | `Grayscale` |
| `Monochrome` (default) | `true` | `Monochrome` |
| `Monochrome` (default) | `false` | **`Full`** |

**JSON 兼容**：
- 旧 `{"monochrome": false}` → effective `Full` ✓ 与 pre-R41 一致
- 旧 `{"monochrome": true}` → effective `Monochrome` ✓ 与 pre-R41 一致
- 新 `{"color_policy": "grayscale"}` → effective `Grayscale` ✓ R41 新能力
- 新 `{"color_policy": "full", "monochrome": true}` → effective `Full` ✓
  （新字段明确表态，不理会 legacy bool）

### 3.3 PDF 7 处 monochrome 分支重写（T3）

| 文件位置 | 当前 | 改写后 |
|---|---|---|
| L429 stroke 颜色（wire 路径） | `if options.monochrome { (0,0,0) } else { rgb }` | `apply_color_policy(eff, r, g, b)` |
| L607 hatch 填充 | 同上 | 同上 |
| L663 pattern hatch 浅灰兜底 | mono 时强制浅灰 | mono / gray 各走 helper，full 走原色 |
| L729 hatch 描边 | 同 L429 | 同上 |
| L1234 native curve 颜色 | 同 L429 | 同上 |
| L1818 native text fill | `if mono { (0,0,0) } else { rgb }` | `apply_color_policy(eff, r, g, b)` |
| L2384 既有测试 | `monochrome: true/false` 二态 | 不动（向后兼容 canary） |

**Pattern hatch 浅灰兜底**特殊处理：R39 把 mono pattern 兜底成 `(0.85, 0.85, 0.85)`
浅灰，避免黑底图案完全占满；R41 在 Grayscale 模式下保留同样兜底（视觉行为一致），
Full 模式下走原色。

### 3.4 Gradient HATCH PDF strip 三态适配（T4）

R39 `emit_hatch_gradient_strips()` 沿 `angle_deg` 切 48 条平行 strip，每条
颜色按 strip 中心位置在 `color` → `color2` 之间插值。R39 已经为 mono 模式
专门做了 `0.15 + 0.70 * u` 的灰度 ramp。R41 改为：

```rust
let eff = effective_color_policy_pdf(options);
for k in 0..STRIP_COUNT {
    let u = (k as f32 + 0.5) / STRIP_COUNT as f32;
    // 在 color -> color2 之间线性插值
    let (r, g, b) = lerp_rgb(color, color2, u);
    // 然后映射到 effective policy
    let (r, g, b) = apply_color_policy(eff, r, g, b);
    paint_strip(k, (r, g, b));
}
```

mono 模式下 `apply_color_policy` 把 strip 全部塌缩成 `(0,0,0)`——梯度方向消失。
为保留 R39 "黑→浅灰"兜底（保证 mono 打印时方向仍可辨识），mono 分支走特殊
插值 `(0.15 + 0.70u, ...)`，与 R39 行为字节级一致；其它两态走 helper。

```rust
let (r, g, b) = match eff {
    ColorPolicy::Monochrome => {
        let g = 0.15 + 0.70 * u;
        (g, g, g)
    }
    policy => {
        let (r, g, b) = lerp_rgb(color, color2, u);
        apply_color_policy(policy, r, g, b)
    }
};
```

### 3.5 GUI 对话框三态 pill（T5）

`src/app/mod.rs` `PdfExportDialogField` 扩展：

```rust
pub enum PdfExportDialogField {
    /// Legacy bool toggle — kept so `toggle_flips_each_boolean_field`
    /// stays green; flipping it flips both `monochrome` and
    /// `color_policy` (`true→Monochrome` / `false→Full`).
    Monochrome,
    /// R41 color-policy pill: `ColorPolicy::Full`.
    ColorPolicyFull,
    /// R41 color-policy pill: `ColorPolicy::Monochrome`.
    ColorPolicyMono,
    /// R41 color-policy pill: `ColorPolicy::Grayscale`.
    ColorPolicyGray,
    // ... existing ...
}
```

`src/ui/pdf_export_dialog.rs` 增加 `apply_color_policy_pill` 函数 + 三 pill
按钮（与 SVG 对话框完全对称）：

```rust
let color_policy_section = column![
    section_label("Color & Strokes"),
    row![
        lbl("Color policy"),
        policy_pill("Full", ColorPolicy::Full,       eff == ColorPolicy::Full),
        policy_pill("Mono", ColorPolicy::Monochrome, eff == ColorPolicy::Monochrome),
        policy_pill("Gray", ColorPolicy::Grayscale,  eff == ColorPolicy::Grayscale),
    ].spacing(6),
    toggle("Monochrome (legacy)", opts.monochrome, F::Monochrome),  // 维持原 toggle
].spacing(8);
```

`apply_toggle` 对三个 pill 的处理：

```rust
F::ColorPolicyFull      => {
    opts.color_policy = ColorPolicy::Full;
    opts.monochrome = false;  // legacy 跟随，避免不一致
}
F::ColorPolicyMono      => {
    opts.color_policy = ColorPolicy::Monochrome;
    opts.monochrome = true;
}
F::ColorPolicyGray      => {
    opts.color_policy = ColorPolicy::Grayscale;
    opts.monochrome = false;  // 灰度路径 effective != Mono
}
F::Monochrome           => {
    opts.monochrome = !opts.monochrome;
    opts.color_policy = if opts.monochrome {
        ColorPolicy::Monochrome
    } else {
        ColorPolicy::Full
    };
}
```

这样三个 pill 互斥（每次切换都把 `color_policy` 设到唯一值），legacy
`Monochrome` toggle 翻 bool 时同步 `color_policy`，两字段始终一致——R41 的
`legacy_monochrome_toggle_keeps_color_policy_in_sync` 测试锁定这一行为。

---

## 4. 测试

### 4.1 单元测试（`src/io/color_policy.rs` 新模块）

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test] fn full_is_identity() { ... }
    #[test] fn monochrome_collapses_to_black() { ... }
    #[test] fn grayscale_uses_bt601_luminance() { ... }
    #[test] fn default_is_monochrome() { ... }
    #[test] fn json_roundtrip_three_variants() { ... }
}
```

5 条 — 这些是把 R40 SVG 端的 `apply_color_policy_*` 测试**移植**到共享模块（行为已经
被 R40 测试覆盖，移植后 SVG 端的测试通过 `pub use` 仍跑同一份函数，零行为变化）。

### 4.2 单元测试（`src/io/pdf_export.rs` 扩展）

```rust
#[test] fn effective_pdf_color_policy_defaults_to_monochrome() { ... }
#[test] fn effective_pdf_color_policy_new_field_wins_when_non_default() { ... }
#[test] fn effective_pdf_color_policy_falls_back_to_legacy_monochrome_when_unset() { ... }
#[test] fn effective_pdf_color_policy_conflict_new_field_overrides_legacy() { ... }

#[test] fn pdf_full_mode_strokes_use_source_color() { /* 端到端字节断言 */ }
#[test] fn pdf_grayscale_mode_strokes_use_bt601_luminance() {
    // 一根红色 LINE → PDF 字节流应含 BT.601 luminance（rgb 0.299）的 set-stroke-color op
    // 与 monochrome 路径字节不同，与 full 路径字节不同
}
#[test] fn pdf_three_color_policies_produce_distinct_bytes() {
    // 同一份 fixture 用 Full / Mono / Gray 三种导出，三份字节互不相同
}

#[test] fn pdf_gradient_grayscale_strips_use_luminance_ramp() {
    // R41 渐变 strip Grayscale 模式不塌缩成黑色，按 luminance ramp 分布
}

// JSON 反序列化兼容
#[test] fn pdf_legacy_json_monochrome_false_deserialises_to_full_effective_policy() { ... }
#[test] fn pdf_legacy_json_monochrome_true_stays_monochrome() { ... }
#[test] fn pdf_new_json_color_policy_grayscale_sets_enum() { ... }
#[test] fn pdf_new_json_color_policy_full_overrides_legacy_monochrome_true() { ... }
```

合计 **+11 条**，加上既有 24 条不动 → 35 条。

### 4.3 对话框测试（`src/ui/pdf_export_dialog.rs` 扩展）

既有 3 条保留（`toggle_flips_each_boolean_field` / `font_choice_updates_options` /
`font_size_scale_field_does_not_flip_on_toggle_path`），R41 新增：

- `color_policy_pills_select_exactly_one_mode_at_a_time`
- `legacy_monochrome_toggle_keeps_color_policy_in_sync`

合计 **+2 条** → 5 条。

### 4.4 集成测试（不新增）

R38 三条 `cli_options_*` 是 JSON 兼容性的端到端 canary，行为不变。

---

## 5. 验收

```bash
cargo check -p H7CAD                                # 零新 warning
cargo test --bin H7CAD io::color_policy             # 5 / 5 全绿（新模块）
cargo test --bin H7CAD io::pdf_export               # 24 → 35 全绿
cargo test --bin H7CAD ui::pdf_export_dialog        # 3 → 5 全绿
cargo test --bin H7CAD io::svg_export               # 88 / 88 不变（pub use 透传）
cargo test --bin H7CAD ui::svg_export_dialog        # 7 / 7 不变
cargo test --bin H7CAD                              # 450 → 463 全绿
cargo test --test cli_batch_export                  # 10 / 10 不变
cargo test --locked --workspace --all-targets       # 全绿
RUSTFLAGS=-Dwarnings cargo check --locked --workspace --all-targets  # 零新 warning
```

手动：
- 启动 H7CAD，`PDFEXPORTDIALOG` 打开对话框，三态 pill 切换 → effective 状态
  即时反映到 active pill；点 Export… 产 PDF
- CLI：`echo '{"color_policy":"grayscale"}' > gray.json` →
  `h7cad.exe drawing.dxf --export-pdf out.pdf --options gray.json` →
  PDF 颜色应走灰度 ramp，与 `monochrome=true` / `color_policy=full` 字节互不
  相同（脚本 `cmp out_gray.pdf out_mono.pdf` 输出第一个差异 byte）

---

## 6. 状态

- [x] 计划定稿（2026-04-25）
- [x] T1 抽出 `src/io/color_policy.rs` + SVG `pub use` 重导出
- [x] T2 `PdfExportOptions` + `effective_color_policy_pdf`
- [x] T3 5 处 `monochrome` 分支改走 helper（实际为 5 处生产 + 1 处测试 canary）
- [x] T4 Gradient strip 三态适配
- [x] T5 PDF 对话框三态 pill
- [x] T6 单元测试（共享模块 6 + PDF 端 11 + 对话框 2 = 19 条新增；既有 mono canary 不变）
- [x] T7 `docs/pdf_export.md` + CHANGELOG R41-PDF
- [x] T8 全量验证（workspace `cargo test` + `RUSTFLAGS=-Dwarnings cargo check` 双绿）

---

## 7. 下轮方向

- **R42**：Paper Space Layout 切换 + 多视口合成（`setupActiveLayoutViews` parity）
- **R43**：OLE 对象 + Rational NURBS + SVG `<pattern>` 真实 hatch
- **可选**：把 SVG / PDF 各自的 `effective_color_policy*` 用泛型 / trait 进一步合并
  （当前两份 5 行函数体相同，但读不同 struct；trait 抽象 ROI 太低，留作技术债）
