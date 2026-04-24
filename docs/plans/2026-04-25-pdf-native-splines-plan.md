# PDF 原生 Spline piecewise bezier（三十五轮）

> **起稿**：2026-04-25（第三十五轮）
> **前置**：三十二-三十三轮 PDF 导出已做完 text / hatch / image /
> 原生曲线(Circle/Arc/Ellipse) / 对话框；三十四轮把
> ARC_DIMENSION / LARGE_RADIAL_DIMENSION bridge 收了。现在补 PDF
> Phase 3 里的 Spline piecewise bezier。
> **目标**：把有 SPLINE 实体的 DXF 导出为 PDF 时，让 SPLINE 走
> 原生 cubic/quadratic bezier 路径，而不是 wire tessellation。

---

## 1. 现状

`src/io/svg_export.rs` 里已经有一套完整的 NURBS → piecewise Bezier
算法：

- `fn bspline_to_bezier(degree, knots, control_points)` — Boehm 节点
  插入，把 clamped 非有理 degree 2/3 B-spline 转成 flat 控制点列表
  （长度 = `segments * degree + 1`）
- `fn spline_emit_strategy(...)` → `enum SplineEmit { ControlPoly,
  Bezier { degree, control_points }, FitPoly }`：给出一个 DXF Spline
  最佳的原生输出策略
- `fn collect_emittable_spline_handles(doc)`：返回 wire 层应该跳过的
  spline handle 集合

这三个是 `fn`（module-private），PDF 端完全可以复用——需要把它们
升到 `pub(crate)`。

PDF 端目前对 SPLINE **没有原生输出**——全部依赖 wire tessellation。
放大打印时会看到折线锯齿，而且字节数偏大。

---

## 2. 范围

| 纳入 | 优先级 | 预估 |
|------|-------|------|
| T1 把 `bspline_to_bezier` / `spline_emit_strategy` / `SplineEmit` / `collect_emittable_spline_handles` 提升到 `pub(crate)` | P0 | 0.2 h |
| T2 新增 `PdfExportOptions::native_splines: bool`（默认 true） | P0 | 0.1 h |
| T3 `emit_pdf_native_splines()` — ControlPoly / Bezier / FitPoly 三路输出 | P0 | 1.0 h |
| T4 `collect_native_handles` 扩展为四元组 `(text, image, curve, spline)`，wire 层跳过 spline handle | P0 | 0.2 h |
| T5 3 个 fixture 测试（degree 1 控制多边形 / degree 3 cubic bezier / fit-poly 回退） | P0 | 0.4 h |
| T6 CHANGELOG + commit + push | P0 | 0.1 h |

---

## 3. T3 设计：PDF 端三路输出

### SplineEmit::ControlPoly（degree = 1）

直接画控制点折线：

```rust
Op::DrawLine {
    line: Line {
        points: control_points.iter().map(|p| LinePoint { p, bezier: false }).collect(),
        is_closed: closed,
    }
}
```

### SplineEmit::Bezier { degree, control_points }（clamped 非有理 degree 2/3）

`control_points` 长度 = `segments * degree + 1`，segment `s` 占
`[s*degree..=s*degree + degree]`。

**Degree 3（cubic）**：每段 4 个控制点 `[P0, C1, C2, P3]`，直接映射
PDF cubic bezier：`P0` 为 anchor，`C1/C2` bezier=true，`P3` 为下一
anchor。

**Degree 2（quadratic）**：PDF 没有 quadratic bezier 操作符，只有
cubic。标准转换公式：对 `[Q0, Q1, Q2]` 构造等效 cubic
`[P0, C1, C2, P3]`：

```
P0 = Q0
C1 = Q0 + (2/3) * (Q1 - Q0)
C2 = Q2 + (2/3) * (Q1 - Q2)
P3 = Q2
```

这能精确复现原 quadratic 曲线（不是近似）。

### SplineEmit::FitPoly

fit_points 直接画折线：与 ControlPoly 同样的输出，但使用 fit_points
作为 anchors。

---

## 4. T4 `collect_native_handles` 扩展

当前签名：

```rust
fn collect_native_handles(native_doc, options) -> (HashSet, HashSet, HashSet)
// (text, image, curve)
```

三十五轮扩成四元组：

```rust
fn collect_native_handles(native_doc, options)
    -> (HashSet, HashSet, HashSet, HashSet)
// (text, image, curve, spline)
```

`emit_wires` 签名相应加一个 `skip_spline_handles: &HashSet<String>`
参数，wire 遍历时 skip。

---

## 5. T5 验收 fixture

- `fixture_pdf_spline_degree_1_emits_control_polyline`：构造 degree
  1 spline + 3 control points，`collect_native_handles` 把 handle 纳
  入 spline 集合
- `fixture_pdf_spline_cubic_emits_bezier_path`：构造 clamped degree 3
  spline（4 control points + clamped knots），`spline_emit_strategy`
  返回 `Bezier { degree: 3, .. }`；PDF 字节比只有 wire tessellation
  版本大至少若干字节（证明原生路径走通）
- `fixture_pdf_spline_rational_falls_back_to_wire`：构造带 weights
  （非 uniform）的 spline → `collect_emittable_spline_handles` 返回
  空集 → wire 路径继续画出

---

## 6. 验收

```bash
cargo check -p H7CAD                 # 零新 warning
cargo test --bin H7CAD io::pdf_export      # 15 → 18 全绿
cargo test --bin H7CAD                     # 不回归
```

---

## 7. 状态

- [x] 计划定稿（2026-04-25）
- [ ] T1 expose svg_export helpers
- [ ] T2 PdfExportOptions::native_splines
- [ ] T3 emit_pdf_native_splines
- [ ] T4 collect_native_handles 扩四元组
- [ ] T5 3 个 fixture
- [ ] T6 CHANGELOG + commit + push
