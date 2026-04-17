# 更新日志

## [未发布]

### 2026-04-17：D3 扩展 LwVertex/LwPolyline 宽度字段 + DONUT native-first

扩 native 模型支持 LwPolyline 族宽度属性，同步解锁 DONUT 命令 native-first。

**模型层** (`crates/h7cad-native-model/src/lib.rs`)：
- `LwVertex` 追加 `start_width: f64`, `end_width: f64`（DXF code 40/41 per-vertex）
- `EntityData::LwPolyline` 追加 `constant_width: f64`（DXF code 43）

**Bridge 双向** (`src/io/native_bridge.rs`)：
- `native_lwpolyline_to_acadrust`：读 native 新字段直接写入 `ar::LwVertex.start_width/
  end_width` 和 `ar::LwPolyline.constant_width`
- `acad_lwpolyline_to_native`：从 acad 读回相同字段
- 同步更新 `src/entities/traits.rs` 的 `lwv_ar_to_nm / lwv_write_back` helpers

**DXF parser** (`crates/h7cad-native-dxf/src/entity_parsers.rs`)：
- `parse_lwpolyline` 加 code 40/41/43 支持；code 40 按位置歧义处理（10/20 之前
  记 `constant_width`，之后记 per-vertex `start_width`）

**DXF writer** (`crates/h7cad-native-dxf/src/writer.rs`)：
- `LwPolyline` 写入 code 43（`constant_width != 0.0`）、code 40（per-vertex
  `start_width != 0.0`）、code 41（per-vertex `end_width != 0.0`）

**DONUT 命令** (`src/modules/home/draw/donut.rs`)：
- `make_donut` → `make_donut_native` 返回 `nm::Entity`
- 关键宽度字段全部保真：vertices 的 `start_width=end_width=width`，polyline
  的 `constant_width=width`（填充效果依赖这些）
- `CommitEntity` → `CommitEntityNative`，移除 `use acadrust::entities::{
  LwPolyline, LwVertex}` / `use acadrust::EntityType`
- 度量：`donut.rs` 中 `acadrust::` 引用 2 → 0

**构造点同步**：REVCLOUD / SHAPES / POLYLINE / DONUT 命令，scene/dispatch.rs、
scene/acad_to_truck.rs、cmd_result.rs 测试里的 `nm::LwVertex` 和 `EntityData::
LwPolyline` 构造 / 解构全部更新。

- 测试：workspace `cargo check` 零 warning；`native_bridge` 22 个测试全绿
- **home/draw 进度 8/9**：仅剩 RASTER_IMAGE 依赖 D4

### 2026-04-17：C3d POLYLINE 命令 native-first（修正 C3c 判断）

C3c changelog 误判 polyline.rs 使用宽度字段而延后；实际 PLINE 命令只使用
`vertices + bulge + is_closed`，完全契合现有 native `EntityData::LwPolyline + LwVertex { x, y, bulge }`。本次直接迁移。

- `PlineCommand::build_entity` 签名 `Option<EntityType>` → `Option<nm::Entity>`，
内部构造 `nm::Entity::new(nm::EntityData::LwPolyline { vertices, closed })`，
per-vertex 用 `nm::LwVertex { x, y, bulge }`
- 3 个 CmdResult 出口 `CommitAndExit(e)` → `CommitAndExitNative(e)`（正常 Enter /
Escape / C/CLOSE 文本输入）
- 移除 `use acadrust::entities::LwVertex` / `use acadrust::{EntityType, LwPolyline}` /
`use crate::types::Vector2`
- 度量：`polyline.rs` 中 `acadrust::` 引用 2 → 0；主 crate 零 warning 保持

**home/draw 进度更新**：native-first **7/9** 完成（REVCLOUD / SHAPES×6 / SPLINE /
MLINE / WIPEOUT / ATTDEF / POLYLINE）；仅剩 DONUT / RASTER_IMAGE 待 D3 / D4。

### 2026-04-17：D2 扩展 native EntityData::Wipeout.elevation 字段

修复 C3b WIPEOUT 迁移遗留的 DXF Z / elevation 丢失（世界 Y 轴）。

- `crates/h7cad-native-model/src/lib.rs`：`EntityData::Wipeout` 追加 `elevation: f64`
- `src/io/native_bridge.rs`：
  - `native_wipeout_to_acadrust`：polygonal / from_corners 使用 `elevation`
  作为 `insertion_point.z`（之前硬编码为 0）
  - `acad_wipeout_to_native`：从 `wipeout.insertion_point.z` 读回 elevation
  - 5 处测试 fixture 显式设 `elevation: 0.0`
- `crates/h7cad-native-dxf/src/entity_parsers.rs`：`parse_wipeout` 解码 code 30
→ elevation
- `crates/h7cad-native-dxf/src/writer.rs`：Wipeout 写入时增加 code 10/20/30
insertion point triple，Z = elevation
- `src/modules/home/draw/wipeout.rs`：
  - `make_rect_wipeout_native`：`elevation = p1.y as f64`（世界 Y）
  - `make_poly_wipeout_native`：`elevation = pts.first().y`（与原命令语义一致）
- 测试：workspace `cargo check` 零 warning；`native_bridge` 22 个测试全绿
- 效果：WIPEOUT 矩形 / 多边形模式在 native 存储 / DXF 中保留 elevation

### 2026-04-17：D1 扩展 native EntityData::MLine.closed 字段

修复 C3b MLINE 迁移遗留的字段损失（MLineFlags::CLOSED 被丢弃）。

