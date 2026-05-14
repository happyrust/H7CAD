# SVG Hatch Pattern Lines（R43-A）

> **起稿**：2026-04-25（第四十三轮 · A 阶段）
> **前置**：R33（PDF Phase 2a）已经为 PDF 端实现了 line family 真实 hatch
> 渲染（`emit_hatch_pattern_lines`，每条 pattern line 一条 PDF Line op，含
> Liang-Barsky AABB clip + dash 周期跨线对齐）。
>
> **当前缺口**：SVG 端 `HatchPattern::Pattern(_families)` **完全占位** —
> 只画一条 `<polygon fill="none" stroke="..." stroke-width="0.1">` boundary
> outline，**完全忽略 family 数据**。这意味着 ANSI31 / ANSI32 这类标准
> hatch 在 SVG 输出里只剩下边框，而 PDF 输出里是完整的对角斜线网格。
>
> **目标**：复用 PDF R33 的 line family 算法（AABB clip + 行索引扫描 + 全
> 局相位），在 SVG 端 emit 真实的 `<polyline>` 集合，使 SVG / PDF 双输出
> 在 hatch pattern 上达到字节级几何等价。

---

## 1. 现状

### 1.1 SVG 端占位代码

```rust
// src/io/svg_export.rs L708
HatchPattern::Pattern(_families) => {
    // Pattern hatches: render boundary outline only (GPU pattern not
    // trivially reproducible in static SVG).
    svg.push_str("<polygon fill=\"none\" stroke=\"");
    svg.push_str(&fill_color);
    svg.push_str("\" stroke-width=\"0.1\"");
    svg.push_str(&class_attr);
    svg.push_str(" points=\"");
    // ... boundary points ...
}
```

`families` 完全没读，注释还说"GPU pattern not trivially reproducible" —
这是 R33 之前的判断，现在 PDF 端已经证明 line family 完全可以静态展开。

### 1.2 PDF 端参考实现

`emit_hatch_pattern_lines`（`src/io/pdf_export.rs` L770-L920）：

1. AABB of boundary（trade-off：使用 AABB 而非真实 boundary，pattern lines
   可能略微伸出 non-convex boundary，但保证不丢线）
2. 颜色走 `apply_color_policy(effective_color_policy_pdf(opts), r, g, b)`
3. 对每个 family：
   - `(sin_a, cos_a) = angle_rad.sin_cos()`
   - `dx, dy = family.dx * scale, family.dy * scale`
   - 退化检查 `dy.abs() < 1e-4 → continue`（perp_step 为 0 ⇒ 所有线重合）
   - 把 4 个 AABB 角投影到 perp 轴 → 行号 `n_start..=n_end`
   - PATTERN_LINES_CAP=4000 防爆炸
   - 每行：`origin + n*dy*perp + n*dx*dir`，Liang-Barsky 切到 AABB →
     `(t0, t1)`
   - solid line → 单段 `Line`；dashed → walk dash sequence 累加

### 1.3 共享算法

`aabb_of(&[[f32; 2]]) -> (f32, f32, f32, f32)` 与 `clip_line_aabb(...)` 都
是纯几何，与 PDF/SVG 无关，可以**抽成共享的 hatch geom helper**（新模块
`src/scene/hatch_geom.rs`）让 SVG/PDF 双方共用，避免 SVG 复制粘贴算法。

---

## 2. 范围

| 任务 | 优先级 | 预估 |
|------|-------|------|
| T1 抽出共享几何 helper：`src/scene/hatch_geom.rs` (`aabb_of` + `clip_line_aabb`)；PDF 端改为 `pub use` 重导出 | P0 | 0.3 h |
| T2 `SvgExportOptions` 加 `hatch_patterns: bool`（默认 `true`，与 PDF 对称） | P0 | 0.2 h |
| T3 SVG 端实现 `emit_hatch_pattern_lines_svg`：移植 PDF 算法，emit `<polyline points="x0,y0 x1,y1" stroke="rgb(...)" stroke-width="0.1" />` × N；dashed 用 `stroke-dasharray="d1 d2"` 一条 `<polyline>` | P0 | 0.6 h |
| T4 `emit_hatches` 的 `HatchPattern::Pattern` 分支改派发：`if options.hatch_patterns { emit_hatch_pattern_lines_svg } else { 既有 outline 占位 }` | P0 | 0.2 h |
| T5 `SvgExportDialogField` 加 `HatchPatterns` toggle；`view_window` 在 Geometry section 加；`apply_toggle` 接入 | P0 | 0.3 h |
| T6 单元测试：Pattern 有 family 时 `emit polyline ×N` 字节断言；hatch_patterns=false 仍是 outline；空 family 跳过；degenerate dy=0 跳过；`stroke-dasharray` 序列化 | P0 | 0.5 h |
| T7 `docs/svg_export.md` 加 `hatch_patterns` 字段 + Hatch pattern 子节；CHANGELOG R43-A 章节 | P0 | 0.3 h |
| T8 跑 `cargo test --locked --workspace --all-targets` + `RUSTFLAGS=-Dwarnings cargo check --locked --workspace --all-targets` | P0 | 0.2 h |

