# Gradient HATCH：SVG `<linearGradient>` + PDF strip-fill（三十九轮）

> **起稿**：2026-04-25（第三十九轮）
> **前置**：三十二轮 PDF Phase 1 / 三十三轮 Phase 2 收口 solid + pattern
> HATCH，把 gradient 留到 Phase 3。R35 结尾明确说 "Gradient HATCH /
> Block Form XObject 延到后续轮次"。本轮关闭 gradient 缺口。
> **目标**：`HatchPattern::Gradient` 在 SVG 和 PDF 导出里从"半透明基色
> polygon"升级到真正的线性渐变显示，与 GPU 渲染的 `angle_deg` + `color →
> color2` 一致。

---

## 1. 现状

### 1.1 数据模型（已就绪）

```rust
pub enum HatchPattern {
    Solid,
    Pattern(Vec<PatFamily>),
    Gradient { angle_deg: f32, color2: [f32; 4] },  // ← already exists
}
```

`Scene::hatch_model_from_native` 已经把 DXF 的 `gradient_color` +
`pattern_angle` + `color2` 正确填进 `HatchPattern::Gradient`，GPU 着色
器按 `mode=2` 走 linear-gradient 路径，显示正确。

### 1.2 SVG 导出（简化）

```rust
// src/io/svg_export.rs, emit_hatch_fill 内部
HatchPattern::Gradient { .. } => {
    // Simplified: solid fill with base color for now.
    svg.push_str("<polygon fill=\"");
    svg.push_str(&fill_color);
    svg.push_str("\" stroke=\"none\" opacity=\"0.5\" points=\"");
    ...
}
```

问题：半透明基色 polygon ≠ 真实渐变。双色信息 (`color` → `color2`)
完全丢失，`angle_deg` 也忽略。

### 1.3 PDF 导出（完全跳过）

```rust
// src/io/pdf_export.rs
HatchPattern::Pattern(_) | HatchPattern::Gradient { .. } => {
    // Gradient is still Phase 3;  pattern gets Phase 2 line-family
    // emission above when `hatch_patterns == true` — otherwise we
    // silently skip to keep Phase 1 semantics available.
}
```

渐变 HATCH 在 PDF 里完全不可见，有无该实体字节数几乎等长。

---

## 2. 范围

| 纳入 | 优先级 | 预估 |
|------|-------|------|
| T1 SVG：emit `<defs>` + `<linearGradient>` per gradient hatch，polygon 改 `fill="url(#grad_N)"` | P0 | 1.0 h |
| T2 PDF：`emit_hatch_gradient_strips()` — boundary AABB 沿 `angle_deg` 切成 N 条平行带，每条纯色填充，颜色线性插值 color→color2 | P0 | 1.5 h |
| T3 `PdfExportOptions::gradient_hatches: bool`（默认 `true`）+ `SvgExportOptions::gradient_hatches: bool`（对称）—— toggle `false` 回退到既有的"半透明基色"/"skip"行为 | P0 | 0.3 h |
| T4 回归测试：SVG / PDF 各 2-3 条 fixture；toggle off 和 on 两个路径都验 | P0 | 1.0 h |
| T5 GUI 对话框接入新 flag（SVG + PDF） | P1 | 0.4 h |
| T6 CHANGELOG + plan + commit | P0 | 0.2 h |

**不纳入**：
- PDF Shading Pattern（`sh` 操作符 + `/Pattern` resource）——需要研究
  printpdf 0.9.1 的 Shading API 覆盖度，scope 不确定；strip-fill 的
  视觉效果对高/中 DPI 打印足够
- Radial gradient（DXF 也支持但用例少）
- Gradient inside pattern hatch（分层 fill）
- Gradient 应用于非 hatch 实体

---

## 3. 设计

### 3.1 SVG

每个 gradient hatch 生成一个 `<defs>` 项 + 一个 `<linearGradient>`：

```xml
<defs>
  <linearGradient id="grad_0" gradientUnits="userSpaceOnUse"
      x1="..." y1="..." x2="..." y2="...">
    <stop offset="0" stop-color="..." />
    <stop offset="1" stop-color="..." />
  </linearGradient>