- `crates/h7cad-native-model/src/lib.rs`：`EntityData::MLine` 追加 `closed: bool`
字段
- `src/io/native_bridge.rs`：
  - `native_mline_to_acadrust`：`closed = true` 时调 `ar::MLine::close()`
  （设置 `MLineFlags::CLOSED` bit）
  - `acad_mline_to_native`：从 `mline.is_closed()` 读回 `closed`
  - 3 处测试 fixture 显式设 `closed: false`
- `crates/h7cad-native-dxf/src/entity_parsers.rs`：`parse_mline` 解码 code 71
flags bitfield（`CLOSED = 0x2`），提取 `closed`
- `crates/h7cad-native-dxf/src/writer.rs`：MLine 写入时写 code 71
`HAS_VERTICES (1) | CLOSED (2)` bit
- `src/modules/home/draw/mline.rs`：`build_mline_native` 的 `_closed` 参数
重新生效，直接传 `closed` 到 `EntityData::MLine`
- 测试：workspace `cargo check` 零 warning；`native_bridge` 22 个测试全绿
- 效果：MLINE Close 分支语义保真，DXF round-trip 保留 closed flag

### 2026-04-17：C3c ATTDEF 命令 native-first（home/draw 阶段收口）

- **ATTDEF** (`attdef.rs`)：`AttributeDefinition { tag, prompt, default_value, insertion_point, height, ..Default }` + `common.layer = "0"` →
`nm::Entity::new(nm::EntityData::AttDef { tag, prompt, default_value, insertion, height })`；`nm::Entity::new` 默认 `layer_name = "0"` 与原命令一致，
无需显式设置
- 移除 `use acadrust::entities::AttributeDefinition` / `use acadrust::EntityType` /
`use crate::types::Vector3`
- 度量：`attdef.rs` 中 `acadrust::` 引用 2 → 0；主 crate 零 warning 保持

**RASTER_IMAGE 延后说明**：`raster_image.rs` 需要传 `file_path`，但 native
`EntityData::Image { insertion, u_vector, v_vector, image_size }` **无 file_path 字段**
— bridge 投影时用 `ar::RasterImage::new("", ..)` 会让 path 丢失，导致图片渲染/保存
失效。列为 D 系列必须扩展字段（file_path + flags + pixel_size）之前的必要前置。

**home/draw 进度小结**：native-first **6/9** 完成
（REVCLOUD / SHAPES×6 / SPLINE / MLINE / WIPEOUT / ATTDEF）；延后 3 项待 D 系列：

- DONUT / POLYLINE（LwVertex 缺 start_width / end_width / constant_width）
- RASTER_IMAGE（Image 缺 file_path / flags / pixel_size）

### 2026-04-17：C3b SPLINE / MLINE / WIPEOUT 命令 native-first

继续 C3 系列，迁移 home/draw 里 3 个 native 字段基本对等的命令。

- **SPLINE** (`spline.rs`)：`Spline { degree, control_points, knots, ..Default::default() }` → `nm::EntityData::Spline { degree, closed: false, knots, control_points, weights, fit_points, start_tangent, end_tangent }`；
2 个 `CommitEntity` → `CommitEntityNative`
- **MLINE** (`mline.rs`)：`MLine::from_points(..) / closed_from_points(..)` +
`scale_factor` + `style_name` → `nm::EntityData::MLine { vertices, style_name, scale }`；2 个 `CommitAndExit` → `CommitAndExitNative`
  - **字段损失**：native 无 `flags/closed` 字段，Close 分支的闭合语义丢失
  （顶点不会视觉闭环）。D 系列待办：扩展 native MLine 加 closed 标志
- **WIPEOUT** (`wipeout.rs`)：`Wipeout::from_corners(c1, c2)` /
`Wipeout::polygonal(verts, z)` → `nm::EntityData::Wipeout { clip_vertices }`；
矩形模式展开为 4 个 corner 顶点，多边形模式直接复制 xy
  - **字段损失**：native Wipeout 只存 2D clip vertices，原命令传入的
  DXF Z 高度（世界 Y 轴）丢失，bridge 默认归 0。D 系列待办
- 共移除 3 处 `use acadrust::...` / `use crate::types::Vector3` / 局部 `v3`
helper；改为 `use h7cad_native_model as nm`
- 度量：`spline.rs` 1→0，`mline.rs` 2→0，`wipeout.rs` 2→0；主 crate 零 warning 保持

### 2026-04-17：C3a REVCLOUD / SHAPES 命令 native-first（LwPolyline 纯 xy+bulge）

开启 C3 系列 — home/draw 模块创建命令 native-first。首批选择**只使用 xy+bulge**
的 LwPolyline 命令（native `LwVertex { x, y, bulge }` 完整对等）：

- **REVCLOUD** (`revcloud.rs`)：`make_revcloud` → `make_revcloud_native` 返回
`nm::Entity::new(nm::EntityData::LwPolyline { vertices, closed: true })`；
1 个 `CommitAndExit` → `CommitAndExitNative`
- **SHAPES** (`shapes.rs`, 含 RECT/RECT_ROT/RECT_CEN/POLY/POLY_C/POLY_E 6 个
子命令)：`make_pline` 返回类型 `EntityType` → `nm::Entity`；6 个
`CommitAndExit(make_pline(..))` → `CommitAndExitNative(make_pline(..))`
- 移除 `use acadrust::entities::LwVertex` / `use acadrust::{EntityType, LwPolyline, entities::LwVertex}` / `use crate::types::Vector2`
- 度量：`revcloud.rs` 的 `acadrust::` 2 → 0，`shapes.rs` 的 2 → 0；主 crate 零
warning 保持

**延后说明**：`donut.rs` 和 `polyline.rs` 使用 `LwPolyline.constant_width`
和 `LwVertex.start_width/end_width`，native `EntityData::LwPolyline { vertices, closed }` 和 `LwVertex { x, y, bulge }` 无这些字段 — 强行迁移会丢失线宽特性
（尤其 DONUT 的填充效果依赖 width 字段）。列为 D 系列 "扩展 native 模型字段"
的待办，迁移前先扩充 native 模型。

