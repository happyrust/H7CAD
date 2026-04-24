# PDF 导出与 SVG 导出功能对齐计划（三十二轮）

> **起稿**：2026-04-24（第三十二轮）
> **前置**：三十轮落地 `export_svg_full`（ODA OdSvgExportEx 端到端移植）+ SVG 导出对话框；三十一轮关闭常规二维 DXF 读-写-桥接-显示缺口，`src/scene/*` 的默认显示路径已全绿。
> **目标**：把 `src/io/pdf_export.rs` 从 191 行的"只能画线"原型升级到可与 SVG 导出 **产品特性对齐**（文字、填充、图像、对话框），让"Export as PDF"与"Export as SVG"都能导出一张可直接交付的图。

---

## 1. 现状对比

| 能力 | SVG (`export_svg_full`) | PDF (`export_pdf`) |
|------|------------------------|---------------------|
| Wire 线段 | ✅ `<polyline>` | ✅ `Op::DrawLine` |
| 线宽 / 颜色 | ✅ 含 CTB override | ✅ 含 CTB override |
| 纸张方向 0/90/180/270 | ✅ CTM | ✅ CTM |
| 白色背景 / 边界抑制 | ✅ | ✅ |
| **TEXT / MTEXT 文字** | ✅ native `<text>` + font fallback + 字高换算 | ❌ 依赖 wire tessellation，字体信息丢失 |
| **HATCH solid 填充** | ✅ `<polygon fill>` | ❌ 不输出 |
| **HATCH pattern 填充** | ⚠️ 部分（line 族路径，pattern 参考 svg_export line 2000 附近）| ❌ 不输出 |
| **IMAGE (RasterImage)** | ✅ `<image>` + base64 / 外链 / ImageBase | ❌ 不输出 |
| 导出选项对话框 | ✅ `src/ui/svg_export_dialog.rs`（12 个字段）| ❌ 无，直接走 rfd save-file 流程 |
| 块定义 `<defs>` + `<use>` | ✅ `use_block_defs` | ❌ 扁平输出 |
| 原生曲线 (circle/ellipse/arc) | ✅ `native_curves` | ❌ tessellation only |
| 原生 Spline | ✅ `native_splines` | ❌ |
| Dim 文字原生渲染 | ✅ `native_dimension_text` | ❌ |

**现实差距**：SVG 导出可直接交付工程图（文字可选可搜、图像清晰、矢量箭头/曲线）；PDF 导出目前只能出"线条骨架"——打开后看不到尺寸文字、填充、插入的图片，实际使用价值低。

---

## 2. 本轮范围（Phase 1）

挑 **用户视觉感知最强** 的三项缺口 + 对话框基础设施：

| 本轮纳入 | 优先级 | 预估 | 理由 |
|---------|-------|------|------|
| **T1 PDF 文字渲染** (TEXT/MTEXT) | P0 | 1.0 h | 工程图无文字不成立（尺寸标注、标题栏、图层名） |
| **T2 PDF solid HATCH** 填充 | P0 | 0.8 h | 剖面/涂实体在工程图普遍存在，空白无法打印 |
| **T3 PDF IMAGE 嵌入** | P1 | 0.8 h | logo / 底图 / 扫描件等真实图纸都会带 |
| **T4 `PdfExportOptions` + 导出对话框** | P1 | 1.0 h | 统一的导出前端，为后续 Phase 2/3 铺路 |
| **T5 回归 fixture + 测试** | P0 | 0.8 h | 每个能力都要可回归、可复现 |
| **T6 CHANGELOG + 提交 + 推送** | P0 | 0.2 h | 收口 |

**后续 Phase 2**（不在本轮，仅列入路线图）：
- T+1 HATCH pattern（line family）/ gradient
- T+2 Block `<defs>` + `<use>` 等价物（PDF Form XObject）
- T+3 原生圆/椭圆/Arc（PDF path with bezier approximation）
- T+4 原生 Spline（piecewise bezier，复用 `svg_export` 已有 NURBS→Bezier 代码）
- T+5 Native dim-text（与 `native_dimension_text` 对齐）

---

## 3. 架构：数据源对齐 SVG export

SVG export 的入参：

```rust
pub fn export_svg_full(
    wires: &[WireModel],
    hatches: &HashMap<Handle, HatchModel>,
    native_doc: Option<&nm::CadDocument>,
    paper_w: f64, paper_h: f64,
    offset_x: f32, offset_y: f32,
    rotation_deg: i32,
    path: &Path,
    plot_style: Option<&PlotStyleTable>,
    options: &SvgExportOptions,
) -> Result<(), String>
```

`export_pdf` 升级到相同形参，新增 `hatches` / `native_doc` / `options`。

**新增** `PdfExportOptions`（`src/io/pdf_export.rs`）：

```rust
#[derive(Clone, Debug)]
pub struct PdfExportOptions {
    pub monochrome: bool,                 // 默认 true（工程打印惯例）
    pub text_as_geometry: bool,           // 默认 false（使用内置字体）
    pub font_family: PdfFontChoice,       // Helvetica / TimesRoman / Courier
    pub font_size_scale: f32,             // 默认 0.8（对齐 SVG）
    pub include_hatches: bool,            // 默认 true
    pub include_images: bool,             // 默认 true
    pub embed_images: bool,               // 默认 true（PDF 自包含）
    pub image_base: Option<PathBuf>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PdfFontChoice { Helvetica, TimesRoman, Courier }
```

