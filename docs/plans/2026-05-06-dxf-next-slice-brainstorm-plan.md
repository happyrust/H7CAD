# DXF 下一开发切片 Brainstorm 方案

> **日期**: 2026-05-06
> **承接**: `2026-05-06-dxf-native-integration-brainstorm-plan.md`
> **当前已完成**: DXF diagnostics、native render stats、native Ellipse/Spline 支持、Ellipse OCS extrusion 修复、LwPolyline OCS 验证、workspace test/check 通过。
> **目标**: 选择下一组小而硬的 DXF 显示保真任务，避免盲目扩大 scope。

---

## 1. 当前基线

DXF 文件层已经比较稳：

- `h7cad-native-dxf` 能读写主流 DXF section。
- `h7cad-native-model::CadDocument` 是 native 真源。
- 保存优先走 native store。
- `cargo test --workspace` 和 `cargo check --workspace` 已通过。

运行时显示层也刚补了两个关键点：

- `Scene::native_render_stats` 可区分 native-rendered / compat-fallback / preserved-only。
- native `Ellipse` 已使用 `entity.extrusion`，避免 tilted ellipse 被压回 XY 平面。

下一步不该再“看到实体就加到 native supported list”。正确方向是：选择一个用户可见的 fidelity gap，先写 regression，再最小修复。

---

## 2. Brainstorm 候选

### 候选 A: Dimension dimstyle 显示参数

问题：

DXF 标注实体不只靠自身点位，还依赖 `DIMSTYLE` 和 header/default dim variables。若 tessellation 里硬编码箭头、延伸线偏移、文字大小等参数，真实工程图会出现“标注能显示但比例不对”的问题。

用户价值：

- 工程图里标注很常见。
- 错误尺寸、箭头、延伸线会直接影响可读性。
- 比起少见实体，Dimension 修正更容易被用户感知。

风险：

- 需要确认当前 native dimension tessellation 已消费哪些 dimstyle 字段，避免重复实现。
- 需要设计 fallback：缺失 dimstyle 时使用 `Standard`，再缺失时用安全默认值。

适合 TDD：

- 构造一个 native doc，写入 `DimStyleProperties`。
- 构造同一条 `Dimension`，只改变 `dimasz` / `dimscale`。
- 断言 tessellated arrow 或 text wire 尺寸发生预期变化。

### 候选 B: nested ByBlock inheritance

问题：

嵌套块中 `ByBlock` 颜色、线型、线宽应沿 INSERT 链逐层继承。单层 ByBlock 已有支持，但三层或中间覆盖时容易断链。

用户价值：

- 工程图常用 block symbol。
- 一旦继承错，颜色/线型看起来会整体不对。

风险：

- 会触碰 `render_style`、INSERT explode、tessellation 递归。
- 影响范围比 Dimension 大。

适合 TDD：

- A -> B -> C -> LINE(ByBlock) 三层嵌套。
- A 红色，B/C ByBlock，最终 LINE 应红。
- A 红色，B 蓝色，C ByBlock，最终 LINE 应蓝。

### 候选 C: Hatch / Leader / MLine OCS regression

问题：

这些实体也可能受 normal/extrusion 或 block transform 影响，但当前优先级不如 Dimension 和 ByBlock 明确。

用户价值：

- Hatch 对图纸显示重要。
- Leader/MLine 对标注辅助重要。

风险：

- Hatch 内部边界类型多，先做可能变成大任务。
- Leader/MLine 的 DXF 坐标语义需要再确认，不能直接假设是 OCS。

---

## 3. 推荐选择

推荐下一切片做 **Dimension dimstyle 显示参数**。

原因：

1. 用户价值高：标注错比颜色继承错更影响工程图阅读。
2. 变更边界相对清晰：集中在 native dimension tessellation 和 dimstyle lookup。
3. 测试可控：构造小 native doc，不依赖复杂真实 DXF fixture。
4. 不会破坏当前 compat fallback 策略。

nested ByBlock inheritance 排第二。它价值高，但递归链和 INSERT explode 影响面更大，适合 Dimension 小切片之后再做。

---

## 4. Dimension 切片详细设计

### 4.1 目标