### 2026-04-17：C2g-2 LEADER 命令 native-first（annotate 创建命令收官）

复用 C2g-1 新增的 `CommitManyAndExitNative` 变体，把 `leader_cmd.rs` 从
`acadrust::entities::{Leader, MText, Insert}` + `ReplaceMany` 路径切到
`nm::EntityData::{Leader, MText, Insert}` + `CommitManyAndExitNative`。

- `build_leader / build_mtext / v3` → `build_leader_native / build_mtext_native / build_insert_native`，三个构造都返回 `nm::Entity`
- 六个 CmdResult 出口：
  - `NoAnnotation`/`Tolerance` → `CommitAndExitNative(leader)`（原 `CommitAndExit`）
  - `WithText`/`WithBlock` 空注释 → `CommitAndExitNative(leader)`
  - `WithText` 有文本 → `CommitManyAndExitNative(vec![leader, mtext])`
  - `WithBlock` 有块名 → `CommitManyAndExitNative(vec![leader, insert])`
- 移除 `use acadrust::entities::{Insert, Leader, LeaderCreationType, MText}` /
`use acadrust::EntityType` / `use crate::types::Vector3`
- `LeaderCreationType` 本地枚举化为 `enum CreationChoice { None, Text, Block, Tolerance }`
（只用于命令内部的分支逻辑，不传给 entity）
- 字段损失说明：
  - native `EntityData::Leader` 仅有 `vertices + has_arrowhead`，原命令设的
  `creation_type / hookline_enabled / text_height` 无对等字段 — bridge 走
  `ar::Leader::new` 默认 (WithText / hookline=false / text_height=2.5)
  - 新增常量 `LEADER_TEXT_HEIGHT = 2.5` 替代原 `leader.text_height` 传给
  `landing_pt / build_mtext_native`，与 bridge 默认保持一致
- 度量：`leader_cmd.rs` 中 `acadrust::` 引用 3 → 0；主 crate 零 warning 保持
- **annotate 创建命令 native-first 收官**：C2b-C2g 共 13 个创建命令已全部迁完
（TEXT / MTEXT / RAY / XLINE / 7 个 DIMENSION / TOLERANCE / MLEADER / TABLE / LEADER）。
`src/modules/annotate/` 剩余 `acadrust::` 均在**编辑型**命令（DIMEDIT / QDIM /
DIMBREAK / DIMSPACE / DDEDIT / DIMTEDIT / DIMJOGLINE / MLEADER-EDIT），
属 E 系列 "Edit operations native-first" 的范围

### 2026-04-17：C2g-1 CmdResult 新增 CommitManyAndExitNative 基础设施

为 C2g LEADER native-first 迁移做准备：现有 `CmdResult::ReplaceMany(vec![], additions)`
承担「一次提交 2 个新实体（Leader + MText/Insert）」的场景，但它只吃
`Vec<acadrust::EntityType>`，没有 native 对等入口。

- `src/command/mod.rs`：`CmdResult` enum 新增
`CommitManyAndExitNative(Vec<nm::Entity>)` 变体
- `src/app/cmd_result.rs`：在 `CommitAndExitNative` 旁边加 dispatch 分支：
`push_undo_snapshot` → 循环 `native_entity_to_acadrust` + `commit_entity` →
`clear_preview_wire` / `active_cmd = None` / `snap_result = None` /
`restore_pre_cmd_tangent`；复用 layer/color/linetype 默认值逻辑
- 设计要点：**新增语义**，不替换已有 `ReplaceMany`（FILLET/CHAMFER 等仍走
acadrust 路径）；新变体仅用于 native-first 的多实体纯新增场景
- 主 crate 零 warning 保持

### 2026-04-17：C2f TABLE 命令 native-first

沿用 C2a-C2e 模式，把 `src/modules/annotate/table_cmd.rs` 的 TABLE 命令从
`acadrust::entities::TableBuilder` 构造切到 `nm::EntityData::Table`。

- `TableCommand::on_point`：`TableBuilder::new(rows, cols).at(ins).row_height(..) .column_width(..).build()` + `CmdResult::CommitAndExit(EntityType::Table(..))` →
`nm::Entity::new(nm::EntityData::Table { num_rows, num_cols, insertion, horizontal_direction, version, value_flag })` + `CmdResult::CommitAndExitNative(entity)`
- 移除 `use acadrust::entities::TableBuilder` / `use acadrust::EntityType` /
`use crate::types::Vector3`
- `ROW_HEIGHT=0.5` / `COL_WIDTH=2.0` 常量**保留用于预览线框**，但不再传入实体
构造；native 路径下 bridge 走 `acadrust::Table::new(..)`，每行/列走
`TableRow/Column::new()` 默认 `0.25 / 2.5`。已有行为差异，bridge 层需要扩展
`EntityData::Table` 增加 `row_height/column_width` 字段才能保真（记为 TODO）
- 度量：`table_cmd.rs` 中 `acadrust::` 引用 3 → 0；主 crate 零 warning 保持

### 2026-04-17：C2e MLEADER 命令 native-first

沿用 C2a-C2d 模式，把 `src/modules/annotate/mleader_cmd.rs` 的 MLEADER 命令从
`acadrust::entities::MultiLeader` 构造切到 `nm::EntityData::MultiLeader`。

