# Entity 类型体系迁移方案

> 目标：将 H7CAD 从 acadrust::CadDocument 迁移到 h7cad-native-model::CadDocument
> 创建：2026-04-09
> 状态：Phase 1 待开始

## 影响分析

`use acadrust` 在 **102 个文件**中出现 **~530 次**：

| 层级 | 文件数 | 引用数 | 复杂度 |
|------|--------|--------|--------|
| `app/` (应用核心) | 6 | ~220 | ★★★★★ |
| `scene/` (渲染/场景) | 15 | ~65 | ★★★★ |
| `entities/` (实体封装) | 17 | ~50 | ★★★ |
| `modules/` (命令/工具) | 55 | ~160 | ★★★ |
| `ui/` (界面) | 5 | ~12 | ★★ |
| `io/` (IO) | 2 | ~5 | ★★ |

## 分阶段计划

### Phase 1: 兼容 trait 层（1 天）
- [ ] 在 h7cad-native-model 定义与 acadrust 相同签名的 Entity trait
- [ ] EntityData 实现 trait
- [ ] 保持两个 CadDocument 并存

### Phase 2: IO 层切换（0.5 天）
- [ ] DXF 读写切换到 native（已有 read_dxf_bytes/write_dxf）
- [ ] DWG 保留 acadrust
- [ ] 添加 native ↔ acadrust 转换函数
- [ ] src/io/mod.rs 修改

### Phase 3: entities/ 封装层（2 天）
- [ ] 17 个文件逐个替换
- [ ] acadrust::EntityType → h7cad_native_model::EntityData 映射

### Phase 4: scene/ 渲染层（3 天）
- [ ] scene/mod.rs（30 引用）
- [ ] tessellate, dispatch, transform, properties
- [ ] 确保几何数据格式兼容

### Phase 5: app/ 应用核心（3-4 天）
- [ ] app/commands.rs（139 引用）
- [ ] app/cmd_result.rs（40 引用）
- [ ] app/update.rs（30 引用）

### Phase 6: modules/ 命令层（3-4 天）
- [ ] 55 个文件逐个命令替换
- [ ] 可按功能分组批量处理

## 预估总工期

12-15 个工作日

## 风险

1. DWG 支持需要转换层
2. 回归风险——102 文件大改
3. 并行开发合并冲突
