# R48-DWG-FACADE-AND-BUILD: facade DWG load 接通 + 主 bin warning 清理

> 起稿：2026-04-28
> 前置：R45-DWG / R46-DWG-LWP / R47-DWG-HANDOFF（第一轮）已完成；
> H7CAD HEAD 主 bin 在 `RUSTFLAGS=-Dwarnings cargo check` 下 fail，
> 但在不带 `-Dwarnings` 时只有 3 个 warning（即生产编译路径其实通），
> 不是阻塞 GUI 运行的 hard error。

## 1. 背景与诚实纠正

R47 后续的 DWG 现状调研（2026-04-28）发现两个值得收口的事实：

1. **次紧迫品质门**：`RUSTFLAGS=-Dwarnings cargo check --workspace --all-targets`
   不通过——H7CAD 主 bin 有 3 个 warning（unreachable pattern + 2 个
   死函数）。这些在普通 `cargo check` 下只是警告、不阻塞编译，但 CI 的
   `-Dwarnings` 卡死了；本轮一次性收口。
2. **文档/代码不一致**：`docs/plans/2026-04-26-native-dwg-next-plan.md` §8
   状态区把 `T3 facade DWG load 接通` 标为 `[x]` 已完成，但
   `crates/h7cad-native-facade/src/lib.rs:12` 实际仍返回
   `"native DWG reader not implemented yet"`。本轮把代码做对、把文档
   状态修正，避免后续读者再被误导。

`facade::Cargo.toml` 已经依赖 `h7cad-native-dwg`，桥接是一行级别的改动；
只是迟迟没落地。

## 2. 范围

| 任务 | 优先级 | 预估 |
|---|---:|---:|
| T1 调研 H7CAD 主 bin 的 3 个 warning 实际位置 + 修复策略 | ✅ 已完成 | 0.2 h |
| T2 落盘本 plan 文件 | P0 | 0.2 h |
| T3a `src/app/commands.rs:6559` 把 `"TP"` 别名合并到 line 4127 的正经分支，删除错误的 "not yet implemented" 占位（4127 是 ribbon-based 正确实现，6559 是过时占位） | P0 | 0.2 h |
| T3b `src/io/mod.rs:402` 删除死函数 `fix_dxf_dimension_rotations`（acadrust 升级后已无需 deg→rad 后处理；唯一调用处早被移除） | P0 | 0.1 h |
| T3c `src/scene/mod.rs:6240` 删除死函数 `paper_boundary_wire`（svg_export.rs 中的 `paper_boundary_wire_is_skipped` 是测试名引用，非函数调用） | P0 | 0.1 h |
| T4 `crates/h7cad-native-facade/src/lib.rs::load(NativeFormat::Dwg, _)` 桥接到 `h7cad_native_dwg::read_dwg(bytes)`；`save` arm 保持 `"native DWG writer not implemented yet"`（自研 writer 仍未实现） | P0 | 0.1 h |
| T4b 更新 facade `dwg_runtime_load_is_unavailable` 测试为 `dwg_runtime_load_returns_minimal_native_document_for_ac1015_signature` 风格，验证 facade DWG load 至少 sniff 出版本错误 / 返回 `CadDocument` | P0 | 0.3 h |
| T5 修订 `docs/plans/2026-04-26-native-dwg-next-plan.md` §8，把 `T3 facade DWG load 接通` 从误标的 `[x]` 改为反映实际进度的注脚（指向本 R48 计划） | P1 | 0.1 h |
| T6 验收：`RUSTFLAGS=-Dwarnings cargo check --locked --workspace --all-targets` + `cargo test --locked --workspace --all-targets` 全过 | P0 | 0.3 h |

## 3. 不纳入

- 不修 `crates/h7cad-native-dwg`（R47 留下的 cursor / handle_offsets 修复
  另起 R49）
- 不接 facade DWG **save**（自研 native writer 不存在，保持 `"not
  implemented yet"`）
- 不接 facade 到 `src/io::load_file_native_blocking` 的 DWG 主路径
  （主路径仍走 acadrust，自研 reader 接 facade 是为了让 facade 测试矩阵
  和 audit 路径少一个 hardcoded "not implemented"）
- 不切换默认 DWG backend
- 不动 AC1018 native reader（R46-DWG-AC1018 范围）
- 不动 R47 已经完成的 sentinel 删除 / baseline 调整 / plan 文件

## 4. 验收

```bash
RUSTFLAGS=-Dwarnings cargo check --locked --workspace --all-targets
cargo test --locked --workspace --all-targets
cargo test -p h7cad-native-facade
cargo test -p h7cad-native-dwg
```

通过标准：

