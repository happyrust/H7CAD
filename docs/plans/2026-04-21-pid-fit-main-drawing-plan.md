# 开发计划：PID 打开后 fit 优先聚焦主绘图层

> 起稿：2026-04-21（第五轮）  
> 前置：`docs/plans/2026-04-21-pid-real-sample-display-and-screenshot-plan.md` 已完成。  
> 该轮 Task 2 发现 target 样本装饰 panel（42 entities）vs 主绘图层（12 entities）比例失衡，而现有 `Scene::fit_all` 把整个 scene bbox 全算进去（包含装饰 panel），导致**主图被挤到视口的一小块**。本轮在不动装饰 panel 布局的前提下，让 fit_all 行为在 PID 场景下优先"只看主绘图层"。

## 动机

`Scene::fit_all()` 当前（`src/scene/mod.rs:2972`）遍历 `entity_wires()` 所有 wires 求 bbox。对 PID 文档：

- 主绘图层（`PID_OBJECTS_*` / `PID_LAYOUT_TEXT` / `PID_RELATIONSHIPS`）：layout item + label + 连接段
- 装饰层（`PID_META` / `PID_FALLBACK` / `PID_CROSSREF` / `PID_UNRESOLVED` / `PID_STREAMS` / `PID_CLUSTERS` / `PID_SYMBOLS`）：右侧 / 底部 / 外延的信息面板

fit_all bbox = union(主绘图 bbox, 装饰 panel bbox)。而装饰 panel 被故意放在很远的坐标（`SIDE_PANEL_X = GRID_COLUMNS * GRID_SPACING_X + 80` / `BOTTOM_PANEL_Y = -820` 等常量），所以 bbox 被"拉得很大"，fit_all 完成后主绘图只占视口一小块角落。

这是上一轮显式留下的问题（CHANGELOG 2026-04-21（四）Task 2 诊断记录）。

## 目标

1. `Scene` 新增 `fit_layers_matching(layer_prefixes: &[&str]) -> bool`：
   - 遍历 native_doc（优先）或 compat document 的实体
   - 只对 `layer_name.starts_with(p)` 匹配任一 `p` 的实体取特征点
   - 计算 bbox → 调 `camera.fit_to_bounds`
   - 返回 bool：true 表示有匹配实体 + 已 fit；false 表示无匹配（调用方应 fallback）
2. `Message::FileOpened` 的 PID 分支（`src/app/update.rs:354-391`）将原 `fit_all()` 改为：
   ```rust
   const PID_MAIN_LAYERS: &[&str] =
       &["PID_OBJECTS_", "PID_LAYOUT_TEXT", "PID_RELATIONSHIPS"];
   if !scene.fit_layers_matching(PID_MAIN_LAYERS) {
       scene.fit_all();
   }
   ```
3. 单测 + 集成测试覆盖：
   - 匹配层存在 → bbox 只包含匹配层几何
   - 匹配层缺失 → 返回 false、不改 camera（非 PID 场景不受影响）
   - 不同层前缀混合 → 正确 filter
   - 与 target 样本联动：PID 打开路径优先调 fit_layers_matching

## 非目标

- 不改 wires 结构（不加 layer 字段）
- 不改装饰 panel 布局 / 坐标 / 尺寸
- 不改 CAD 文档的 fit_all 行为（仅 PID tab 受益）
- 不实现更精细的 "fit to selection" / "fit to viewport" — 独立工作
- 不改 camera fit_to_bounds 语义

## 关键设计

### 1. 特征点提取 helper

为每种 entity 提供 "bbox 贡献点"：

```rust
fn entity_bbox_points(entity: &nm::Entity) -> Vec<[f64; 3]> {
    match &entity.data {
        nm::EntityData::Line { start, end } => vec![*start, *end],
        nm::EntityData::Circle { center, radius } => {
            vec![
                [center[0] - radius, center[1] - radius, center[2]],
                [center[0] + radius, center[1] + radius, center[2]],
            ]
        }
        nm::EntityData::Arc { center, radius, .. } => {
            vec![
                [center[0] - radius, center[1] - radius, center[2]],
                [center[0] + radius, center[1] + radius, center[2]],
            ]
        }
        nm::EntityData::Text { insertion, .. }
        | nm::EntityData::MText { insertion, .. } => vec![*insertion],
        nm::EntityData::Point { position } => vec![*position],
        nm::EntityData::LwPolyline { vertices, .. } => vertices
            .iter()
            .map(|v| [v.x, v.y, 0.0])
            .collect(),
        nm::EntityData::Polyline { vertices, .. } => {
            vertices.iter().map(|v| v.position).collect()
        }
        _ => Vec::new(),
    }
}
```

