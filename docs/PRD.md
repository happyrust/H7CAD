# H7CAD 产品需求文档（PRD）

| 字段 | 内容 |
|------|------|
| 文档版本 | v1.0（首版） |
| 对应代码版本 | `Cargo.toml` v0.1.3，`main` @ `9540b2a`（round 39） |
| 文档状态 | Draft — 与代码同次迭代维护 |
| 适用读者 | 产品经理、技术负责人、贡献者、运维 / QA |
| 维护方式 | 与 [`docs/DEVELOPMENT-PLAN.md`](DEVELOPMENT-PLAN.md) 联动；阶段目标变更先改本文件再改代码 |

---

## 1. 产品愿景

打造一款 **以 Rust 为唯一语言** 的开源 2D/3D CAD 桌面应用，原生支持 AutoCAD 生态最常见的输入输出（DXF / DWG / PDF / SVG），并能作为 SmartPlant P&ID 等工业 CAD 链路的下游工具。目标：

- 让中文区工程师拥有一个 **可阅读源码、可二次开发、可商用合规** 的 CAD 选项；
- 把绘图、出图、批处理三条链路放在 **同一个二进制**，既可桌面 GUI、也可 CI 脚本；
- 通过明确的分层架构（facade → dxf/dwg → model）逐步替换历史 `acadrust` 依赖，最终实现 **100% native 数据真源**。

## 2. 范围与非目标

### 2.1 当前 In-Scope（v0.1.x）

| 类别 | 范围 |
|---|---|
| 绘制 | LINE / PLINE / ARC / CIRCLE / ELLIPSE / RECTANGLE / POLYGON / SPLINE / MTEXT / HATCH / GRADIENT 等共 20 个 ✅ |
| 修改 | MOVE / COPY / ROTATE / SCALE / MIRROR / OFFSET / TRIM / EXTEND / FILLET / CHAMFER / ARRAY 系列 30 个 ✅ |
| 标注 | DIMLINEAR / DIMALIGNED / DIMANGULAR / DIMRADIUS / DIMDIAMETER / MLEADER 等 22 个 ✅ |
| 图层 | LAYER / LINETYPE / LTSCALE / LAYISO 系列 12 个 ✅ |
| 块 / 外参 | BLOCK / INSERT / WBLOCK / XATTACH / XREF / REFEDIT / ATTDEF 11 个 ✅ |
| 3D | BOX / SPHERE / CYLINDER / EXTRUDE / REVOLVE / SWEEP / LOFT / EXPORTSTEP / EXPORTSTL 10 个 ✅ |
| 视图导航 | ZOOM / PAN / ORBIT / VPORTS / MSPACE / PSPACE 等 10 个 ✅ |
| IO | DXF 读写 ✅，PDF 出图 ✅（含 HATCH pattern + gradient strip-fill），SVG 出图 ✅，STEP / STL ✅，PID 入包 ✅ |
| PID / SPPID | `PIDSETDRAWNO` / `PIDSETPROP` / `PIDGETPROP` / `PIDSETGENERAL` / `SPPIDLOADLIB` / `SPPIDBRANDEMO` / `SPPIDEXPORT` 7 个 ✅ |
| 批处理 CLI | `--export-pdf` / `--export-svg`，多输入，`--options <PATH>` JSON 覆盖 ✅ |
| 平台分发 | Linux Flatpak（HakanSeven12.H7CAD），macOS / Windows 源码构建 ✅ |

完整命令支持矩阵见 [`COMMANDS.md`](../COMMANDS.md)（256 条 / 141 ✅ / 24 🔶 / 91 ❌）。

### 2.2 Out-of-Scope（短期不做）

- 高级 3D 布尔（UNION / SUBTRACT / INTERSECT）与曲面建模（PRESSPULL / THICKEN）。
- 渲染管线（RENDER / MATERIALS / LIGHT / SUNPROPERTIES / VISUALSTYLES）。
- LISP / ARX / .NET 插件运行时（`APPLOAD` / `NETLOAD`）。
- 数据库连接 / Action Recorder（`DBCONNECT` / `ACTRECORD`）。
- 真正可用的 **DWG 运行时打开**：解析层在 `crates/h7cad-native-dwg` 内部演进，但 `h7cad-native-facade::load(Dwg, …)` 当前刻意返回 `native DWG reader not implemented yet`，待阶段 P2 完成后才向用户放行。
- 云协作 / 多人实时同步。

## 3. 目标用户与场景

### 3.1 用户画像