- 上述 4 条命令全部 exit 0；
- `crates/h7cad-native-facade/src/lib.rs` 的 `load(NativeFormat::Dwg, _)`
  不再 hardcode 返回 `"native DWG reader not implemented yet"`；
- `save(NativeFormat::Dwg, _)` 保持显式 `"native DWG writer not
  implemented yet"`，且测试覆盖该返回值；
- `next-plan.md` §8 的 `T3 facade DWG load 接通` 状态修正为反映现实
  （`[x] T3 facade DWG load 接通（R48-DWG-FACADE-AND-BUILD 落地，2026-04-28）`
  或类似真实记录）；
- H7CAD 主 bin 在 `-Dwarnings` 下编译干净；
- 所有 R47 现有测试保持 pass（27 integration + 99 unit + 1 baseline）。

## 5. 风险

- 删除 `paper_boundary_wire` 后如果有运行时通过 reflection/string lookup
  调用它（不可能，但防御性 grep 一遍 svg / pdf / scene 整个 src）—— 已经
  确认 grep 仅有 1 处定义 + 1 处测试名引用，无实际调用方；
- `fix_dxf_dimension_rotations` 删除后如果 acadrust DXF 读取真的需要
  deg→rad 转换，dimension 旋转会显示错——但既然函数 1 年没人调用且
  CI 没回归，acadrust 已经在 reader 内部处理了。删除前先确认它不在任何
  `cfg(...)` feature gate 下被条件调用（grep 已确认）；
- facade 接通后 `dwg_runtime_load_is_unavailable` 测试会反向 fail——
  T4b 必须同步替换为正面测试；
- next-plan.md §8 状态修订要保持 git history 可追，把误标 `[x]` 改成
  `[x] ... (...实际由 R48 落地，详见 ...)` 比直接 `[~]` 或删除好。

## 6. 状态

- [x] T1 主 bin warning 调研（unreachable / 2 dead fn）+ facade Cargo 依赖
  确认（已经依赖 `h7cad-native-dwg`，桥接零增量依赖）
- [x] T2 plan 文件落盘
- [x] T3a commands.rs `"TP"` 合并 + 删 6559 占位
- [x] T3b 删 `fix_dxf_dimension_rotations`（顺便清理 io/mod.rs:18 的
  unused `Dimension` / `EntityType` import）
- [x] T3c 删 `paper_boundary_wire`
- [x] T3d **额外 schema fix**：`-Dwarnings` 让 `cargo check --all-targets`
  暴露了 2 处 schema drift（`WireModel` 缺 `aabb`、`SheetStream` 缺
  `endpoint_decode_error`），都是 test fixture 没跟上上游 schema 变化；
  补 `aabb: WireModel::UNBOUNDED_AABB` / `endpoint_decode_error: None`
- [x] T4 facade DWG load 接通 `h7cad_native_dwg::read_dwg`
- [x] T4b facade DWG load 测试翻面（`dwg_runtime_load_rejects_truncated_signature_with_real_error` +
  `dwg_runtime_save_is_unavailable` 双向覆盖）
- [x] T5 next-plan.md §8 状态修订（标注误标 + 指向本 R48 计划）
- [x] T6 **额外**：`SPPID_SOFTWARE_VERSION` 从 `"0.1.3"` bump 到 `"0.1.7"`
  与 `Cargo.toml` 同步（`sppid_software_version_tracks_cargo_pkg_version`
  test 之前 fail，bump 后 pass；测试 doc 自身建议 "bump 或 relax"，
  bump 是默认选择）
- [x] T7 验收：
  - `RUSTFLAGS=-Dwarnings cargo check --locked --workspace --all-targets`
    全过 ✓
  - `cargo test --locked --workspace --all-targets`：**425 passed / 1 failed**
  - 唯一失败：`app::properties::tests::commit_entity_syncs_native_viewport_in_paper_space`
    （`left=0 right=1`），**pre-existing**，已通过 `git stash` 在干净 HEAD
    上跑同一测试验证仍 fail；与 R48 改动 scope 无关，归为 R49 候选

## 7. 留给后续

- **R49**（建议起 `R49-VIEWPORT-PAPER-SPACE-SYNC`）：修
  `commit_entity_syncs_native_viewport_in_paper_space`——`src/app/properties.rs`
  在 paper-space viewport commit 时未 mirror 到 native document，
  涉及业务逻辑层修改，不在 R48 scope。
- **R50/R47-续**：cursor / handle_offsets 层的 56 个 LINE 失败修复（R47 第二轮
  根因结论）；目标把 `sample_AC1015.dwg` LINE recovery 从 26 推到接近 82。
