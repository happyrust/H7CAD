# DXF Fidelity Diff 盘点清单

> **日期**: 2026-05-06
> **性质**: 只读盘点，不提交、不回滚。
> **验证基线**: `cargo check --workspace`、`cargo test --workspace`、`git diff --check` 均通过。

---

## 1. 总体状态

当前工作树非常大：

- tracked modified 文件约数百个。
- untracked 目录很多，包括 `.agents/`、`.augment/`、`.claude/skills/`、`.cursor/`、`.factory/`、`.kiro/`、`.vscode/`、`.windsurf/`、`vendor_tmp/` 等。
- `git diff --check` 无 whitespace errors。
- Git 输出大量 LF->CRLF 警告，说明 Windows line ending 设置会在 Git touch 时转换。

结论：

当前不适合直接一次性提交。需要按逻辑切片 stage，且默认不纳入大批配置目录和历史计划文件。

---

## 2. 本轮 DXF fidelity 核心文件

这些文件属于本轮明确完成的 DXF fidelity/observability 工作：

- `src/io/diagnostics.rs`
- `src/io/mod.rs`
- `src/entities/ellipse.rs`
- `src/scene/acad_to_truck.rs`
- `src/scene/mod.rs`
- `src/scene/render.rs`
- `src/scene/tessellate.rs`
- `crates/h7cad-native-model/src/lib.rs`
- `crates/h7cad-native-dxf/src/lib.rs`
- `crates/h7cad-native-dxf/src/writer.rs`
- `crates/h7cad-native-dxf/tests/entity_2d_roundtrip.rs`

相关验证/文档：

- `task_plan.md`
- `findings.md`
- `progress.md`
- `docs/plans/2026-05-06-dxf-native-integration-brainstorm-plan.md`
- `docs/plans/2026-05-06-dxf-next-slice-brainstorm-plan.md`
- `docs/plans/2026-05-06-dxf-fidelity-next-brainstorm-plan.md`
- `docs/plans/2026-05-06-dxf-fidelity-diff-inventory.md`

---

## 3. 本轮之前/merge 累积文件

这些文件看起来来自上游 merge、格式化、既有开发或工作树历史，不应和 DXF fidelity 核心切片盲目混在一起：

- `crates/h7cad-native-dwg/**`
- 大量 `src/modules/**`
- 大量 `src/ui/**`
- 大量 `src/app/**`
- `src/cli.rs`
- `tests/cli_batch_export.rs`
- 许多历史 `docs/plans/2026-04-*.md`
- `clippy_full.log`
- `vendor_tmp/`

处理建议：

- 不回滚。
- 不自动提交。
- 若需要提交，单独做 merge/update commit 或让用户确认这些历史改动属于当前分支目标。

---

## 4. 提交切片建议

### Slice A: DXF diagnostics + observability

文件：

- `src/io/diagnostics.rs`
- `src/io/mod.rs`
- `src/scene/mod.rs`
- `task_plan.md`
- `findings.md`
- `progress.md`

验证：

```powershell
cargo test -p H7CAD scene::tests::native_render_stats_classify_native_fallback_and_preserved_entities
cargo test -p H7CAD io
```

### Slice B: native render geometry fidelity

文件：

- `src/entities/ellipse.rs`
- `src/scene/acad_to_truck.rs`
- `src/scene/mod.rs`

验证：

```powershell
cargo test -p H7CAD scene::acad_to_truck
cargo test -p H7CAD entities::lwpolyline
```

### Slice C: DIMSTYLE / Dimension fidelity

文件：

- `crates/h7cad-native-model/src/lib.rs`
- `crates/h7cad-native-dxf/src/lib.rs`
- `crates/h7cad-native-dxf/src/writer.rs`
- `crates/h7cad-native-dxf/tests/entity_2d_roundtrip.rs`
- `src/scene/tessellate.rs`

验证：

```powershell
cargo test -p h7cad-native-dxf --test entity_2d_roundtrip
cargo test -p H7CAD tessellate_native_dimension
```

### Slice D: native nested ByBlock inheritance

文件：

- `src/scene/render.rs`
- `src/scene/mod.rs`

验证：

```powershell
cargo test -p H7CAD nativerender_nested_insert_byblock_uses_nearest_insert_color
cargo test -p H7CAD nativerender_insert
```

### Slice E: planning docs

文件：

- `docs/plans/2026-05-06-dxf-native-integration-brainstorm-plan.md`
- `docs/plans/2026-05-06-dxf-next-slice-brainstorm-plan.md`
- `docs/plans/2026-05-06-dxf-fidelity-next-brainstorm-plan.md`
- `docs/plans/2026-05-06-dxf-fidelity-diff-inventory.md`

可选：

- `task_plan.md`
- `findings.md`
- `progress.md`

---

## 5. 明确不要自动纳入

除非用户明确要求，否则不要 stage：

- `.agents/`
- `.augment/`
- `.claude/skills/`
- `.cursor/`
- `.factory/`
- `.junie/`
- `.kiro/`
- `.memory/`
- `.pi/`
- `.vscode/`
- `.windsurf/`
- `vendor_tmp/`
- `clippy_full.log`
- 大量历史 `docs/plans/2026-04-*.md`

---

## 6. 下一步

推荐顺序：

1. 用户确认是否要提交。
2. 如果要提交，先从 Slice C 或 Slice D 这种边界最清晰的小切片开始。
3. 每个 slice 单独 stage、单独跑对应验证。
4. 最后再跑 `cargo check --workspace` 和 `cargo test --workspace`。
