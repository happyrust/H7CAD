# 更新日志

## [未发布]

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