`Default` 与 `SvgExportOptions::default()` 语义对齐。

---

## 4. T1 · PDF 文字渲染（TEXT / MTEXT）

### 设计

PDF 渲染文字的方式有两条：

1. **使用 printpdf 内置 Standard 14 字体**（Helvetica / Times / Courier 等）
   - 优点：无需嵌入字体，体积小，兼容所有 viewer
   - 缺点：中文字形不保证（ASCII + Latin-1 安全，CJK 需嵌入 TTF）
2. **嵌入自定义 TTF 字体**（运行时动态加载）
   - 超出 Phase 1 范围

**本轮策略**：先走 (1) 用 Helvetica，把字符 → PDF 字形索引的路径打通；中文字符走 fallback = 继续 wire tessellation（与 `text_as_geometry=true` 同等效果）。后续 Phase 2 可加 TTF 嵌入。

### 实现步骤

1. 从 `native_doc` 取出 `EntityData::Text` / `EntityData::MText`，计算 `(x, y, height, rotation, h_align, v_align, content)`
2. 对每个 entity：
   - 计算 baseline 坐标（`y + offset_y`，考虑 v_align）
   - 字高：`height * options.font_size_scale`（mm）
   - Rotation：额外 `Op::SaveGraphicsState` + CTM
   - 如果 content 含非 ASCII/Latin-1 字符 → fallback 走 wire
   - 否则发出 `Op::StartTextSection` / `Op::WriteText { text, font }` / `Op::EndTextSection`
3. `build_pdf` 保留既有 wire 绘制流程；在 wire 遍历时 **跳过 text wire**（通过 `wire.name` 前缀 `"text:"` 识别——与 scene tessellate 约定）
4. 如果 `options.text_as_geometry=true` 或字符 fallback → 保留 wire 绘制（当前路径）

### 验收

- `fixture_pdf_text_renders_as_native_text`：单行 ASCII TEXT → PDF 字节里含 `/F1` 字体引用 + `Tj` 写文字操作符
- `fixture_pdf_cjk_text_falls_back_to_geometry`：中文 TEXT → PDF 字节不含 `Tj`（证明 fallback 成功）
- `fixture_pdf_text_alignment_positions_baseline_correctly`：middle-center TEXT 输出的 `Td` 坐标距离 entity.location 不超过 font size 的 0.6 倍

---

## 5. T2 · PDF solid HATCH 填充

### 设计

`HatchModel` 的 `polygons: Vec<Vec<[f64; 2]>>` 已经是屏幕坐标的 tessellated 多边形（与 SVG fill 路径一致）。PDF 渲染：

```
Op::SetFillColor { col }
Op::MoveTo { x, y }
Op::LineTo { ... } ...
Op::ClosePath
Op::FillPath { winding_rule: EvenOdd }
```

printpdf 目前 `Op` 里没有直接的 MoveTo/LineTo——需要用 `Op::DrawPolygon` 或等价的填充路径构造。查 printpdf 0.7+ 的 API。

**Fallback**：如果 printpdf 不支持一次画多边形，可用 `Line { is_closed: true }` + `fill_color`。若 `Line` 只描边不填充，则在 wire 之前先画一层 `Op::DrawRectangle` 近似（极端 fallback）。具体 API 在实现阶段确认。

### 实现步骤

1. 在 `build_pdf` 遍历 `wires` 之前，遍历 `hatches`：
   - 对每个 `HatchModel`：
     - 解析 `hatch.color` → PDF fill color
     - 遍历 `polygons`，每个 polygon 一个 `Op::DrawPolygon { points, is_closed: true, fill: true }`
   - solid hatch 只输出一次；pattern hatch 如果 `HatchPattern != Solid` 暂时跳过（Phase 2 处理）
2. hatch 画在 wire 之下（PDF 画图顺序 = 先画的在下面）

### 验收

- `fixture_pdf_solid_hatch_emits_fill_ops`：单个 solid HATCH → PDF 字节里 `fill` 操作符出现 ≥1 次
- `fixture_pdf_pattern_hatch_skipped_when_not_implemented`：pattern HATCH → 不产生 fill，wire 边界仍画出（优雅降级）

---

## 6. T3 · PDF IMAGE 嵌入

### 设计

printpdf 支持 `ImageXObject` — 把 PNG/JPEG 字节嵌入 PDF 并在页面 `Do` 引用：

```
let image = Image::from_dynamic_image(&dyn_img);
let image_id = doc.add_image(image);
ops.push(Op::UseXObject { id: image_id, transform: ... });
```

对齐 SVG export 的 `populate_images_from_document` 行为：
- 如果 entity 是 `RasterImage` 且 `options.include_images == true`
- 根据 `image_base` 解析文件路径
- 尝试 `image::open(path)` 加载 `DynamicImage`
- 加载成功 → 嵌入 + 放置；失败 → 跳过（不阻塞导出）

