# H7CAD 开发计划（草案）

面向 **原生模型迁移**、**DWG 运行时接入** 与 **可维护性** 的分阶段路线。优先级自上而下；每阶段结束前应保持 `cargo test` / `cargo check` 全绿（或明确记录已知豁免项）。

## 阶段 P0 — 协作基线（当前已启动）

| ID | 交付物 | 完成标准 |
|----|--------|----------|
| P0.1 | 架构教程与图示 | `docs/ARCHITECTURE-TUTORIAL.md` + `docs/diagrams/*.html` 可浏览；`README` 链到开发者文档 |
| P0.2 | 文档索引 | `docs/README.md` 汇总本目录入口 |
| P0.3 | 计划可追踪 | 本文件纳入版本控制；重大变更在 PR 描述中引用对应阶段 |

**执行中**：P0.1 已完成；P0.2 / P0.3 随本提交落地。

## 阶段 P1 — Native 数据路径硬化

| ID | 目标 | 备注 |
|----|------|------|
| P1.1 | 明确 `Scene` 中 `native_store` 与 `acadrust::CadDocument` 的同步策略 | 文档化 + 关键路径单测 |
| P1.2 | 新命令优先走 `CadStore` / `nm::CadDocument` | 按命令逐个迁移，避免大爆炸式 PR |
| P1.3 | `native_render_enabled` 调试路径与主路径差异收敛 | 减少双份渲染逻辑 |

## 阶段 P2 — DWG 运行时（facade 层）

| ID | 目标 | 备注 |
|----|------|------|
| P2.1 | `h7cad_native_dwg` 解析结果映射到 `nm::CadDocument` | 与 README「Native DWG Parser Status」对齐 |
| P2.2 | `h7cad_native_facade::load(Dwg, …)` 返回真实文档或结构化错误 | 替换占位字符串前更新测试期望 |
| P2.3 | GUI / CLI 打开 `.dwg` 的 UX 与错误提示 | 需产品文案与安全边界（大文件、恶意输入） |

## 阶段 P3 — 质量与自动化

| ID | 目标 | 备注 |
|----|------|------|
| P3.1 | 批处理 CLI 回归用例扩充 | 覆盖 `--options` 与多输入 |
| P3.2 | 关键命令 golden / 快照测试（若适用） | 与 `h7cad-native-testkit` 协同 |
| P3.3 | CI 中文档链接检查（可选） | 仅当仓库引入 link checker 时启用 |

---

## 如何认领工作

1. 在 issue 或 PR 标题中标注阶段 ID（例如 `P1.2: migrate LINE store path`）。  
2. 合并前在 PR 描述勾选本文件中对应条目或说明范围调整原因。  
3. 若阶段目标变化，先改本文件再改代码，避免口头约定漂移。

*最后更新：与仓库 `docs/` 同次迭代维护。*