让 native dimension tessellation 消费 render-relevant dimstyle 字段，至少覆盖：

- `dimscale`
- `dimasz`
- `dimexo`
- `dimexe`
- `dimtxt`

### 4.2 不做

本切片不做：

- 所有 dimension 类型重写。
- 完整 AutoCAD 标注布局兼容。
- UI dimstyle 编辑面板。
- DWG 侧额外字段解析。

### 4.3 文件

主要文件：

- `src/scene/tessellate.rs`
- `src/scene/render.rs`
- `crates/h7cad-native-model/src/lib.rs`

辅助测试位置：

- `src/scene/tessellate.rs` 现有 tests
- 必要时新增 `src/scene/dimension_style.rs`，但默认先不加文件，避免过早抽象。

### 4.4 数据流

1. `tessellate_native_dimension(document, handle, entity, ...)` 拿到 `entity`。
2. 从 `entity.data` 取 `style_name` 或等价 dimstyle 字段。
3. 通过 `document.dim_styles.get(style_name)` 查 dimstyle。
4. fallback 顺序：
   - entity 指定 style
   - `Standard`
   - `DimStyleProperties::new("Standard")` 等价默认值
5. 计算有效参数：
   - `effective_scale = dimstyle.dimscale.max(epsilon)`
   - `arrow_size = dimstyle.dimasz * effective_scale`
   - `text_height = dimstyle.dimtxt * effective_scale`
   - `extension_offset = dimstyle.dimexo * effective_scale`
   - `extension_extend = dimstyle.dimexe * effective_scale`

### 4.5 TDD 测试

先写 RED：

1. `tessellate_native_dimension_uses_dimstyle_arrow_size`
   - 构造两个 dimstyle：`SmallArrow` 和 `LargeArrow`。
   - 同样的 linear dimension，分别绑定 style。
   - 断言 arrow wing length 不同，且 Large > Small。

2. `tessellate_native_dimension_uses_dimstyle_text_height`
   - 已有类似测试时复用/扩展。
   - 断言 text wire bbox 高度随 `dimtxt * dimscale` 变化。

3. `tessellate_native_dimension_falls_back_to_standard_style`
   - entity 引用不存在 style。
   - `Standard` 存在且 dimtxt/dimasz 被设置。
   - 断言使用 Standard 参数，不 panic。

4. `tessellate_native_dimension_missing_standard_uses_safe_defaults`
   - 删除 `Standard`。
   - 断言 tessellation 仍有 wire，不 panic。

### 4.6 实现顺序

1. 搜索并阅读当前 `tessellate_native_dimension`。
2. 写第一个 RED 测试：arrow size。
3. 抽一个小 helper：
   - `native_dimension_style(document, entity) -> DimensionRenderStyle`
4. 只接入箭头尺寸，跑 GREEN。
5. 再写 text height / fallback 测试。
6. 若 helper 变大，再考虑拆到 `dimension_style.rs`。

执行进度：

- 已新增 `tessellate_native_dimension_uses_native_dimstyle_arrow_size`。
- 已让 `native_dimension_geometry` 接收 `native_document`。
- 已接入 `dimasz * dimscale`、`dimexo * dimscale` 和 `dimexe * dimscale`。
- 已扩展 `h7cad-native-model::DimStyleProperties` 增加 `dimexe`。
- 已修正 DIMSTYLE 表 code 映射：`44 = DIMEXE`，`147 = DIMGAP`。
- `cargo test -p h7cad-native-dxf --test entity_2d_roundtrip` 通过 38 个测试。

---

## 5. 验收命令

```powershell
cargo test -p H7CAD scene::tessellate::tests::tessellate_native_dimension
cargo test -p H7CAD scene::acad_to_truck
cargo test -p h7cad-native-dxf --test entity_2d_roundtrip
cargo check --workspace
```

若修改公共结构或 dimstyle model，再跑：

```powershell
cargo test --workspace
```

---

## 6. 回滚边界

如果 Dimension helper 牵连过大，立即停止在测试层，不继续改实现。

可安全保留：

- RED 测试
- 文档

必须回滚：

- 半成品 helper
- 影响非 Dimension tessellation 的抽象
- 任何让 existing dimension tests 变红的变更