| 画像 | 主要诉求 | 使用频率 |
|---|---|---|
| **机械 / 建筑 / 电气工程师** | 在没有 AutoCAD 授权的环境下做 2D 绘图、出 PDF / 蓝图 | 日常 |
| **工艺 / SPPID 工程师** | 用我方工具读写 SmartPlant `.pid` 文件元数据，做 BRAN demo | 项目期 |
| **CI / DevOps 工程师** | 把 `*.dxf` 在流水线里批量转 PDF / SVG，无需启动 X11 | 持续 |
| **CAD 二次开发者** | 阅读 Rust 源码，扩展命令、改渲染、嵌入到自家产品 | 持续 |
| **教学 / 自学者** | 学习 CAD 实现原理，需可读架构文档与图示 | 偶发 |

### 3.2 关键场景

1. **桌面绘图**：双击 `.dxf` → Iced 窗口打开，使用 Ribbon 中的 Home / Annotate / Insert 工具栏完成绘制 → `SAVE` 回写 DXF。
2. **批量出图**：CI 中执行 `h7cad A.dxf B.dxf C.dxf --export-pdf out_dir/`，无窗口完成转换；可通过 `--options opts.json` 关闭 hatch pattern / gradient 等高保真特性以追求更快或更小输出。
3. **P&ID 元数据维护**：在已有 `.pid` 包上用 `PIDSETDRAWNO / PIDSETPROP` 编辑 SmartPlant 属性、再 `SPPIDEXPORT` 出 `.pid + _Data.xml + _Meta.xml` 三件套。
4. **3D 体出工程图**：`BOX/CYLINDER/EXTRUDE` → `EXPORTSTEP` 或 `EXPORTSTL` 转给下游 CAM / FDM。
5. **嵌入二次开发**：把 `crates/h7cad-native-model` + `h7cad-native-dxf` 作为 path 依赖单独引入，复用 DXF 读写而不携带 GUI。

## 4. 产品架构（与 `ARCHITECTURE-TUTORIAL.md` 对齐）

```
┌─────────────────────────────────────────────────────────┐
│ UI 层  Ribbon / 命令行 / 浮动窗口（src/ui, src/modules）│
├─────────────────────────────────────────────────────────┤
│ App  H7CAD 状态机：文档、捕捉、剪贴板（src/app）        │
├─────────────────────────────────────────────────────────┤
│ Scene  相机、选区、tessellate、GPU 缓存（src/scene）    │
├─────────────────────────────────────────────────────────┤
│ Store  CadStore trait + NativeStore（src/store）        │
├─────────────────────────────────────────────────────────┤
│ Model  h7cad-native-model::CadDocument（迁移后真源）    │
├─────────────────────────────────────────────────────────┤
│ Format  h7cad-native-dxf / -dwg via -facade            │
├─────────────────────────────────────────────────────────┤
│ Sibling  pid-parse（P&ID / SPPID 出包）                 │
└─────────────────────────────────────────────────────────┘
```

入口分流（`src/main.rs`）：

- 命中 `cli::parse_batch_args` → `cli::run_batch_export`，**不启窗**；
- 否则 `app::run()`，进入 Iced 0.14 主循环。

迁移期同时保留 `acadrust::CadDocument` 与 `h7cad_native_model::CadDocument`，新命令优先经 `CadStore` 写 native 真源，最终目标见 `docs/plans/2026-04-17-acadrust-removal-plan.md`。

## 5. 功能需求（FR）

### 5.1 必备能力（P0 — 已满足）

| ID | 需求 | 验收 |
|----|------|------|
| FR-DRAW-1 | 提供 LINE / PLINE / ARC / CIRCLE / RECTANGLE / SPLINE / HATCH / MTEXT 等基础绘制命令 | `cargo test` 全绿，`COMMANDS.md` 标 ✅ |
| FR-MOD-1 | MOVE / COPY / ROTATE / SCALE / MIRROR / OFFSET / TRIM / EXTEND / FILLET / CHAMFER / ARRAY | 同上 |
| FR-LAYER-1 | LAYER / LINETYPE / LTSCALE，支持冻结 / 锁定 / 关闭 | UI 与 LAYER 管理器跑通 |
| FR-DIM-1 | 8 类基本标注 + DIMSTYLE 风格管理 | DXF 圆 trip 后标注几何 / 文本不丢 |
| FR-IO-DXF | DXF 读、改、写零数据丢失（在已实现实体范围内） | `h7cad-native-dxf` 单测 + golden |
| FR-IO-PDF | DXF / 当前文档导出 PDF，支持 HATCH pattern、gradient strip-fill、SPLINE 原生贝塞尔 | round 32–39 PDF Phase 1–3 收口测试 |
| FR-IO-SVG | DXF / 当前文档导出 SVG，与 ODA `OdSvgExportEx` 行为对齐 | `docs/svg_export.md` 用例集合 |
| FR-CLI-BATCH | 单 / 多输入 DXF → PDF / SVG，支持 `--options <PATH>` JSON 覆盖 | `tests/` 中 CLI 集成测试 |
| FR-PID-MIN | `PIDSETDRAWNO` / `PIDSETPROP` / `PIDGETPROP` / `PIDSETGENERAL` 四件套 + `SPPIDEXPORT` 出包 | 与 `pid-parse` 联调 + `_Data.xml` / `_Meta.xml` golden |
| FR-PLATFORM | Linux Flatpak 安装；macOS / Windows 源码 `cargo build --release` 一次成功 | Release artifact + 手动验证 |

