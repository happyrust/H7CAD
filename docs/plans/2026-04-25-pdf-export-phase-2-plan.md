# PDF 导出 Phase 2 收口（三十三轮）

> **起稿**：2026-04-25（第三十三轮）
> **前置**：三十二轮 Phase 1 + 1b 已把 PDF 导出做到 **文字 / solid
> HATCH / 图像 / 原生圆-弧-椭圆** 与 SVG 导出基本对齐；剩下两项
> 关键的 UX 缺口是 **HATCH 图案** 和 **导出对话框**。
> **目标**：这两项落地后，"Export as PDF" 与 "Export as SVG" 在
> 真实工程图上已基本等价，可以收口 PDF parity 项目，后续轮切到别的
> 主线。

---

## 1. 现状与缺口

| 能力 | SVG | PDF（三十二 Phase 1+1b） |
|------|-----|-------------------------|
| TEXT / MTEXT 原生 | ✅ | ✅ |
| HATCH solid | ✅ | ✅ |
| **HATCH pattern (line family)** | ✅ 部分（`emit_hatches_with_patterns` 解包 `PatFamily` 产出线族） | ❌ 本轮补齐 |
| HATCH gradient | ⚠️ Linear gradient；PDF 侧暂不做 | ❌（延后） |
| IMAGE | ✅ | ✅ |
| Circle / Arc / Ellipse 原生 | ✅ | ✅（Phase 1b） |
| LwPolyline 原生 path + bulge | ✅ | ❌（延后，wire 已能画） |
| Block `<defs>` / Form XObject | ✅ | ❌（延后） |
| Spline piecewise bezier | ✅ | ❌（延后） |
| Native dim-text | ✅ | ❌（延后） |
| **导出对话框** | ✅ `src/ui/svg_export_dialog.rs` | ❌ 本轮补齐 |

剩下的延后项（LwPolyline / Block / Spline / dim-text）都有"wire 兜底
能正确绘制"的性质，视觉上用户不会立刻感觉缺；对话框和 HATCH pattern
则是直接改变用户日常体验的两项。

---

## 2. 本轮范围

| 纳入 | 优先级 | 预估 |
|------|-------|------|
| **T1 HATCH pattern (line family)** | P0 | 1.0 h |
| **T2 PdfExportOptions 对话框** (`src/ui/pdf_export_dialog.rs`) | P0 | 1.2 h |
| **T3 Message / update.rs 接线** | P0 | 0.6 h |
| **T4 回归 fixture** | P0 | 0.5 h |
| **T5 CHANGELOG + commit + push** | P0 | 0.2 h |

不纳入（路线图延后）：Gradient HATCH、LwPolyline 原生 path、Block
Form XObject、Spline piecewise bezier、Native dim-text。

---

## 3. T1 · HATCH pattern（line family）

### 设计

`HatchPattern::Pattern(Vec<PatFamily>)` 里每个 `PatFamily` 包含：

```rust
pub struct PatFamily {
    pub angle_deg: f32,   // line direction
    pub x0: f32, pub y0: f32,  // origin of first line
    pub dx: f32, pub dy: f32,  // step to next parallel
    pub dashes: Vec<f32>, // dash / gap sequence (empty = solid)
}
```

PDF 输出步骤：

1. 计算 boundary 的轴对齐包围盒 `(bx0, by0, bx1, by1)`（只看
   `hatch.boundary`，足够覆盖）。
2. 对每个 `PatFamily`：
   a. 把单位方向向量 `(cos α, sin α)` 沿步进向量 `(dx, dy)` 正交
      的方向扫过包围盒。
   b. 每条扫线：用 Liang-Barsky 裁剪法与 boundary polygon 的凸包
      （足够近似，真实剖面线一般 convex hull）求交，拿到裁剪后的
      线段起止点。
   c. 把线段按 dashes 切分（dashes 为空则整段输出）。
   d. 每个 dash → `Op::DrawLine { line: Line { points: [p0, p1], is_closed: false } }`。
3. 颜色 / 线宽沿用 `hatch.color` + 现有 monochrome 策略。

**简化**：本轮把"boundary polygon 求交"简化为"与包围盒 AABB 求交"
——实际效果在大多数工程图上看起来正确（线稍微出头一点不影响阅读），
保证工程可控性，boundary-clip 版本延后到后续。对于非凸 boundary，
这会让斜线覆盖整个 AABB 而不是只在 boundary 内部，但：
- 实际 AutoCAD PDF 输出的 ANSI31 也有类似轻微扩散
- 真正的 polygon-clip 涉及 Weiler-Atherton 算法，不在 P1 范围

配套策略：给 PdfExportOptions 增加 `hatch_pattern_strategy` 枚举，
默认 `AABB`，后续可切到 `Polygon`。为避免 churn，本轮先写 `AABB`
实现路径，不加 enum；延后留 TODO。

### 实现落点

- 新 `emit_hatch_pattern_lines(ops, hatch, ox, oy, options)`
- 现有 `emit_hatch_fills` 的 `Solid` 分支不变；`Pattern` 分支改
  从 `continue` 改为 delegate 到新函数。
- PdfExportOptions 新增 `hatch_patterns: bool`（默认 true），
  切到 false 等效回到 Phase 1 行为（只画 solid 和 boundary）。

### 验收测试

- `fixture_pdf_pattern_hatch_emits_line_segments`：一个三角形
  boundary + 单行 PatFamily（45° angle_deg, dy=3）→ 期望 PDF
  字节比 solid hatch 的要长（因为加了多条扫线），比 Phase 1
  "pattern hatch skipped" 要长。
