# R49-VIEWPORT-PAPER-SPACE-SYNC: paper-space viewport commit 镜像到 native document

> 起稿：2026-04-28
> 前置：R48-DWG-FACADE-AND-BUILD 完成；workspace test 仅剩 1 个 fail
> （`commit_entity_syncs_native_viewport_in_paper_space`），R49 收口让
> `cargo test --workspace --all-targets` 100% pass。

## 1. 根因

`H7CAD::commit_entity`（`src/app/properties.rs:402+`）对 paper-space
viewport 走自己的 `document.add_entity_to_layout` path：

```rust
if matches!(&entity, acadrust::EntityType::Viewport(_))
    && self.tabs[i].scene.current_layout != "Model"
{
    // ... viewport id 计算 ...
    self.tabs[i].scene.document.add_entity_to_layout(entity, &layout)  // ← 只动 acadrust
    // ... auto_fit_viewport ...
}
```

但 `scene::add_entity`（`src/scene/mod.rs:1813+`）**已经**在内部正确做了：

1. 按 `current_layout` 路由到 `add_entity_to_layout` 或 `document.add_entity`；
2. **mirror 到 `native_store`**（line 1876-1893），通过
   `native_bridge::acadrust_entity_to_native` 转换并设 `owner_handle` 为
   layout 的 block_record_handle 或 model space handle。

`commit_entity` 的 viewport path 绕过 `scene.add_entity`，所以
`native_store` 不接收 viewport——测试断言
`native_doc.entities().filter(|e| Viewport).count() == 1` 失败（实测 == 0）。

## 2. 修复

把 paper-space + viewport 分支改为复用 `scene.add_entity`：保留 viewport id
计算（依赖 acadrust document 的 entity 列表，无法在 native_store 上做），
然后用 `scene.add_entity` 完成 add + native mirror，再 `auto_fit_viewport`。

`scene.add_entity` 的 paper-space 判定 `current_layout != "Model" &&
active_viewport.is_none()` 与 `commit_entity` 的语义一致：测试场景
`current_layout = "Layout1"`、`active_viewport = None` 时两者都走
`add_entity_to_layout` 路径。

## 3. 范围

| 任务 | 优先级 | 预估 |
|---|---:|---:|
| T1 调研 commit_entity 的 paper-space viewport path 和 scene.add_entity 的 native mirror 逻辑 | ✅ 完成 | 0.2 h |
| T2 落盘本 plan 文件 | P0 | 0.1 h |
| T3 修改 `commit_entity`：paper-space + viewport path 复用 `scene.add_entity` | P0 | 0.2 h |
| T4 验证 `commit_entity_syncs_native_viewport_in_paper_space` 测试 pass | P0 | 0.1 h |
| T5 跑完整 workspace test 确认 100% pass，无新回归 | P0 | 0.3 h |

## 4. 不纳入

- 不重构 `scene.add_entity` 主体逻辑（它已经对了）
- 不动 `add_entity_to_layout` 的 acadrust 实现
- 不动 viewport id 分配规则（保持 max+1 / min=2）
- 不修改 `auto_fit_viewport`

## 5. 验收

```bash
cargo test -p H7CAD --bin H7CAD commit_entity_syncs_native_viewport_in_paper_space
cargo test --locked --workspace --all-targets
RUSTFLAGS=-Dwarnings cargo check --locked --workspace --all-targets
```

通过标准：

- 上述 viewport test pass；
- `cargo test --workspace --all-targets` **100% pass**（R48 时 425/1 → R49
  完成 426/0）；
- `-Dwarnings cargo check workspace` 仍通过；
- 不引入新的 audit / golden 测试不平衡。

## 6. 状态

- [x] T1 根因调研（commit_entity paper-space viewport path 绕过 scene.add_entity 的 native mirror）
- [x] T2 plan 文件落盘
- [x] T3 commit_entity 修复（paper-space + viewport 分支改用 scene.add_entity；保留 viewport id 计算 + auto_fit_viewport 后处理）
- [x] T4 `commit_entity_syncs_native_viewport_in_paper_space` 测试 pass
- [x] T5 workspace test 100% pass（426/0），`RUSTFLAGS=-Dwarnings cargo check workspace` 全过