**不纳入**：
- 真实 SVG `<pattern>` 元素方案（文件更小但 transform/clip 复杂，Inkscape
  / 浏览器引擎差异较大；R43-A2 可作为优化轮）
- Rational NURBS（R43-B）/ OLE 对象（R43-C）/ DWG CLI 输入（R44）
- Paper Space 多视口合成（R45）

---

## 3. 设计

### 3.1 共享几何 helper

新建 `src/scene/hatch_geom.rs`：

```rust
//! Pure geometry helpers shared between PDF (R33) and SVG (R43-A) hatch
//! pattern line emission.  Neither helper writes any output bytes — they
//! only return geometric data so each exporter can format its own.

/// Axis-aligned bounding box of a polygon ring.  Returns `(x0, y0, x1, y1)`
/// in CAD world coordinates (no offset applied).
pub fn aabb_of(points: &[[f32; 2]]) -> (f32, f32, f32, f32) { ... }

/// Liang-Barsky line-vs-AABB clip.  Returns `Some((t0, t1))` for the
/// parametric range where the ray `P(t) = (ox, oy) + t * (dx, dy)`
/// intersects the box, or `None` when the ray misses entirely.
pub fn clip_line_aabb(
    ox: f32, oy: f32, dx: f32, dy: f32,
    bx0: f32, by0: f32, bx1: f32, by1: f32,
) -> Option<(f32, f32)> { ... }
```

PDF `pdf_export.rs` 改 `pub use crate::scene::hatch_geom::{aabb_of, clip_line_aabb};`
重导出（保持 PDF 内部 callers 零修改）；SVG 直接 `use crate::scene::hatch_geom::*`.

### 3.2 SvgExportOptions 字段

```rust
pub struct SvgExportOptions {
    // ... existing ...
    /// **R43-A**: emit `HatchPattern::Pattern(family)` as real `<polyline>`
    /// stroke families (mirrors PDF R33 / `PdfExportOptions::hatch_patterns`).
    /// When `false`, falls back to the pre-R43 outline-only placeholder
    /// (a single `<polygon fill="none" stroke=... />` around the boundary).
    /// Default `true`.
    pub hatch_patterns: bool,
}
```

`Default::default()` 加 `hatch_patterns: true`.

### 3.3 SVG emit 函数

```rust
fn emit_hatch_pattern_lines_svg(
    svg: &mut String,
    hatch: &HatchModel,
    families: &[crate::scene::hatch_model::PatFamily],
    ox: f32,
    oy: f32,
    options: &SvgExportOptions,
    class_attr: &str,
) {
    if families.is_empty() { return; }

    let (bx0, by0, bx1, by1) = crate::scene::hatch_geom::aabb_of(&hatch.boundary);
    if (bx1 - bx0).abs() < 1e-6 || (by1 - by0).abs() < 1e-6 { return; }

    let policy = effective_color_policy(options);
    let [r, g, b, _] = hatch.color;
    let (r, g, b) = apply_color_policy(policy, r, g, b);
    let stroke = format!("rgb({},{},{})",
        (r * 255.0).round() as i32,
        (g * 255.0).round() as i32,
        (b * 255.0).round() as i32);

    let scale = hatch.scale.max(1e-6);
    let global_offset = hatch.angle_offset;

    for family in families {
        // ... compute n_start..=n_end (same as PDF) ...
        for n in n_start..=n_end {
            let (origin_x, origin_y) = ...;
            if let Some((t0, t1)) = clip_line_aabb(...) {
                let p0x = origin_x + t0 * dir_x + ox;
                let p0y = origin_y + t0 * dir_y + oy;
                let p1x = origin_x + t1 * dir_x + ox;
                let p1y = origin_y + t1 * dir_y + oy;

                if family.dashes.is_empty() {
                    write!(svg, "<polyline points=\"{:.4},{:.4} {:.4},{:.4}\" \
                                  stroke=\"{}\" stroke-width=\"0.1\" \
                                  fill=\"none\"{}/>\n",
                           p0x, p0y, p1x, p1y, stroke, class_attr).ok();
                } else {
                    // Use SVG stroke-dasharray for the dash pattern.
                    // dashes are alternating dash/gap with sign convention
                    // (positive dash, negative gap), but stroke-dasharray
                    // wants all positive lengths in dash/gap/dash/gap order.
                    let dasharray: Vec<String> = family.dashes.iter()
                        .map(|d| format!("{:.4}", d.abs()))
                        .collect();
                    write!(svg, "<polyline points=\"{:.4},{:.4} {:.4},{:.4}\" \
                                  stroke=\"{}\" stroke-width=\"0.1\" \
                                  fill=\"none\" stroke-dasharray=\"{}\"{}/>\n",
                           p0x, p0y, p1x, p1y, stroke, dasharray.join(" "),
                           class_attr).ok();
                }
            }
        }
    }
}
```