### 实现步骤

1. 新增 helper `collect_pdf_images(native_doc, options)` → `Vec<PdfImageSpec>`（路径、insert/u_vec/v_vec）
2. 每个 `PdfImageSpec`：
   - load → encode → `doc.add_image`
   - 计算仿射矩阵：图像本地 (0,0)→(1,0)→(0,1) 映射到 CAD (insert)→(insert+u_vec)→(insert+v_vec)
   - 发出 `Op::UseXObject { transform: CurTransMat::Raw([a,b,c,d,tx,ty]) }`
3. 图像画在 hatch 之下（最底层）

### 验收

- `fixture_pdf_image_embeds_raster`：含 4×4 PNG 的 DXF → PDF 字节里 `/XObject` 引用 ≥1 次
- `fixture_pdf_image_missing_file_does_not_crash`：IMAGE 指向不存在文件 → PDF 导出成功，wire 部分仍在

---

## 7. T4 · `PdfExportOptions` + 导出对话框

### 设计

完全对照 `src/ui/svg_export_dialog.rs`：

- 新建 `src/ui/pdf_export_dialog.rs`（估 200 行，与 SVG 对话框同结构）
- 在 `src/app/mod.rs` 增 `Message::PdfExportDialogToggle(...)` / `Message::PdfExportDialogRun` 等变体
- 字段：monochrome / text_as_geometry / font_family (3 选 1 pill) / font_size_scale / include_hatches / include_images / embed_images
- 纸张尺寸 / 方向 / 比例 **复用 page_setup 已有状态**（不在本轮 UI 内重做）

### 实现步骤

1. `PdfExportOptions` 落在 `src/io/pdf_export.rs`（见 §3）
2. `src/ui/pdf_export_dialog.rs` 从 `svg_export_dialog.rs` copy + 改字段映射
3. `src/app/update.rs` 在原 `PdfExportPathSelected` 路径之前插入 `PdfExportDialogOpen` → 用户确认 options → 再走 rfd save-file

### 验收

- 手动路径：App → Menu → Export PDF → 弹对话框 → 点 "Export" → rfd 保存对话框
- `cargo check -p H7CAD` 绿

---

## 8. T5 · 回归 fixture + 测试

### 新增测试文件

`src/io/pdf_export.rs #[cfg(test)] mod tests`（与 svg_export 同位置）：

| fixture | 目的 |
|---------|------|
| `fixture_pdf_wire_smoke` | baseline：LINE → PDF 可解析 + 字节非空 |
| `fixture_pdf_text_renders_as_native_text` | T1 ASCII text native |
| `fixture_pdf_cjk_text_falls_back_to_geometry` | T1 CJK fallback |
| `fixture_pdf_text_alignment_positions_baseline_correctly` | T1 alignment |
| `fixture_pdf_solid_hatch_emits_fill_ops` | T2 solid hatch |
| `fixture_pdf_pattern_hatch_skipped_when_not_implemented` | T2 degrade |
| `fixture_pdf_image_embeds_raster` | T3 PNG embed |
| `fixture_pdf_image_missing_file_does_not_crash` | T3 robust |
| `fixture_pdf_options_monochrome_overrides_aci_color` | T4 options wire-up |

### 方法

- 生成 PDF bytes in-memory（不落盘）
- 用正则 / 子串搜索在 PDF 二进制里找关键 operator（`Tj` / `f` 或 `f*` / `Do`）
- 不依赖 PDF parser——ASCII 操作符足够判定

---

## 9. 验收清单（Phase 1 收口）

```bash
cargo check -p H7CAD                 # 零新 warning
cargo test --bin H7CAD io::pdf_export -- --nocapture   # 9 fixture 全绿
cargo test --workspace --quiet       # 只剩已记录的 DWG AC1015 红灯
```

UI 手动回归：
1. App → 打开一张含 TEXT/HATCH/IMAGE 的 DXF（可用 `examples` 下的 fixture）
2. Export → PDF → 对话框出现 → 选默认 → 保存
3. 在系统 PDF 阅读器打开：文字可选、填充可见、图像清晰

---

## 10. 路线图快照

```
R30 (已完成)  ── SVG export full + 对话框
R31 (已完成)  ── DXF 2D 读-写-桥-显示收口
R32 (本轮)    ── PDF export Phase 1: text + solid hatch + image + dialog
R33           ── PDF export Phase 2: pattern hatch + native curves
R34           ── PDF export Phase 3: block defs / spline native / dim-text native
R35+          ── 回到路线图 (docs/plans/2026-04-22-post-dimalt-roadmap.md):
                  Path B 实体扩展 (OLE2FRAME / GEOPOSITIONMARKER / ...)
                  Path C DWG AC1015 红灯
```

此快照非硬承诺；owner 可在任一轮后重排。

---

## 11. 状态

- [x] 计划定稿（2026-04-24）
- [ ] T1 文字渲染
- [ ] T2 solid hatch
- [ ] T3 image embed
- [ ] T4 dialog + options
- [ ] T5 fixture + 测试
- [ ] T6 CHANGELOG + 提交 + 推送