### 5.2 期望能力（P1 — 进行中）

| ID | 需求 | 备注 |
|----|------|------|
| FR-NATIVE-1 | 明确 `Scene.native_store` 与 `Scene.document` 同步策略，关键路径加单测 | 对应 `docs/DEVELOPMENT-PLAN.md` P1.1 |
| FR-NATIVE-2 | 新命令一律走 `CadStore` / `nm::CadDocument`，老命令分批迁移 | P1.2 |
| FR-NATIVE-3 | 收敛 `native_render_enabled` 双份渲染逻辑 | P1.3 |
| FR-PROPS | `PROPERTIES` 面板从 stub 升级为可编辑实体属性的真功能（当前 🔶） | 高复杂度（见 ROADMAP） |
| FR-OVERKILL | `OVERKILL` 实现真实重复几何清理（当前 🔶） | High |
| FR-AUDIT | `AUDIT` 真正校验并修复文档完整性（当前 🔶） | High |
| FR-CMD-COVERAGE | Draw / Modify / Dim 缺失项收口（如 `REGION` / `DIMCENTER` / `CENTERLINE` / `3DPOLY`） | 按 `COMMANDS.md` ❌ 项 |

### 5.3 长期能力（P2/P3 — 计划中）

| ID | 需求 | 阶段 |
|----|------|------|
| FR-DWG-LOAD | `h7cad_native_facade::load(Dwg, …)` 返回真实 `nm::CadDocument` 或结构化错误，UI 能打开 `.dwg` | P2.1–P2.3 |
| FR-DWG-SAFETY | DWG 大文件 / 恶意输入安全边界（OOM 防御、错误提示） | P2.3 |
| FR-CLI-COVER | 批处理 CLI 回归扩到 `--options` 全字段 + 多输入边界 | P3.1 |
| FR-GOLDEN | 关键命令 golden / 快照测试，借助 `h7cad-native-testkit` | P3.2 |
| FR-DOCS-LINK | CI 中文档链接检查 | P3.3 |
| FR-3D-BOOL | UNION / SUBTRACT / INTERSECT / SLICE 等 3D 布尔实现 | 待立项 |
| FR-RENDER | RENDER / MATERIALS / LIGHT / VISUALSTYLES | 待立项 |

完整未实现命令清单与复杂度评估见 [`ROADMAP.md`](../ROADMAP.md)。

## 6. 非功能需求（NFR）

| 维度 | 目标 | 说明 |
|---|---|---|
| **性能** | 10k 实体 DXF 在 M1 / 8 GB 机器首屏 < 2 s；`PAN/ZOOM` 60 FPS 不掉帧 | WebGPU + tessellate 缓存；多实体测试用例待补 |
| **稳定性** | `cargo test --workspace` 全绿是合并门槛；P0 阶段每阶段结束前要求 `cargo check` / `cargo test` 全绿（豁免须显式记录） | 见 DEVELOPMENT-PLAN.md |
| **可读性** | 关键 crate `pub` 暴露面要带 doc；架构文档与图示与代码同次迭代 | 已有 `docs/diagrams/*.html` |
| **可移植性** | Tier-1 平台：x86_64-linux / aarch64-darwin / x86_64-windows | Flatpak 仅做 Linux 发行 |
| **可扩展性** | 新增 ribbon 模块仅需 `src/modules/<name>/` + 在 `mod.rs` 中 `pub mod`，Ribbon 自动挂接 | 见 ARCHITECTURE-TUTORIAL §5 |
| **合规** | 全仓 GPL-3.0-only；任何引入更宽松或更严格协议的依赖须在 PR 描述里声明 | `LICENSE` |
| **构建** | Rust 1.75+；`cargo build --release` 单条命令出可执行 | README §Installation |
| **国际化** | 内部错误 / 命令保持英文，文档与计划允许中文；UI 文案以英文为主 | 当前实状 |
| **可观测性** | 启动 / 命令派发关键路径有 `log::*`，错误统一通过 `app/document` 流到状态栏 | 已有 `src/io/diagnostics.rs` |