### 3.4 dispatch 在 `emit_hatches`

```rust
HatchPattern::Pattern(families) => {
    if options.hatch_patterns {
        emit_hatch_pattern_lines_svg(
            svg, hatch, families, ox, oy, options, &class_attr,
        );
    } else {
        // Pre-R43 fallback: outline only.
        svg.push_str("<polygon fill=\"none\" stroke=\"");
        // ... existing outline emission ...
    }
}
```

### 3.5 GUI 对话框

`SvgExportDialogField` 加 `HatchPatterns` variant；`view_window` 在
Geometry section 末尾追加：

```
[x] Emit hatch pattern lines (line family)        ← R43-A
```

`apply_toggle`：`F::HatchPatterns => opts.hatch_patterns = !opts.hatch_patterns`

---

## 4. 测试

### 4.1 单元测试（`src/io/svg_export.rs` 扩展）

新增 5 条：

- `pattern_hatch_emits_polylines_with_default_options` — 构造一个 ANSI31-类
  pattern（45° 单 family，spacing 1），SVG 输出含 `<polyline` 字样，且
  数量 ≥ 1
- `pattern_hatch_off_falls_back_to_outline_only` — `hatch_patterns=false`
  时回退到 R32 的占位 `<polygon fill="none" stroke=... />`
- `pattern_hatch_with_dash_pattern_emits_stroke_dasharray` — dashed 家族产
  生含 `stroke-dasharray="..."` 的 polyline
- `pattern_hatch_empty_family_skipped` — 空 family 不 emit 任何 polyline
- `pattern_hatch_degenerate_perp_step_skipped` — `dy=0` 的 family 跳过（避
  免无限循环）

### 4.2 对话框测试（`src/ui/svg_export_dialog.rs`）

既有 `toggle_flips_each_boolean_field` 扩展含 `HatchPatterns`；既有
`numeric_fields_do_not_flip_booleans` 不动。

### 4.3 共享几何测试（`src/scene/hatch_geom.rs`）

新增 3 条：
- `aabb_of_includes_all_points`
- `clip_line_aabb_horizontal_through_box_returns_full_extent`
- `clip_line_aabb_misses_box_returns_none`

---

## 5. 验收

```bash
cargo check -p H7CAD                                # 零新 warning
cargo test --bin H7CAD scene::hatch_geom            # 3 / 3 全绿（新模块）
cargo test --bin H7CAD io::svg_export               # 103 → 108 全绿
cargo test --bin H7CAD ui::svg_export_dialog        # 7 / 7 不变（toggle 扩展）
cargo test --bin H7CAD io::pdf_export               # 32 / 32 不变（pub use 透传）
cargo test --bin H7CAD                              # 495 → 503 全绿
cargo test --test cli_batch_export                  # 16 / 16 不变
cargo test --locked --workspace --all-targets       # 全绿
RUSTFLAGS=-Dwarnings cargo check --locked --workspace --all-targets  # 零新 warning
```

手动：构造 ANSI31 hatch DXF → CLI 导出 SVG → 浏览器 / Inkscape 打开 →
应看到 45° 平行斜线，而不是只有 boundary 边框。

---

## 6. 状态

- [x] 计划定稿（2026-04-25）
- [x] T1 抽 `src/scene/hatch_geom.rs` + PDF `pub use`
- [x] T2 `SvgExportOptions` 加 `hatch_patterns`
- [x] T3 `emit_hatch_pattern_lines_svg` 实现
- [x] T4 `emit_hatches` 分发
- [x] T5 GUI 对话框 toggle
- [x] T6 单元测试
- [x] T7 docs + CHANGELOG
- [x] T8 全量验证

---

## 7. 下轮方向

- **R43-A2**（可选）：替换 `<polyline>` × N 为真实 SVG `<pattern>` 元素 +
  `<clipPath>`，文件大小可降到 1/N（每个 pattern 模板只写一次，boundary
  做 clip）。需要研究 Inkscape / 浏览器对 `userSpaceOnUse` patternTransform
  的兼容性
- **R43-B**：Rational NURBS（degree-2/3 + 非 unit weight）
- **R43-C**：OLE 对象（OLE2_FRAME entity 解析与渲染）
- **R44**：DWG CLI 输入批处理路径
- **R45**：Paper Space 多视口合成（`setupActiveLayoutViews` parity）