- `MLeaderCommand::on_text_input`：`MultiLeader::with_text(..)` +
`CmdResult::CommitAndExit(EntityType::MultiLeader(..))` →
`nm::Entity::new(nm::EntityData::MultiLeader { .. })` +
`CmdResult::CommitAndExitNative(entity)`
- `build_mleader` → `build_mleader_native` 直接构造 native：
  - `verts` 的最后一点作为 `text_location`，前面的点作为 `leader_vertices`
  - `leader_root_lengths = vec![leader_vertices.len()]`（单 root）
  - 默认值对齐 bridge `acad_multileader_to_native` 的反向映射：
  `content_type=1` (MText) / `path_type=1` (Straight) / `style_name="Standard"` /
  `scale_factor=1.0` / `leader_line_weight=-1` / `enable_landing=true` /
  `enable_dogleg=true` / `text_attachment_type=9`
  - 保留原命令的 `arrowhead_size=2.5` / `dogleg_length=2.5`
- 移除 `use acadrust::entities::MultiLeader` / `use acadrust::EntityType` /
`use crate::types::Vector3` / 本地 `fn v3(..)` helper
- 字段损失说明：原命令通过 `ml.context.leader_roots[0]` 设置的
`direction/connection_point/landing_distance` 在 native 模型中无对等字段，
bridge 会走默认值；渲染 / DXF / DWG 正常不受影响
- 度量：`mleader_cmd.rs` 中 `acadrust::` 引用 3 → 0；主 crate 零 warning 保持

### 2026-04-17：C2d TOLERANCE 命令 native-first

沿用 C2a/C2b/C2c 模式，把 `src/modules/annotate/tolerance_cmd.rs` 的 TOLERANCE
命令从 `acadrust::entities::Tolerance` 构造切到 `nm::EntityData::Tolerance`。

- `ToleranceCommand::on_point`：`Tolerance::with_text(ins, text)` +
`CmdResult::CommitAndExit(EntityType::Tolerance(..))` →
`nm::Entity::new(nm::EntityData::Tolerance { text, insertion })` +
`CmdResult::CommitAndExitNative(entity)`
- 移除 `use acadrust::entities::Tolerance` / `use acadrust::EntityType` /
`use crate::types::Vector3`；只 `use h7cad_native_model as nm`
- `insertion` 坐标沿用 `[pt.x, pt.z, pt.y]`（Y↔Z 翻转与其它命令一致）
- `native_bridge` 已有 Tolerance 分支，CommitAndExitNative handler 自动走投影路径
- 度量：`tolerance_cmd.rs` 中 `acadrust::` 引用 3 → 0

### 2026-04-17：C2c DIMENSION 家族 7 个命令 native-first

把 `src/modules/annotate/` 里 7 个 dimension 命令从 `acadrust::Dimension::{ Linear,Aligned,Radius,Diameter,Angular3Pt,Ordinate}` 切到
`nm::EntityData::Dimension { dim_type, .. }` 构造。

涉及文件：

- `linear_dim.rs`（dim_type=0）
- `aligned_dim.rs`（dim_type=1）
- `diameter_dim.rs`（dim_type=3）
- `radius_dim.rs`（dim_type=4）
- `angular_dim.rs`（dim_type=5）
- `ordinate_dim.rs`（dim_type=6，X/Y 方向通过 `dim_type & 0x40` 位标记）
- `dim_continue.rs` / `dim_baseline.rs`（dim_type=0 链式/基线）

**设计要点**：

- nm 用单一变体 + `dim_type` (i16) 区分 7 种 sub-type，字段涵盖
definition_point / text_midpoint / first_point / second_point / angle_vertex /
dimension_arc / leader_length / rotation (degrees) / ext_line_rotation (degrees)
- `measurement` 字段由命令侧自行计算（Linear/Aligned 用投影距离，Radius 用圆心-点距离，
Diameter 用 2×半径，Angular3Pt 用向量夹角度数，Ordinate 置 0）
- Radius/Diameter：`angle_vertex` 承载 center，`definition_point` 承载圆周点
- native_bridge 中 `native_dimension_to_acadrust` 已支持所有 7 种分支，直接复用
- 每个文件删除本地 `fn v3(..)` helper（用 `[f64;3]` 字面量替代）

**度量**：7 个命令文件中 `acadrust::` 引用各 3 → 0；共减少 21 处 acadrust 引用

- DWG 88 / DXF 81 / model 9 全绿；主 crate 零 warning 保持

### 2026-04-17：C2b TEXT / MTEXT 命令 native-first

把 `src/modules/annotate/{text,mtext}.rs` 两个命令从 `acadrust::{Text, MText}`
切到 `nm::EntityData::{Text, MText}` 构造，沿用 B5b 的 `CommitEntityNative`
通道。

- `TextCommand::on_text_input`：`acadrust::Text::with_value` → `nm::Entity::new( nm::EntityData::Text { insertion, height, value, rotation, style_name, width_factor, oblique_angle, horizontal_alignment, vertical_alignment, alignment_point })`
- `MTextCommand::on_text_input`：`acadrust::MText { ... }` → `nm::Entity::new( nm::EntityData::MText { insertion, height, width, rectangle_height, value, rotation, style_name, attachment_point, line_spacing_factor, drawing_direction })`
- 两个文件的 `acadrust::` 引用各 2 → 0
- `native_bridge` 已有 Text/MText 的投影（以 radians 持有，度角在投影时转换）
- DWG 88 / DXF 81 / model 9 全绿；主 crate 零 warning 保持

### 2026-04-17：C2a RAY / XLINE 命令 native-first

沿用 B5b 模式，把 `src/modules/home/draw/ray.rs` 里的 RAY / XLINE 两个命令
从 `acadrust::EntityType::{Ray,XLine}` 构造切到 `nm::EntityData::{Ray,XLine}`。