</defs>
<polygon fill="url(#grad_0)" stroke="none" points="..." />
```

`x1,y1,x2,y2` 由 boundary AABB 沿 `angle_deg` 方向推导——最简单取 AABB
中心 ± 半对角线投影，让 gradient 恰好覆盖整个 boundary。gradient 的
id 用 `grad_{index}` 顺序递增。`monochrome=true` 时两 stop 都走灰度
调色板。

### 3.2 PDF（strip-fill）

`emit_hatch_gradient_strips` 流程：

```
1. 计算 AABB
2. 沿 angle_deg 求 AABB 的 perp 轴范围 [t_min, t_max]
3. 分成 N = 32 / 48 条平行 strip（单条宽度 = (t_max - t_min) / N）
4. 每条 strip ← boundary AABB 相交，用 Op::DrawPolygon 填充，颜色
   按 (t - t_min) / (t_max - t_min) 插值 (color, color2)
5. 安全网：N 固定上限，极扁 boundary 下 strip 数不会爆
```

简化 trade-off：
- 裁剪到 AABB 而不是 boundary polygon 本身（非凸 boundary 会有少量
  溢出）——与 Phase 2a 的 pattern 扫线采取一致策略
- N 足够大 (32-48) 肉眼已经看不出 strip 边界，满足 300 DPI 打印

### 3.3 新 Options 字段

```rust
pub struct PdfExportOptions {
    // ... existing ...
    /// Emit HatchPattern::Gradient as strip-fill polygons (三十九轮).
    /// Default true; toggle false to revert to silent-skip.
    pub gradient_hatches: bool,
}

pub struct SvgExportOptions {
    // ... existing ...
    /// Emit HatchPattern::Gradient as `<linearGradient>` (三十九轮).
    /// Default true; toggle false to revert to half-opaque base color.
    pub gradient_hatches: bool,
}
```

两个 field 都 `#[serde(default)]` 下生效（R38 已经给 struct-level
`#[serde(default)]` 了）。

### 3.4 Monochrome 行为

- `monochrome = true`：两端颜色都强制走灰度（取 luminance，或简单黑→浅灰）
- `monochrome = false`：保真两端颜色

---

## 4. 测试

### SVG（`src/io/svg_export.rs` unit tests）

- `fixture_svg_gradient_emits_linear_gradient_defs` — boundary + gradient
  hatch 渲染后 SVG 包含 `<linearGradient id="grad_0"` 和 `fill="url(#grad_0)"`
- `fixture_svg_gradient_toggle_off_matches_legacy_half_opacity` —
  `gradient_hatches=false` 时 SVG 里没有 `<linearGradient>`、回退到
  `opacity="0.5"` 字面量

### PDF（`src/io/pdf_export.rs` unit tests）

- `fixture_pdf_gradient_strips_increase_byte_length` — 空 doc 的 PDF
  字节数 vs 含一个 gradient hatch 的 PDF：后者至少多 500 B（strip 数
  足够产生可观察字节增长）
- `fixture_pdf_gradient_toggle_off_matches_empty` — `gradient_hatches=false`
  时字节数和空 PDF 等长
- `fixture_pdf_gradient_monochrome_forces_greyscale_stops` — 白盒：
  `monochrome=true` 时第一条和最后一条 strip 的填充颜色都落在 r=g=b
  的灰度分支

### 集成（保守不动 `tests/cli_batch_export.rs`）

不新增，既有 fixture 已经跑 `export_pdf_full` / `export_svg_full` 端到端。

---

## 5. 验收

```bash
cargo check -p H7CAD                        # 零新 warning
cargo test --bin H7CAD io::pdf_export       # 18 → 21+ 全绿
cargo test --bin H7CAD io::svg_export       # +2 全绿
cargo test --bin H7CAD                      # 421 → 426+ 全绿
cargo test --test cli_batch_export          # 10 / 10（不变）
```

手动：用 iced GUI 的 `GRADIENT` 命令画一个 LINEAR hatch，`PLOT` 出 PDF
+ `EXPORT SVG` 分别打开，应能看到从 `color` 到 `color2` 的线性渐变（不
再是纯色/空白）。

---

## 6. 状态

- [x] 计划定稿（2026-04-25）
- [x] T1 SVG `<linearGradient>` with `gradientUnits=userSpaceOnUse`
- [x] T2 PDF `emit_hatch_gradient_strips` 48 条 strip-fill（monochrome 走灰度 ramp）
- [x] T3 `SvgExportOptions.gradient_hatches` + `PdfExportOptions.gradient_hatches`（默认 true）
- [x] T4 回归测试：PDF 3 + SVG 2 全绿，bin 421→426
- [ ] ~~T5 GUI 对话框接入~~（P1，延到下轮与其他 dialog 拓展合并）
- [x] T6 CHANGELOG + commit