## 7. 技术约束

- **编程语言**：Rust（edition 2021，workspace `resolver = "2"`）。
- **GUI 栈**：`iced 0.14`（含 `image`/`svg`/`canvas`/`advanced`/`debug` features）；不引入第二个 GUI 框架。
- **几何 / 数学**：`glam 0.27` + `truck-modeling 0.6` + `truck-meshalgo 0.4`（仅 `tessellation`）+ `truck-polymesh 0.6`。
- **PDF 输出**：`printpdf 0.9` + `flate2 1` 内联实现，避免引入额外 C 依赖。
- **图像**：`image 0.25`，仅启用 `png/jpeg/bmp/tiff`。
- **文件对话框**：`rfd 0.15`。
- **过渡兼容层**：`acadrust 0.3.3` 仍存在，但任何新代码不得增加对它的耦合（参见 `2026-04-17-acadrust-removal-plan.md`）。
- **同仓兄弟 crate**：`pid-parse`（path 依赖），用于 SmartPlant P&ID 出入包，不得在 H7CAD 主路径中重复实现。
- **平台 API**：仅在 Windows 启用 `windows-sys 0.59`（`Win32_UI_Shell` / `Win32_UI_WindowsAndMessaging`），其它平台保持纯 Rust。
- **测试**：DWG 解析单测 `cargo test -p h7cad-native-dwg -- --test-threads=1`；facade 编译门 `cargo check -p h7cad-native-facade`。
- **CHANGELOG**：每"轮"工作合入 `CHANGELOG.md`，PR 描述引用阶段 ID（如 `P1.2: ...`）。

## 8. 用户体验要点

- **Ribbon 五大 Tab**：Home / Annotate / Insert / View / Manage —— 严格映射到 `src/modules/{home,annotate,insert,view,manage}`，工具按钮的 `command` 属性即 [`COMMANDS.md`](../COMMANDS.md) 中条目。
- **命令行**：底部命令行接受 AutoCAD 风格命令名 + 别名（如 `L` = `LINE`），不区分大小写。
- **Layout 切换**：模型空间 / 图纸空间通过 `MSPACE` / `PSPACE`，多布局 tab 通过 `LAYOUTTAB`（当前 🔶）。
- **报错 UX**：DWG 打开当前必须明确提示 "native DWG reader not implemented yet"，避免静默失败；后续 P2 替换为真实加载或结构化错误。
- **批处理输出**：CLI 单输入可省略 output 路径（自动同 stem 改后缀）；多输入时 output 必须是目录或省略。

## 9. 里程碑规划

与 [`docs/DEVELOPMENT-PLAN.md`](DEVELOPMENT-PLAN.md) 一一对应：

| 阶段 | 目标摘要 | 状态 |
|------|----------|------|
| **P0 协作基线** | 架构教程 + 图示、文档索引、计划可追踪 | P0.1 已完成；P0.2 / P0.3 随本 PRD 落地 |
| **P1 Native 数据路径硬化** | `Scene.native_store` 同步策略、新命令走 `CadStore`、收敛 `native_render_enabled` | 进行中 |
| **P2 DWG 运行时（facade 层）** | 解析结果 → `nm::CadDocument`、facade 公开 load、UI/CLI 打开 `.dwg` | 未开始 |
| **P3 质量与自动化** | CLI 回归扩展、命令 golden、文档链接检查 | 部分（CLI 回归已有基线） |

> 每阶段必须保持 `cargo test` / `cargo check` 全绿；豁免项需在阶段总结的 PR 描述里显式标注。

## 10. 验收标准（DoD）