- `RayCommand::on_point` / `XLineCommand::on_point` 的 `CmdResult::CommitEntity( EntityType::Ray(..))` 全部改为 `CmdResult::CommitEntityNative(nm::Entity::new( nm::EntityData::Ray {..}))`
- 移除 `use acadrust::entities::{Ray as RayEnt, XLine as XLineEnt}` 和
`use acadrust::EntityType`；只 `use h7cad_native_model as nm`
- `native_entity_to_acadrust` 已有 Ray/XLine 分支，cmd_result 的
CommitEntityNative handler 自动走投影路径
- 度量：`ray.rs` 中 `acadrust::` 引用 3 → 0；DWG 88 / DXF 81 / model 9 全绿；
主 crate 零 warning 保持

### 2026-04-17：B5g Compat adapter 物理删除 + feature 移除

把 10 个 entity 文件里全部 **44 个** `#[cfg(feature = "acadrust-compat")]`
adapter impl 物理删除，从 `Cargo.toml` 移除 `acadrust-compat` feature 及其
`default` 声明，完成 B 系列 compat 清理最终一步。

- 删除的文件：`line/circle/arc/point/ellipse/lwpolyline/ray/solid/spline/shape`
各自的 `impl {TruckConvertible,Grippable,PropertyEditable,Transformable} for acadrust::entities::Xxx` 共 44 个 impl
- 连带删除的 adapter 专用 helper：`lwpolyline::ar_to_nm` /
`lwpolyline::write_back_verts` / `solid::ar_corners` / `solid::write_back` /
`common::v3_to_arr` / `common::arr_to_v3`
- `Cargo.toml`：移除 `[features] default = ["acadrust-compat"]` 和 `acadrust-compat = []`
- B5 系列的"精确量化"闭环：B3 建 feature gate → B5a/b/c/d/e inline dispatch →
B5g 物理删除，全部 66 处 trait bound 依赖彻底消除

**度量**：

- `cargo check -p H7CAD` 0 error / 0 warning
- `cargo check -p H7CAD --no-default-features` 已不适用（feature 已删）
- DWG 88/88、DXF 81/81、model 9/9 全绿
- 10 个 entity 文件 `acadrust::` 引用：全部仅保留 free function 内部（~30 处，
属 bridge 合理归属）

**剩余 acadrust 依赖**：`src/scene/`*, `src/modules/`*, `src/app/*`, `src/entities/`
中复杂 entity（Polyline/Hatch/Text/Dimension/Insert/Viewport 等 25 个）仍通过
本地 `struct` 承载 acadrust 字段；`src/io/{mod,native_bridge}.rs` 保留 acadrust
DwgReader/Writer 路径。这些属于 B 系列之外的长期工作，不影响 B5 闭环。

### 2026-04-17：B5e 剩余 5 个 entity dispatch 彻底脱钩 acadrust

把 `src/entities/traits.rs::EntityTypeOps` 里 6 个 dispatch 方法中，对 Ray / XLine /
Solid / Spline / Shape 这 5 个复杂 entity 的调用从 `Trait::method(x)`（依赖
`impl ... for acadrust::entities::X` adapter）inline 成直接调用 native free
function（`ray::ray_to_truck(&o, &d)`、`solid::to_truck(&corners)`、
`spline::to_truck(degree, knots, &cps)`、`shape::to_truck(&ins, size)` 等）。

- `to_truck_entity` / `grips` / `geometry_properties` / `apply_geom_prop` /
`apply_grip` / `apply_transform` 6 个方法中的 5 个 arm 全部 inline
- 共 **30 个 arm** 改造完成（Spline 的 `apply_geom_prop` 本就是空实现，改为 noop）
- XLine 的 grips/properties/apply_* 复用 Ray 的 free function（`ray::ray_grips` 等），
native 层已经这样设计，本次只是把 dispatch 接过来

**量化收益**：`cargo check -p H7CAD --no-default-features` 错误数
  **31 → 0**（全部 5 个 entity 的 trait bound 错误消除）

- DWG 88/88、DXF 81/81、model 9/9 全绿
- 主 crate 默认 feature 下零 warning 保持
- 至此，`--no-default-features` **首次能完整编译**（纯 native dispatch 路径打通）

**下一步（B5g）**：可物理删除 `src/entities/{line,circle,arc,point,ellipse, lwpolyline,ray,solid,spline,shape}.rs` 中的 44 个 `#[cfg(feature = "acadrust-compat")]`
adapter impl，最终从 `Cargo.toml` 移除 `acadrust-compat` feature。需先处理
`src/scene`/`src/modules` 中仍直接使用 `EntityType` dispatch 的业务代码。

### 2026-04-17：B5c LwPolyline inline dispatch

把 LwPolyline 加入 traits.rs 的 inline native dispatch 行列。

- 在 `traits.rs` 新增 `lwv_ar_to_nm` 和 `lwv_write_back` helper
（`acadrust::entities::LwVertex` ↔ `nm::LwVertex` 转换）
- 对 LwPolyline 在 6 个 dispatch 方法里 inline 调用 `lwpolyline::to_truck/grips/ properties/apply_geom_prop/apply_grip/apply_transform`
- **量化收益**：`cargo check -p H7CAD --no-default-features` 错误数
**36 → 30**（减 6，对应 LwPolyline 的 6 个 dispatch 方法）
- DWG 88/88、DXF 81/81、model 9/9 全绿
- 主 crate 默认 feature 下零 warning 保持

### 2026-04-17：B5d EntityTypeOps dispatch 部分脱钩 acadrust（5 个简单 entity）

把 `src/entities/traits.rs::EntityTypeOps` 里 6 个 dispatch 方法中，对 Line /
Circle / Arc / Point / Ellipse 这 5 个简单 entity 的调用从 `Trait::method(x)`
（依赖 `impl ... for acadrust::entities::X` adapter）inline 成直接调用 native
free function（`line::to_truck(&s, &e)` 等）。

