# ARC_DIMENSION + LARGE_RADIAL_DIMENSION bridge 收口（三十四轮）

> **起稿**：2026-04-25（第三十四轮）
> **前置**：三十一轮 2D 显示收口明确把 HELIX / ARC_DIMENSION /
> LARGE_RADIAL_DIMENSION 列为「本轮不要求」，三十二-三十三轮做完了
> PDF 导出 Phase 1 + Phase 2，现在回头补这两个 dimension variants。
> **目标**：关掉 R31 覆盖矩阵中 `ARC_DIMENSION` / `LARGE_RADIAL_DIM`
> 两行的「Bridge→compat ❌ / 默认显示 ❌」空格。

---

## 1. 现状

| 实体 | Read | Write | Bridge→compat | 默认显示 |
|------|------|-------|---------------|---------|
| ARC_DIMENSION | ✅ | ✅ | ❌（走 `_ => None`） | ❌ 无 wire |
| LARGE_RADIAL_DIMENSION | ✅ | ✅ | ❌ | ❌ |

SVG 导出端已经能用 `emit_text_entities` 给两类 dim 输出尺寸文字，
但几何部分（引线 / 箭头 / 文字定位）完全依赖 `WireModel`——bridge 把
他们丢到 `None` 后 compat 文档就没有这两类实体，scene tessellate 也
就产不出 wire。

`acadrust::Dimension` 枚举不含专门的 ArcLength / LargeRadial 变体，
只有 Linear / Aligned / Radius / Diameter / Angular2Ln / Angular3Pt /
Ordinate 七种。所以 bridge 必须选一种 **最接近的近似变体** 让显示
正确，牺牲一点 compat 侧的精确语义。

---

## 2. 方案

**bridge 近似映射**（只影响显示路径，不影响 save）：

| native variant | compat 近似 | 字段对应 |
|---------------|-----------|---------|
| `ArcDimension { first_point, second_point, arc_center, text_midpoint, measurement, .. }` | `ar::Dimension::Angular3Pt` | `angle_vertex := arc_center`, `first_point := first_point`, `second_point := second_point`, `definition_point := text_midpoint` |
| `LargeRadialDimension { definition_point, chord_point, text_midpoint, leader_length, measurement, .. }` | `ar::Dimension::Radius` | `angle_vertex := text_midpoint`（圆心的视觉对应）, `definition_point := definition_point`（半径端点）, `leader_length := leader_length` |

**不做反向转换**：`acadrust_entity_to_native` 不识别这两个变体——
compat 侧编辑回写时会退化成普通 `nm::EntityData::Dimension`。这是
R31 确立的「显示为先，保存为真源」模式的显式 trade-off，不做静默
破坏（不 panic、不丢数据），只是编辑后 ArcDimension 的
arc_center 这类专有字段不保存。Native 层保留的原 ArcDimension 数据
在 save_dxf 时继续走 native 写回，未编辑的实体保真度 100%。

---

## 3. 改动面

- `src/io/native_bridge.rs`：在 `native_entity_to_acadrust` 大 match
  里新增两个 arm（~40 行）
- 新增 2 个 fixture test in `src/scene/mod.rs #[cfg(test)] mod tests`：
  - `fixture_arc_dimension_bridges_and_produces_wires`
  - `fixture_large_radial_dim_bridges_and_produces_wires`
- CHANGELOG + commit + push

**不**改动：
- DXF reader / writer（已经绿）
- native_model 定义（已经存在）
- scene 主逻辑（只依赖 bridge 输出）

---

## 4. 验收

```bash
cargo check -p H7CAD              # 零新 warning
cargo test --bin H7CAD            # 394 → 396 全绿（+2 fixture）
```

fixture 断言：
1. 构造 native doc 塞一个 `ArcDimension` 实体
2. 调 `native_doc_to_acadrust` 得到 compat doc
3. compat doc 的 entities 里存在 1 个 Dimension entity（handle 相等）
4. 用该 compat doc 构建 Scene，`entity_wires()` 非空（证明有 wire
   进了显示管线）
5. 同样流程对 LargeRadialDimension

---

## 5. 不在本轮

- HELIX：3D entity，需要 project-to-XY 投影算法；延到 3D 收口
  轮次（R35+ 按需做）
- ArcLength / LargeRadial 的 roundtrip（写 → 读 → 桥）：写侧已验证
  (R31)，读侧已验证，桥现在补齐；合起来天然形成 roundtrip，额外
  测试不必要

---

## 6. 状态

- [x] 计划定稿（2026-04-25）
- [ ] T1 bridge ArcDimension → Angular3Pt
- [ ] T2 bridge LargeRadialDimension → Radius
- [ ] T3 fixture 测试
- [ ] T4 CHANGELOG + commit + push
