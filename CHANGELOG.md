# 更新日志

## [未发布]

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

**下一步（B5g）**：可物理删除 `src/entities/{line,circle,arc,point,ellipse,
lwpolyline,ray,solid,spline,shape}.rs` 中的 44 个 `#[cfg(feature = "acadrust-compat")]`
adapter impl，最终从 `Cargo.toml` 移除 `acadrust-compat` feature。需先处理
`src/scene`/`src/modules` 中仍直接使用 `EntityType` dispatch 的业务代码。

### 2026-04-17：B5c LwPolyline inline dispatch

把 LwPolyline 加入 traits.rs 的 inline native dispatch 行列。

- 在 `traits.rs` 新增 `lwv_ar_to_nm` 和 `lwv_write_back` helper
  （`acadrust::entities::LwVertex` ↔ `nm::LwVertex` 转换）
- 对 LwPolyline 在 6 个 dispatch 方法里 inline 调用 `lwpolyline::to_truck/grips/
  properties/apply_geom_prop/apply_grip/apply_transform`
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

| 批次 | 收益 |
|---|---|
| **B1 类型别名门面** | 新建 `src/types.rs` re-export 层；78 文件批量替换 `acadrust::types::*` → `crate::types::*`；业务代码 `acadrust::types::` 引用 62 → 0 |
| **B2 XData 双向投影** | `native_bridge.rs` 新增 14 种 XDataValue 完整往返；修复 DWG save 丢 xdata 的隐性 bug；XDATA 命令迁到 native_store，`acadrust::xdata` 业务引用 2 → 0 |
| **B3 Adapter feature gate** | 10 个 entity 文件 44 个 compat impl 加 `#[cfg(feature = "acadrust-compat")]`；关闭 feature 时编译报出 **66 处 trait bound 错误**，精确量化 B5 工作范围 |
| **B5a Native dispatch 起步** | `traits.rs` 新增 6 个 `*_native(&nm::EntityData, ...)` 函数，覆盖 Line/Circle/Arc/Point/Ellipse；为后续命令切 native_store 提供接入点 |

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
里 44 个 `impl {TruckConvertible,Grippable,PropertyEditable,Transformable} for
acadrust::entities::Xxx` 的 compat adapter 门控在新 feature 下。

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
  覆盖 `String/ControlString/LayerName/BinaryData/Handle/Point3D 家族/Real/
  Distance/ScaleFactor/Integer16/Integer32` 全部 14 种 `XDataValue`，group code
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

把 `acadrust::types::{Vector2, Vector3, Color, Handle, LineWeight, Transparency,
Transform, Matrix3, Matrix4, BoundingBox2D, BoundingBox3D, DxfVersion, aci_table}`
的直接引用全面切换到 `crate::types::*` 门面层。业务代码与 `acadrust` 实现解耦，
未来切换到 native 实现只需改 `src/types.rs` 一个文件。

- 新增 `src/types.rs`：顶层门面，`pub use acadrust::types::*`（14 个类型）
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
  Surface 家族（`EXTRUDEDSURFACE / LOFTEDSURFACE / REVOLVEDSURFACE /
  SWEPTSURFACE / PLANESURFACE / NURBSURFACE`）、`LIGHT`、`CAMERA`、`SECTION`、
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
  支持 DWG 全部原生类型（`BitShort / BitLong / BitLongLong / BitDouble / Handle /
  Text`）
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