- `to_truck_entity`：5 arm 改为 inline
- `grips`：5 arm 改为 inline
- `geometry_properties`：5 arm 改为 inline
- `apply_geom_prop`：5 arm 改为 inline（含字段写回）
- `apply_grip`：5 arm 改为 inline（含字段写回）
- `apply_transform`：5 arm 改为 inline（含字段写回）
- 共 **30 个 arm** 改造完成

**量化收益**：`cargo check -p H7CAD --no-default-features` 错误数
  **66 → 36**（减少 30，正好对应 30 个被解耦的调用点）

剩余 36 个错误全部在复杂 entity（Spline/LwPolyline/Polyline/Text/Dimension/Hatch
/Insert 等）的 trait dispatch 上，需要 B5c 先扩展 nm schema 再接通。

DWG 88/88、DXF 81/81、model 9/9 全绿；主 crate 默认 feature 下零 warning 保持。

### 2026-04-17：B5b 简单画图命令 native-first（LINE/CIRCLE/ARC/POINT/ELLIPSE）

把 5 个最核心的画图命令从 acadrust 类型完全切换到 `nm::Entity` 构造。

- `CmdResult` 枚举新增两个 native 变体：
`CommitEntityNative(nm::Entity)`（对等 CommitEntity，命令保持活动）
`CommitAndExitNative(nm::Entity)`（对等 CommitAndExit，命令退出）
- `cmd_result.rs` 新增两个 handler 分支，用 `native_entity_to_acadrust` 投影回
compat 层，复用现有 `commit_entity` 流程（layer/color/linetype 默认值 + scene
镜像）
- 5 个画图命令文件切换：
  - `modules/home/draw/line.rs`：`acadrust::Line::from_points` → `nm::EntityData::Line`
  - `modules/home/draw/circle.rs`：`acadrust::Circle` → `nm::EntityData::Circle`
  - `modules/home/draw/arc.rs`：`acadrust::Arc as CadArc` → `nm::EntityData::Arc`
  - `modules/home/draw/point.rs`：`acadrust::Point as CadPoint` → `nm::EntityData::Point`
  - `modules/home/draw/ellipse.rs`：`acadrust::Ellipse` → `nm::EntityData::Ellipse`
- **度量**：5 个命令文件的 `acadrust::` 引用 7 → **0**
- DWG 28+53+7=88、DXF 81/81、model 9/9 全绿；主 crate 零 warning 保持

注：`--no-default-features` 错误数仍为 66（traits.rs 的 EntityTypeOps dispatch 还
未切换）。这属于 B5c/d 工作范围 —— commands 层的 native-first 是一步，scene 层
的 dispatch 替换是下一步。

### 2026-04-17 综述：Compat 清理 B 系列完成 B1/B2/B3/B5a

一天内完成 4 个批次，主 crate 始终保持零 warning、测试全绿。


| 批次                          | 收益                                                                                                                            |
| --------------------------- | ----------------------------------------------------------------------------------------------------------------------------- |
| **B1 类型别名门面**               | 新建 `src/types.rs` re-export 层；78 文件批量替换 `acadrust::types::`* → `crate::types::`*；业务代码 `acadrust::types::` 引用 62 → 0           |
| **B2 XData 双向投影**           | `native_bridge.rs` 新增 14 种 XDataValue 完整往返；修复 DWG save 丢 xdata 的隐性 bug；XDATA 命令迁到 native_store，`acadrust::xdata` 业务引用 2 → 0   |
| **B3 Adapter feature gate** | 10 个 entity 文件 44 个 compat impl 加 `#[cfg(feature = "acadrust-compat")]`；关闭 feature 时编译报出 **66 处 trait bound 错误**，精确量化 B5 工作范围 |
| **B5a Native dispatch 起步**  | `traits.rs` 新增 6 个 `*_native(&nm::EntityData, ...)` 函数，覆盖 Line/Circle/Arc/Point/Ellipse；为后续命令切 native_store 提供接入点             |


**度量**：

- `acadrust::types::` 引用：62 → 0 （业务侧）
- `acadrust::xdata::` 引用：2 → 0 （业务侧）
- `cargo check -p H7CAD --no-default-features` 错误数：0（默认 feature）→ 66（关闭 acadrust-compat，精准暴露 B5 工作量）
- 主 crate `cargo check -p H7CAD` warning：0 全程保持
- DWG 88/88、DXF 81/81、model 9/9 全绿全程保持

**下一步（下一会话可直接启动）**：B5b —— 扩展 CmdResult 枚举 + 改造 5 个画图
命令（LINE/CIRCLE/ARC/POINT/ELLIPSE）走 native-first。详见
`docs/plans/2026-04-17-acadrust-removal-plan.md`。

---

### 2026-04-17：B5a Native dispatch 起步（5 个简单 entity）

在 `src/entities/traits.rs` 建立并行的 `nm::EntityData` dispatch 入口，为未来逐个
命令切到 native_store 准备落地点。

- 新增 6 个 `*_native(&nm::EntityData, ...)` 自由函数：
`to_truck_native` / `grips_native` / `properties_native` / `apply_geom_prop_native`
/ `apply_grip_native` / `apply_transform_native`
- 覆盖 5 个简单 entity 类型：Line / Circle / Arc / Point / Ellipse
（与 `src/entities/{line,circle,arc,point,ellipse}.rs` 已存在的 native free
function 对接）
- 其他 variant 回落到默认值（`None` / `vec![]` / `{}`），未来批次扩展 LwPolyline/
Spline/Text/Dimension/Hatch/Insert/Viewport 等
- 这些函数当前**尚未被调用**（acadrust EntityType dispatch 仍是主路径），用
`#[allow(dead_code)]` 标注；后续 B5b-B5g 每个命令切换时依次接通
- DWG 88/88、DXF 81/81、model 9/9 全绿；主 crate 零 warning 保持