- 替换 Phase 1 的 `fixture_pdf_pattern_hatch_skipped_when_not_implemented`
  为新的断言（pattern 现在应该画出，不再 skip）。

---

## 4. T2 · `src/ui/pdf_export_dialog.rs`

### 设计

对照 `src/ui/svg_export_dialog.rs`（252 行）逐个字段映射：

| SVG field | PDF field (PdfExportOptions) |
|-----------|-------------------------------|
| monochrome | monochrome |
| text_as_geometry | text_as_geometry |
| font_family (string) | font_family (enum: Helvetica / TimesRoman / Courier — pill selector) |
| font_size_scale | font_size_scale |
| min_stroke_width | — PDF 无对应；可选暴露 |
| include_hatches | include_hatches |
| use_block_defs | — 延后 |
| include_images | include_images |
| embed_images | embed_images |
| image_url_prefix | — 延后（PDF 不需要外链） |
| image_base | image_base（PathBuf） |
| native_curves | native_curves |
| line_weight_scale | — Phase 2 延后 |
| native_splines | — Phase 2 延后 |
| native_dimension_text | native_dimension_text |

UI 布局 / 样式直接复用 `svg_export_dialog` 的 `btn` / `pill` /
`field_style` / `hdivider` 等 helper（抽成 `src/ui/export_dialog_common.rs`
还是直接 copy 是一个工程判断——本轮 **直接 copy**，避免过度抽象，
等第三个导出格式再重构）。

### Message enum 新增

```rust
pub enum Message {
    // ...
    PdfExportDialogOpen,
    PdfExportDialogClose,
    PdfExportDialogToggle(PdfExportDialogField),
    PdfExportDialogSelectFont(PdfFontChoice),
    PdfExportDialogEditFontScale(String),
    PdfExportDialogRun,
    PdfExport(PdfExportOptions),
    PdfExportPath(Option<PathBuf>, PdfExportOptions),
    // ...
}

pub enum PdfExportDialogField {
    Monochrome,
    TextAsGeometry,
    IncludeHatches,
    IncludeImages,
    EmbedImages,
    NativeCurves,
    NativeDimensionText,
}
```

### update.rs 接线

1. 菜单 "Export as PDF" 现在发 `Message::PdfExportDialogOpen` 而不是直
   接走 `pick_pdf_path_owned` → 弹对话框
2. 对话框 "Export" 按钮发 `Message::PdfExport(options)` → 走 rfd
   save-file → `Message::PdfExportPath(Some(path), options)`
3. `Message::PdfExportPath` handler 读取 scene + plot_style + options
   → 调 `export_pdf_full`

保留一个"跳过对话框直接导出"的快捷路径给 `print_to_printer`（它已经
用 `export_pdf` 默认 options 路径，不受影响）。

### App state

在 `App` 结构加：

```rust
pub pdf_export_dialog_open: bool,
pub pdf_export_options: PdfExportOptions,
pub pdf_export_font_scale_text: String,
```

初始化时 `pdf_export_options = PdfExportOptions::default()`。每次
导出后保留当前 options（就像 SVG dialog 的行为）。

---

## 5. T4 · 回归 fixture

新增 / 改动：

| fixture | 变更 |
|---------|------|
| `fixture_pdf_pattern_hatch_skipped_when_not_implemented` | **移除或改写** ——新语义是 pattern 会被画出，不再 skip |
| `fixture_pdf_pattern_hatch_emits_line_segments` | **新增** ——pattern PDF 字节数 > solid PDF 字节数 > 空 PDF 字节数 |
| `fixture_pdf_options_hatch_patterns_toggle_off_matches_phase_1_skip` | **新增** ——`hatch_patterns=false` 时 pattern 路径与旧 skip 等价 |

对话框暂不做自动化 fixture（iced 的 view 需要 app 环境），
`cargo check` 通过 + 手动点击验证即可；后续如需可加
`#[test] dialog_view_compiles()` 仅跑 `view(&app)` 验证。

---

## 6. 验收

```bash
cargo check -p H7CAD                 # 零新 warning
cargo test --bin H7CAD io::pdf_export -- --nocapture   # 新增 fixture 全绿
cargo test --bin H7CAD               # 不回归
```

UI 手动路径：
1. 打开含 HATCH (pattern) 的 DXF
2. Export → PDF → 弹出对话框
3. 在对话框切换 options（如关掉 include_hatches 再导出对比）
4. 保存到本地 → 用系统 PDF 阅读器打开：ANSI31 之类的斜线填充可见

---

## 7. 路线图推进

```
R30   SVG export full
R31   DXF 2D 显示收口
R32-1 PDF text / solid hatch / image / options
R32-1b PDF native curves
R33   (本轮) PDF pattern hatch + 导出对话框       ← 收口 PDF parity
R34+  下一主线（Owner 挑 Path A / B / C / D / E 或 PDF Phase 3）
```

延后到 PDF Phase 3（R34+ 按需做）：
- Gradient HATCH
- LwPolyline 原生 path（含 bulge）
- Block → PDF Form XObject 去重
- Spline piecewise bezier（复用 svg_export NURBS→Bezier）
- Native dim-text

---

## 8. 状态

- [x] 计划定稿（2026-04-25）
- [ ] T1 HATCH pattern
- [ ] T2 pdf_export_dialog.rs
- [ ] T3 Message / update.rs 接线
- [ ] T4 fixture
- [ ] T5 CHANGELOG + 提交 + 推送