覆盖 PID preview 输出的所有 kind（Line / Circle / Arc / Text / MText / Point / Polyline）；其他（Hatch / Insert / Spline）在 PID 预览里不会出现，即使出现也不被 filter 收集（无副作用，等于"被忽略"）。

### 2. `fit_layers_matching` 实现

```rust
pub fn fit_layers_matching(&mut self, layer_prefixes: &[&str]) -> bool {
    let Some(native) = self.native_doc() else {
        return false;
    };
    let mut min = glam::Vec3::splat(f32::MAX);
    let mut max = glam::Vec3::splat(f32::MIN);
    let mut found = false;
    for entity in &native.entities {
        if !layer_prefixes
            .iter()
            .any(|p| entity.layer_name.starts_with(p))
        {
            continue;
        }
        for point in entity_bbox_points(entity) {
            let v = glam::Vec3::new(point[0] as f32, point[1] as f32, point[2] as f32);
            min = min.min(v);
            max = max.max(v);
            found = true;
        }
    }
    if !found {
        return false;
    }
    if min == max {
        max += glam::Vec3::splat(1.0);
    }
    self.camera.borrow_mut().fit_to_bounds(min, max);
    self.camera_generation += 1;
    true
}
```

### 3. FileOpened 整合

`src/app/update.rs` PID 分支末尾：

```rust
const PID_MAIN_LAYERS: &[&str] = &[
    "PID_OBJECTS_",
    "PID_LAYOUT_TEXT",
    "PID_RELATIONSHIPS",
];
if !self.tabs[i].scene.fit_layers_matching(PID_MAIN_LAYERS) {
    self.tabs[i].scene.fit_all();
}
```

CAD / DXF / DWG 分支继续走 `scene.fit_all()`，不受影响。

## 实施步骤

### M1 — Scene 新增 fit_layers_matching（20 min）

1. 在 `src/scene/mod.rs` fit_all 附近加 `fit_layers_matching` + private helper `entity_bbox_points`
2. 辅助函数放在 impl Scene 的同一 block，与 fit_all 紧邻
3. `cargo check` 过

### M2 — 入口整合（10 min）

1. `src/app/update.rs::Message::FileOpened` 的 PID 分支改为 fit_layers_matching + fallback
2. `cargo check -p H7CAD` 过

### M3 — 集成测试（25 min）

在 `src/scene/mod.rs::tests` 追加：

- `fit_layers_matching_uses_only_matching_entities`：构造 native doc 含主图层（PID_OBJECTS_*）+ 装饰层（PID_META）实体于远坐标 → 调 fit_layers_matching(&["PID_OBJECTS_"]) → bbox 不包含装饰层
- `fit_layers_matching_returns_false_when_no_match`：doc 全是 CAD 层实体 → 调 fit_layers_matching(&["PID_OBJECTS_"]) → false、camera 未改
- `fit_layers_matching_noop_without_native_doc`：scene 无 native_doc → 返回 false

在 `src/io/pid_import.rs::tests` 追加：

- `target_pid_sample_fit_layers_matching_succeeds`：打开 target sample → `scene.fit_layers_matching(&["PID_OBJECTS_", "PID_LAYOUT_TEXT", "PID_RELATIONSHIPS"])` 返回 true

### M4 — validator + CHANGELOG（10 min）

- `cargo test --bin H7CAD scene -- --nocapture`
- `cargo test --bin H7CAD io::pid_import::tests::target_pid`
- `cargo check`
- CHANGELOG 追加条目

## 风险与缓解

| 风险 | 缓解 |
|---|---|
| native_doc 不存在时返回 false 触发 fit_all，装饰 panel 又被囊括 | 这正是 fallback 期望行为；调用方显式 `if !fit_layers_matching { fit_all() }` |
| PID tab 装饰 panel 被裁出视口导致"看不到 cross-ref / stream 信息" | 用户可 pan / zoom 到装饰面板位置；主绘图优先是 PID 阅读的正确默认 |
| CAD 文档误用 PID 专用 prefix | PID_OBJECTS_ / PID_LAYOUT_TEXT / PID_RELATIONSHIPS 有 `PID_` 前缀，CAD 侧不会冲突 |
| layer_prefixes 空数组 | 所有实体都不匹配 → found 保持 false → 返回 false 无副作用 |

## 验收

- `cargo test --bin H7CAD scene` 新增 3 条 + 全部既有测试绿
- `cargo test --bin H7CAD io::pid_import::tests::target_pid` 新增 1 条 + 全部既有测试绿
- `cargo check -p H7CAD` 零新 warning
- CHANGELOG 条目记录

## 执行顺序

M1 → M2 → M3 → M4（严格串行）