策略说明：为什么只做 5 个类型？

1. Line/Circle/Arc/Point/Ellipse 是最简单、最标准化的原语，各自 `nm::EntityData`
  variant 字段≤5 个，能一次对接完成而不引入不一致
2. Polyline/Dimension/Hatch 等复杂 entity 的 native free function 尚未完全就绪
  （需要先在各自 entity 文件里补 native 接口），是 B5c/d 工作
3. 5 个类型已覆盖约 70% 的日常画图场景，为 B5b 切换命令（DRAWLINE/CIRCLE/ARC…）
  提供充分入口

### 2026-04-17：B3 Entity adapter 隔离（acadrust-compat feature gate）

把 `src/entities/{line,circle,arc,point,ellipse,solid,ray,spline,lwpolyline,shape}.rs`
里 44 个 `impl {TruckConvertible,Grippable,PropertyEditable,Transformable} for acadrust::entities::Xxx` 的 compat adapter 门控在新 feature 下。

- `Cargo.toml` 新增 feature `acadrust-compat`（`default = ["acadrust-compat"]`，
现有行为完全不变）
- 10 个 entity 文件每个 impl block 头上加 `#[cfg(feature = "acadrust-compat")]`
- **关键度量**：关闭 feature 时编译报错 **66 处 trait bound 未满足**——这正是
B5 要处理的 acadrust `EntityType` dispatch 调用点，工作量被精确量化
- 默认 feature 下零改动（代码路径、行为、测试全部不变）
- DWG 88/88、DXF 81/81、model 9/9 全绿；主 crate 零 warning 保持

策略说明：没有物理搬动代码到独立文件，只加 cfg gate。原因：

1. 零代码搬动风险为 0
2. 门控粒度足够（每个 impl 独立开关）
3. 关 feature 时编译错误成为"acadrust 依赖清单"，B5 可据此逐项处理
4. 将来 B5 完成后删除 compat 一行 sed 即可（批量删 cfg attr + impl block）

### 2026-04-17：B2 XData 迁移到 native + bridge 双向投影

把 `acadrust::xdata::{ExtendedDataRecord, XDataValue}` 从业务代码剥离，集中到
`src/io/native_bridge.rs` 的 bridge 层；XDATA 命令（LIST/SET/CLEAR）完全走
native-first，以 `nm::Entity.xdata` 为真源。

- `native_bridge.rs` 新增 `xdata_to_acadrust` / `xdata_from_acadrust` 双向投影，
覆盖 `String/ControlString/LayerName/BinaryData/Handle/Point3D 家族/Real/ Distance/ScaleFactor/Integer16/Integer32` 全部 14 种 `XDataValue`，group code
1000-1071 完整往返
- `native_common_from_acadrust` 调用 `xdata_from_acadrust`（DWG/DXF 读入时填充
`nm::Entity.xdata`）
- `apply_common` 调用 `xdata_to_acadrust`（native→acadrust 投影时同步 xdata，
保证 DWG 保存路径不丢 xdata）
- `commands.rs` 的 XDATA 命令：
  - LIST：从 `native_store.inner().get_entity(nh).xdata` 读取，格式化输出 `code: value`
  - SET / CLEAR：改走 `apply_store_edit` 通用入口，编辑 `entity.xdata`，自动
  snapshot + compat 投影
- 结果：`src/` 业务代码 `acadrust::xdata::` 引用 2 处 → **0 处**（仅 bridge 内部
使用，属合理归属）
- DWG 88/88、DXF 81/81、model 9/9 全绿；主 crate 零 warning 保持

### 2026-04-17：B1 类型别名迁移（acadrust::types 去直接依赖）

把 `acadrust::types::{Vector2, Vector3, Color, Handle, LineWeight, Transparency, Transform, Matrix3, Matrix4, BoundingBox2D, BoundingBox3D, DxfVersion, aci_table}`
的直接引用全面切换到 `crate::types::`* 门面层。业务代码与 `acadrust` 实现解耦，
未来切换到 native 实现只需改 `src/types.rs` 一个文件。

- 新增 `src/types.rs`：顶层门面，`pub use acadrust::types::`*（14 个类型）
- 在 `src/main.rs` 登记 `mod types`
- 批量替换 78 个源文件中的 `acadrust::types` → `crate::types`（包括 `aci_table` 路径）
- 结果：`src/` 下 `acadrust::types::` 引用 62 处 → **0 处**（仅 `src/types.rs` 自身 2 处 re-export）
- `cargo check -p H7CAD` 零 warning（保持）
- DWG 88/88、DXF 81/81、model 9/9 全绿

参见 `docs/plans/2026-04-17-acadrust-removal-plan.md` Layer 1。
下一批（B2 XData / B3 Entity adapter / B4 ObjectType/Table / B5 Scene dispatch）
继续按该计划推进。

### 2026-04-17：DXF 补齐、DWG 原生解析 M3-A 贯通、Compat 清理

#### DXF 冷门类型补齐

覆盖之前被 `EntityData::Unknown` / `ObjectData::Unknown` 吞没的大量常见 AutoCAD 对象。
Reader / Writer / bridge 全链路接通。

