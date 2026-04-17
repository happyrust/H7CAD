# H7CAD Runtime Native Migration Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 将 H7CAD 运行时的单一真源从 `acadrust::CadDocument/EntityType` 迁移为 `h7cad-native-model::CadDocument/Entity`，先让 native DXF 真正进入主程序，再把 DWG 收束到 I/O 边界。

**Architecture:** 迁移分三层推进：先补 `h7cad-native-model` 的编辑态 API，再把 `src/io` 改造成 native-first 边界，最后按 `scene/app/entities/modules` 主链逐段替换运行时类型。过渡期允许 `acadrust <-> native` 只存在于 I/O 边界，不允许进入运行时编辑链路。

**Tech Stack:** Rust 2021, `h7cad-native-model`, `h7cad-native-dxf`, `h7cad-native-facade`, `acadrust`, crate/unit 测试与 `cargo check`.

---

## Summary

1. 先补 native 文档的运行时最小能力：`Clone`、查询、插入、删除、布局路由、owner/handle 维护。
2. 让 `src/io/mod.rs` 具备 native-first 的 `load_file_native/save_native`，兼容路径保留给现有 UI。
3. 修正并扩展 `src/io/native_bridge.rs`，至少保证基础几何、文字、折线、INSERT、VIEWPORT 在 I/O 边界可往返。
4. 后续主链替换按 `scene -> app/history -> entities/dispatch -> modules` 逐批推进。

## Key Changes

- `crates/h7cad-native-model/src/lib.rs`
  - 新增编辑态 `CadDocument` API 与单测。
- `src/io/native_bridge.rs`
  - 增加 `acadrust_doc_to_native()`。
  - 修正角度单位与公共属性映射。
- `src/io/mod.rs`
  - 新增 native-first `load_file_native/save_native`。
  - 旧 `load_file/save` 暂保留为兼容包装。
- `crates/h7cad-native-facade/src/lib.rs`
  - DXF 统一走 `read_dxf_bytes()`。

## Test Plan

- `cargo test -p h7cad-native-model`
- `cargo test --quiet native_to_acadrust_preserves_arc_and_text_rotation_units`
- `cargo test --quiet acadrust_to_native_restores_common_fields_and_rotation_units`
- `cargo test -p h7cad-native-dxf --quiet`
- `cargo check -p H7CAD`

## Assumptions

- DWG 原生解析暂不作为本阶段关键路径，DWG 继续使用 `acadrust` 读写并在 I/O 边界转换。
- 兼容路径会短期保留，直到 `Scene` 与命令系统切换完成。
- 若 `cargo check -p H7CAD` 暴露旧仓噪声，应与本次改动问题分开记录。