- [ ] `cargo build --workspace --release` 在 macOS / Linux / Windows 任一 Tier-1 平台成功。
- [ ] `cargo test --workspace` 全绿（包含 `h7cad-native-dwg` 单线程门）。
- [ ] `COMMANDS.md` 中标 ✅ 的命令在 GUI 与命令行都可被触发并产生符合预期的几何 / 文档变更。
- [ ] 批处理 CLI 五条最小用例通过：单输入 PDF、单输入 SVG、多输入到目录、多输入到 stem、`--options opts.json` 关闭 gradient_hatches 后字节级回退到 R39 之前。
- [ ] PID 最小用例：在样例 `.pid` 上 `PIDSETDRAWNO` → `PIDGETPROP` → `SPPIDEXPORT` 出包成功，`_Data.xml` / `_Meta.xml` 与 golden 一致。
- [ ] 对外文档同步：本 PRD、`README.md`、`docs/ARCHITECTURE-TUTORIAL.md`、`docs/DEVELOPMENT-PLAN.md`、`COMMANDS.md`、`ROADMAP.md` 在每个 release tag 上不矛盾。

## 11. 风险与对策

| 风险 | 影响 | 对策 |
|------|------|------|
| `acadrust` 残留耦合长期化 | 阻碍 P1 收口，渲染/命令双份逻辑维护成本翻倍 | 严守"新命令禁止新增 `acadrust` 耦合"；按 `2026-04-17-acadrust-removal-plan.md` 节奏拆迁 |
| DWG 运行时复杂度被低估 | P2 跳票，向用户曝出半成品 | facade 层保留 `native DWG reader not implemented yet`，直至解析 + 错误路径 + 大文件防御均完成 |
| 自定义 PDF / SVG 输出与 ACAD 视觉差异 | 用户报"不一致" | 每轮 PDF/SVG 改动写明 trade-off（如 gradient strip-fill 裁到 AABB 而非 boundary），并提供 `--options` 回退 |
| GPL-3.0 与下游闭源集成冲突 | 外部用户无法再封装 | 在 README 与 PRD 明确许可证；如有商业需求，单独走双许可方案讨论 |
| 命令支持矩阵长期 91 项缺失 | 用户视为"半成品" | 用 [`COMMANDS.md`](../COMMANDS.md) + [`ROADMAP.md`](../ROADMAP.md) 复杂度标注，按"高频 + 低复杂度"优先排期 |
| Iced 0.14 与 WebGPU 的栈升级 | API 破坏导致渲染回归 | 升级前先在分支跑全量 `cargo test`，CHANGELOG 单列升级条目 |

## 12. 度量与成功指标

- **能力覆盖率**：`COMMANDS.md` 中 ✅ 比例（当前 141/256 ≈ 55%）目标 v0.2 达 65%、v1.0 达 85%。
- **代码质量门**：`cargo test --workspace` 在 main 分支历史命中率 ≥ 95%。
- **生态采纳**：Flathub 月度安装量、GitHub Star、外部 fork 中以 `h7cad-native-*` 作为 path 依赖的二次工程数。
- **稳定性**：从 v0.1.x 到 v0.2 的 release 周期内，与 DXF / PID 相关 issue 严重 bug（数据丢失类）不超过 0 起。
- **文档健康**：架构文档 + 图示在每次 release 与代码同步；P3.3 文档链接检查 CI 引入后保持 0 broken。

## 13. 参考文档

- [`README.md`](../README.md) — 用户视角的产品概览与安装。
- [`COMMANDS.md`](../COMMANDS.md) — 全量命令支持矩阵（256 条）。
- [`ROADMAP.md`](../ROADMAP.md) — Ribbon 已暴露但缺实现的命令清单。
- [`docs/ARCHITECTURE-TUTORIAL.md`](ARCHITECTURE-TUTORIAL.md) — 架构教程与图示索引。
- [`docs/DEVELOPMENT-PLAN.md`](DEVELOPMENT-PLAN.md) — P0–P3 阶段计划。
- [`docs/svg_export.md`](svg_export.md) — SVG 导出与 ODA 行为对齐说明。
- [`docs/diagrams/h7cad-architecture.html`](diagrams/h7cad-architecture.html) — 系统分层图。
- [`docs/diagrams/h7cad-startup-flow.html`](diagrams/h7cad-startup-flow.html) — 双入口流程图。
- [`docs/diagrams/h7cad-crates-layers.html`](diagrams/h7cad-crates-layers.html) — workspace crate 分层图。
- [`docs/plans/`](plans/) — 单功能 / 单轮次工作的执行计划。
- [`CHANGELOG.md`](../CHANGELOG.md) — 每轮工作的实现笔记与 trade-off。

---

*本 PRD 与代码同次迭代维护。如阶段目标变化，先改本文件再改代码，避免口头约定漂移；条目变更需在 PR 描述里引用阶段 ID。*