- **ENTITIES 新增变体**：`HELIX`、`ARC_DIMENSION`、`LARGE_RADIAL_DIMENSION`、
Surface 家族（`EXTRUDEDSURFACE / LOFTEDSURFACE / REVOLVEDSURFACE / SWEPTSURFACE / PLANESURFACE / NURBSURFACE`）、`LIGHT`、`CAMERA`、`SECTION`、
`ACAD_PROXY_ENTITY`
- **OBJECTS 新增变体**：`FIELD`、`IDBUFFER`、`LAYER_FILTER`、`LIGHTLIST`、
`SUNSTUDY`、`DATATABLE`、`WIPEOUTVARIABLES`、`GEODATA`、`RENDERENVIRONMENT`、
`ACAD_PROXY_OBJECT`
- Surface 家族统一用 `Surface { surface_kind, u_isolines, v_isolines, acis_data }`
承载 6 种子类型，避免变体爆炸
- `ProxyEntity` / `ProxyObject` 用 `raw_codes` 原始透传，保证读→写不丢失信息
- `h7cad-native-dxf` 测试 72 → 81 全绿

#### DWG 原生解析 M3-A 知识层贯通

在 `crates/h7cad-native-dwg` 建立了对真实 AC1015 (R2000) DWG 的完整读取路径。
每一砖都用 `ACadSharp/samples/sample_AC1015.dwg` 真实字节做硬锚点验证。

- 新增 `crates/h7cad-native-dwg/src/bit_reader.rs`：MSB-first bit 流读取器，
支持 DWG 全部原生类型（`BitShort / BitLong / BitLongLong / BitDouble / Handle / Text`）
- 新增 `crates/h7cad-native-dwg/src/known_section.rs`：`KnownSection` 枚举
（`Header / Classes / Handles / ObjFreeSpace / Template / AuxHeader`）与 start/end sentinel
常量
- 修正 `section_map.rs`：AC1015 section locator record 从错误的 8 字节修正为
正确的 9 字节（1 byte record_number + 4 byte seeker + 4 byte size），
`section_count` 加 128 上界保护
- 确认 6 段布局全部匹配：AcDb:Header / Classes 的 16 字节 start sentinel 相等
- 真实解出：4 BitDouble 常量（`412148564080, 1, 1, 1`）+ 4 TV（`"m"`）+ 2 BL +
Viewport Handle + 20 个 CadHeader 布尔标志 + 8 个单位 BS（LUNITS=2, LUPREC=4,
AUNITS=0, ATTMODE=1, PDMODE=34）
- Classes section：51 条真实 class records（AcDbDictionaryWithDefault /
AcDbLayout / AcDbTableStyle ...）
- Handles section：1047 个 handle→offset 条目（2 chunks，通过 ModularChar +
SignedModularChar 增量编码）
- `crates/h7cad-native-dwg` 测试 0 → 88 全绿

#### Compat 清理（acadrust 依赖收缩）

- 新增 `docs/plans/2026-04-17-acadrust-removal-plan.md`：盘点 src/ 下 ~700 处
`acadrust::` 引用，按 5 层分类（I/O 边界保留 / 类型别名 / entity adapter /
scene-module dispatch / object-table），给出 B1–B5 分批迁移路径
- 删除 ~200 行真实 dead code：
  - `app/helpers.rs::sync_native_entity_from_compat`（compat←native 旧同步方向）
  - `scene/hit_test.rs::click_hit_hatch / box_hit_hatch / poly_hit_hatch`
  （HashMap 版本，被 `_entries` slice 版本取代）
  - `scene/transform.rs::mirror_xy_line`（直接操作 `acadrust::entities::Line`）
  - `modules/home/modify/splinedit.rs::apply_spline_op`（compat 版，
  被 `apply_spline_op_entity` 取代）
  - `modules/home/modify/attedit.rs::apply_attedit`（compat 版，
  被 `apply_attedit_native` 取代）
  - `entities/common.rs::transform_angle`、`entities/spline.rs::apply_geom_prop` 空实现
  - DXF tokenizer 的 `read_i64_le` 未使用方法
- `CadStore` trait / `StoreSnapshot` struct / `NativeStore::into_inner`
加 `#[allow(dead_code)]`（是为 native-first 迁移保留的预留接口，不是真死代码）
- **主 crate `cargo check -p H7CAD` 零 warning**

### 架构重构：CadStore 统一文档存储层

引入 `CadStore` trait 和 `NativeStore` 实现，将文档编辑流向从 compat-first（acadrust → native）
切换为 native-first（native → compat 投影）。

#### 新增

- `src/store/mod.rs` — `CadStore` trait：实体 CRUD、常用属性编辑（layer/color/linetype/lineweight/invisible/transparency）、持久化、快照/撤销
- `src/store/native_store.rs` — `NativeStore`，包装 `h7cad_native_model::CadDocument` 的 `CadStore` 实现
- `Scene::native_doc()` / `native_doc_mut()` / `set_native_doc()` 访问器方法
- `H7CAD::apply_store_edit()` — native-first 单闭包属性编辑方法
- `H7CAD::sync_compat_from_native()` — 反向同步（native → compat 投影）
- `Scene::rebuild_gpu_model_after_grip()` — grip 编辑后重建 hatch/solid GPU 模型

#### 变更

- `Scene::native_document: Option<nm::CadDocument>` → `Scene::native_store: Option<NativeStore>`
- `save_active_tab_to_path` 改用 `CadStore::save`
- 属性编辑（Layer/Color/LineWeight/Linetype/Toggle/GeomProp/Transparency）改为 native-first
- Grip 拖拽编辑改为 native-first
- `transform_entities`（MOVE/ROTATE/SCALE/MIRROR）改为 native-first
- MATCHPROP（layer 匹配 + 全属性匹配）改为 native-first
- `HistorySnapshot::native_document` → `native_doc_clone`

#### 移除

- `apply_property_edits` 双闭包方法（被 `apply_store_edit` 替代）
- compat 版 `toggle_invisible`、`Scene::apply_grip` 成为 dead code

