# H7CAD 架构教程

本文面向需要阅读或扩展 **H7CAD** 的开发者，说明仓库分层、运行时数据流与常见扩展点。配套图示为独立 HTML（内联 SVG），可在浏览器中直接打开。

## 图示索引

| 文件 | 内容 |
|------|------|
| [diagrams/h7cad-architecture.html](diagrams/h7cad-architecture.html) | 系统分层：UI → 应用 → Scene → 存储，以及 workspace crates 与外部依赖 |
| [diagrams/h7cad-startup-flow.html](diagrams/h7cad-startup-flow.html) | `main` 双入口：无 GUI 的批处理导出 vs `app::run()` |
| [diagrams/h7cad-crates-layers.html](diagrams/h7cad-crates-layers.html) | Workspace 内 native crate 的依赖分层（应用 → facade → dxf/dwg → model） |

在仓库根目录执行：

```bash
open docs/diagrams/h7cad-architecture.html
```

（Linux 可用 `xdg-open`，Windows 可用资源管理器双击。）

## 1. 仓库与工作区

根目录 `Cargo.toml` 声明 **workspace**，成员包括主包 `H7CAD` 与多个 `crates/h7cad-native-*`：

- **h7cad-native-model**：核心 CAD 数据模型（`Handle`、`CadDocument`、图层与实体等）。
- **h7cad-native-dxf**：DXF 读写在 Rust 侧的实现，经 **h7cad-native-facade** 暴露统一 `load` / `save`。
- **h7cad-native-dwg**、**h7cad-native-builder**、**h7cad-native-testkit**：DWG 相关与构建/测试辅助。

主应用依赖 **iced**（GUI）、**acadrust**（兼容层文档类型）、**glam** / **truck-***（几何与网格）、以及路径依赖的 **pid-parse**（兄弟仓库，P&amp;ID 等能力）。

## 2. 程序入口与双模式

入口在 `src/main.rs`：

- 若 `cli::parse_batch_args` 解析到批处理参数（例如 `--export-pdf` / `--export-svg`），则调用 `cli::run_batch_export`，**不启动窗口**，适合 CI 与脚本。
- 否则调用 `app::run()`，进入 **Iced** 事件循环与完整 UI。

批处理行为与选项结构说明见 `src/cli.rs` 模块级文档注释。

## 3. 应用层（`src/app/`）

`app::H7CAD` 聚合整块桌面应用状态：多文档标签、Ribbon、命令行、状态栏、捕捉与正交/极轴模式、各类浮动窗口 ID、剪贴板等。命令与视口交互在此协调，再下沉到 `Scene` 与 `store`。

与架构强相关的子模块包括：

- `document` / `workspace`：单标签文档生命周期与路径。
- `commands`：命令分发与 CAD 操作编排。
- `view`：将 `Scene` 与 Iced 视图绑定。

## 4. 命令与数据流（简图）

Ribbon / 命令行发出 `ModuleEvent::Command` 或解析后的命令名后，由 `app` 层 `dispatch_command` 一类路径调度到具体实现；实现侧读写 `Scene` 与 `NativeStore`（或过渡期的 `acadrust` 文档），并触发 `Task` 刷新 Iced 视图。详细步进可后续补「序列图」；当前以 [startup-flow](diagrams/h7cad-startup-flow.html) 与 [architecture](diagrams/h7cad-architecture.html) 把握主干即可。

## 5. 模块系统（`src/modules/`）

功能按 **CadModule** 组织；`src/modules/mod.rs` 中说明了添加新模块的步骤：新建目录、实现模块、在 `mod.rs` 中 `pub mod` 声明后，Ribbon 会自动挂接。

`ModuleEvent` 将 Ribbon 工具与宿主行为连接（例如 `Command(String)`、`OpenFileDialog`）。

## 6. 场景与渲染（`src/scene/`）

`Scene` 负责相机、选择集、布局与模型/图纸空间、填充与实体网格的 GPU 侧缓存（`hatches`、`meshes`、`images`）等。

当前处于 **迁移期**：`Scene` 内同时存在：

- `document: acadrust::CadDocument` — 兼容既有渲染与命令路径；
- `native_store: Option<NativeStore>` — 以 `h7cad_native_model::CadDocument` 为前向数据真源；
- `native_render_enabled` 等开关用于调试原生渲染路径。

阅读渲染管线可从 `scene/render.rs`、`scene/pipeline/` 与 `scene/tessellate.rs` 入手。

## 7. 存储抽象（`src/store/`）

`NativeStore` 实现 `CadStore` trait，内部持有 `h7cad_native_model::CadDocument`，对实体与表（图层等）提供统一 CRUD。新功能应优先针对 **native model** 与 `CadStore` 接口设计，减少对 `acadrust` 的直接耦合。

## 8. IO 与外部格式

- DXF：经 **h7cad-native-facade** → **h7cad-native-dxf**。
- P&amp;ID / 发布相关：`src/io/` 下与 **pid-parse** 的集成（具体入口以各子模块为准）。
- 导出：PDF、SVG、打印等分布在 `src/io/` 与 UI 对话框模块中。

## 9. 推荐阅读顺序

1. `src/main.rs` → `src/cli.rs`（若关心无头导出）。
2. `src/app/mod.rs`（状态全貌，可配合搜索 `fn update` / `fn view`）。
3. `src/modules/mod.rs` → 任选一个子模块（如 `home`）看工具如何注册。
4. `src/scene/mod.rs` 中 `Scene` 字段与 `scene/render.rs`。
5. `src/store/native_store.rs` 与 `crates/h7cad-native-model/src/lib.rs` 中的 `CadDocument`。

## 10. 图示风格说明

HTML 图示遵循项目内 **diagram-design** 技能约定：浅色纸底、克制描边、少量强调色节点表示「当前整合焦点」（应用状态与 Scene）。若需将配色对齐品牌，可对该技能中的 `style-guide` 做定制后迭代图示。

---

*文档与图示生成于仓库 `docs/`，与源码版本同步维护；若目录结构重命名，请更新上表中的相对链接。*
