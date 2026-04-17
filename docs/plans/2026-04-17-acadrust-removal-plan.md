# acadrust Removal Plan

> 起稿：2026-04-17  
> 依赖：本计划以完成 DXF 冷门类型补齐（2026-04-17）和 DWG M3-A 知识层贯通（2026-04-17）为前提。
>
> **目标**：把 H7CAD 运行时对 `acadrust` 的直接依赖逐步收缩到仅保留 DWG I/O 边界（读旧格式 + 写旧格式）。运行时的单一真源为 `h7cad-native-model`。

---

## 现状盘点（2026-04-17）

* 直接引用 `acadrust::` 的源文件 **~115 个**，总计 **~700 处**引用
* 已完成：`src/io/mod.rs` 的 DXF 路径已切 native-first；`CadStore`/`NativeStore` 引入；STRETCH / MATCHPROP / DDEDIT / grip / 属性编辑已 native-first
* 未完成：`src/scene`, `src/modules`, `src/entities`, `src/app` 的大多数代码仍直接操作 `acadrust::EntityType`、`acadrust::Handle`、`acadrust::types::*`

### 依赖密度分布（按 `acadrust::` 出现数量排序）

| 模块 | 次数 | 主要用途 |
|---|---:|---|
| `src/scene/mod.rs` | 59 | EntityType dispatch、Viewport、BlockRef |
| `src/app/commands.rs` | 150 | ObjectType/Layer/DimStyle 创建 |
| `src/app/update.rs` | 30 | EntityType/ObjectType 操作、Layer 表修改 |
| `src/entities/dimension.rs` | 29 | Vector3 参数、Dimension family 处理 |
| `src/app/helpers.rs` | 20 | EntityType match |
| `src/app/cmd_result.rs` | 60 | PlotSettings、Color 类型 |
| `src/scene/tessellate.rs` | 8 | Dimension/Leader/Text、Color |
| `src/scene/dispatch.rs` | 2 | 入口 EntityType dispatch |
| `src/modules/**` | ~200 | 各命令实现（create/modify/draw/annotate） |
| `src/entities/**` | ~150 | 10+ entity 的 `impl Trait for acadrust::entities::Xxx` adapter |
| `src/io/native_bridge.rs` | 11 | 本身就是 compat bridge |
| `src/io/mod.rs` | 2 | DWG 读写（保留） |

---

## 分层策略

### Layer 0：保留（I/O 边界）
* `src/io/mod.rs` — `DwgReader` / `DwgWriter`，直到 native DWG 读写就绪（M3-A+/M3-C）
* `src/io/native_bridge.rs` — 本身就是 acadrust ↔ native 桥梁，本 plan 外管理

### Layer 1：纯类型别名依赖（**最易清理**）
大多数 `use acadrust::types::{Color, Handle, Vector2, Vector3, LineWeight, Transparency}` 本质上是类型别名。可以：
1. 在 `h7cad-native-model` 补上对应类型（若缺失）
2. 一次性替换 `use acadrust::types::Vector3 as V3` → `use glam::DVec3 as V3`（或等价 native）
3. 确认 API 兼容后，删除全部 `use acadrust::types::` 行

**预计影响**：70+ 文件，都是 import 替换，单文件 diff 小。
**工作量**：1-2 天（验证 Vector2/Vector3/Color 的字段布局一致）。

### Layer 2：Entity adapter（`impl Trait for acadrust::entities::Xxx`）
`src/entities/{line,circle,arc,point,ellipse,solid,ray,spline,lwpolyline,shape,polyline,dimension,hatch,...}.rs` 都有如下模式：

```rust
impl TruckConvertible for acadrust::entities::Line {
    fn to_truck(&self, _document: &acadrust::CadDocument) -> Option<TruckEntity> {
        let s = [self.start.x, self.start.y, self.start.z];
        let e = [self.end.x, self.end.y, self.end.z];
        Some(self::to_truck(&s, &e))
    }
}
```

底层 `to_truck / grips / properties / apply_geom_prop / apply_transform` 已经是 native `[f64; 3]`。真正的 compat 层是上面这个 `impl`。

**清理方案**：把 adapter 迁到新 module `src/entities/compat_acadrust.rs`，并加 `cfg(feature = "acadrust-compat")` 门。让 native dispatch 不走 adapter。

**预计影响**：10-15 文件，每文件 50-100 行移走。
**工作量**：2-3 天。

### Layer 3：Scene / Modules 中 EntityType dispatch
`src/scene/dispatch.rs`、`src/scene/mod.rs` 中存在大量 `match entity { acadrust::EntityType::Line(l) => … }` 的 dispatch。这是最核心的 runtime 耦合点。

**清理方案**：
1. 为 `nm::EntityData` 建立等价的 dispatch（`match entity_data { EntityData::Line { .. } => … }`）
2. 所有 `acadrust::EntityType` 出现的地方并行写一份 `nm::EntityData` 版本
3. 通过 feature flag 切换；验证后删除 acadrust 版本

**预计影响**：40+ 文件，涉及每个命令实现。
**工作量**：1 周以上。

### Layer 4：ObjectType / Table 类
`acadrust::objects::{PlotSettings, SortEntitiesTable, ObjectType, MLineStyle, TableStyle}` 和 `acadrust::tables::{Layer, DimStyle, Ucs}` 在 `commands.rs` 和 `update.rs` 被大量引用。

**清理方案**：
1. `nm::ObjectData` 已经覆盖大部分 ObjectType（见 2026-04-17 的 DXF 补齐）
2. 给 `nm::ObjectData` 补 PlotSettings 等剩余类型
3. 逐命令迁移

**预计影响**：`commands.rs`（150 处）+ `update.rs`（30 处）。
**工作量**：1 周。

### Layer 5：XData / Xdef
`acadrust::xdata::{ExtendedDataRecord, XDataValue}`：1 处。

**清理方案**：把 XData 移到 `nm::Entity.xdata` 字段（已存在）。  
**工作量**：半天。

---

## 分批执行提案

| 批次 | 范围 | 风险 | 预期工作量 | 状态 |
|---|---|---|---:|---|
| B1 | Layer 1 — 类型别名（Vector/Color/Handle/…） | 低 | 1-2 天 | ✅ 2026-04-17 完成 |
| B2 | Layer 5 — XData | 低 | 半天 | ✅ 2026-04-17 完成 |
| B3 | Layer 2 — Entity adapter feature gate | 中 | 2-3 天 | ✅ 2026-04-17 完成（调整方案：不物理搬动，只加 cfg 门） |
| B5a | 为 `nm::EntityData` 建 native dispatch（5 个简单 entity） | 低 | 1 小时 | ✅ 2026-04-17 完成 |
| B4/B5 合并 | Scene/Commands 切到 native_store（含 ObjectType/Table） | 高 | 2-3 周 | 待做 |

### B4/B5 合并原因（2026-04-17 发现）

原 plan 把 B4（ObjectType/Table）和 B5（Scene/Modules EntityType dispatch）
分开。实地调查后发现：
- `commands.rs` 里 105 处 `acadrust::` 引用**全部**操作 `scene.document`
  （acadrust CadDocument）
- 迁移 `acadrust::objects::ObjectType` 本质就是 "让 scene.document 让位给
  native_store"，这正是 B5 的核心
- 两者人为分离没有独立的完成标准

因此把 B4 合并进 B5。新的 B5 子批次（按 commands 文件粒度）：
- **B5b**：简单画图命令（LINE/CIRCLE/ARC/POINT/ELLIPSE） — native_store 已自动
  镜像，只需把 CmdResult::CommitEntity 流程清理成 native-first（语法清理，1 天）
- **B5c**：复杂画图命令（LWPOLYLINE/SPLINE/TEXT/HATCH） — 需先扩展 `nm::EntityData`
  覆盖必要字段（elevation 等），再补 native dispatch（2-3 天）
- **B5d**：编辑命令（MOVE/ROTATE/SCALE/MIRROR） — 已部分 native-first，收尾（2 天）
- **B5e**：MATCHPROP/XDATA/选择等混合命令 — 迁 scene.document 读路径（3 天）
- **B5f**：Viewport/Insert/Layer 表管理（原 B4 核心） — 迁 `acadrust::objects::*`、
  `acadrust::tables::*` 到 native 路径（5 天）
- **B5g**：删除 `src/entities/*.rs` 里的 `#[cfg(feature = "acadrust-compat")]`
  adapter；关闭 feature 确认零 error；最后从 Cargo.toml 删 feature（半天）

### B5 入口精确定位

运行 `cargo check -p H7CAD --no-default-features` 得到 **66 处 trait bound 未满足
错误**。这 66 处就是 B5 的精确工作清单（每个对应一个 acadrust EntityType dispatch
调用点）。

---

## Exit Criteria

完成后：
* `src/` 下 `acadrust::` 引用数量从 ~700 → 0（除 `src/io/mod.rs` 的 DwgReader/DwgWriter）
* `cargo check -p H7CAD` 无错误且不新增 warning
* DXF/DWG 读写功能全部通过（DXF roundtrip ≥ 既有测试水平，DWG 保留 acadrust 回路）
* `CadStore` 是所有编辑操作的唯一入口

---

## 下一步（2026-04-17 晚更新）

B1/B2/B3/B5a 已完成。下一会话推荐从 **B5b** 起步：

1. 扩展 `CmdResult` 枚举：增加 `CommitEntityNative(nm::Entity)` 变体
2. `commit_entity` handler 分叉：遇 Native 变体时走 native_store.add_entity，
   走 `native_entity_to_acadrust` 投影出 acadrust 副本
3. 把 `modules/home/draw/{line,circle,arc,point,ellipse}.rs` 5 个 command 的
   `CmdResult::CommitEntity(EntityType::X)` 改为 `CmdResult::CommitEntityNative(nm::Entity)`
4. 确保 `cargo check -p H7CAD` 零 warning 继续保持
5. DWG 88 / DXF 81 / model 9 测试继续全绿

完成后可考虑在 5 个简单 entity 的 `impl ... for acadrust::entities::X` 上加
`#[allow(dead_code)]`（未来的 B5g 一次性删除）。

进度记录：
- 每次批次完成后更新 `CHANGELOG.md` 的"acadrust removal"节
- 每日在 `.memory/{date}.md` 追加条目（agent 协议要求）
