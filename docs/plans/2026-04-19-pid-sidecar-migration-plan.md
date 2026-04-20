# 开发计划：`.pid` Save-As 同迁移 publish sidecar

> 起稿：2026-04-19  
> 背景：今日 H7CAD × SPPID 集成分析识别的 P2 用户体验 bug。`save_pid_native(dst, src)` 写出 `.pid` 时**未处理同目录的 publish sidecar**（`{stem}_Data.xml` / `{stem}_Meta.xml`），导致"另存为到新目录"后 re-open 新 .pid 丢失 sidecar 里承载的对象图增强数据。

## 现状（经 grep 验证）

```rust
// src/io/pid_import.rs:1051-1066
pub fn save_pid_native(path: &Path, source_path: &Path) -> Result<(), String> {
    let package = pid_package_store::get_package(source_path).ok_or_else(...)?;
    // 确保 parent 目录存在
    PidWriter::write_to(&package, &WritePlan::default(), path).map_err(|e| e.to_string())?;
    Ok(())
}
```

只写 .pid，不复制 sidecar。

sidecar 命名：
- `publish_data_path(pid_path)` → `{stem}_Data.xml`（同目录）
- `publish_meta_path(pid_path)` → `{stem}_Meta.xml`（同目录）

open 消费：`merge_publish_sidecars(path)` 要求两个 sidecar 都存在（缺一抛错）或都不在（静默跳过）。

## 复现场景

```
A/drawing.pid                (H7CAD publish 产物)
A/drawing_Data.xml           (同上)
A/drawing_Meta.xml           (同上)

用户：Save As → B/renamed.pid
结果：
B/renamed.pid                ✓ 写入 byte-level 保真
B/renamed_Data.xml           ✗ 缺失
B/renamed_Meta.xml           ✗ 缺失

re-open B/renamed.pid → merge_publish_sidecars 找不到 sidecar → 静默
      丢失 publish 时存入 sidecar 的对象图增强 / summary.title 覆盖 / …
```

## 修复目标

1. `save_pid_native(dst, src)` 在写完 .pid 后，若 `publish_data_path(src).exists() && publish_meta_path(src).exists()`，将它们**复制**到 `publish_data_path(dst)` / `publish_meta_path(dst)`（按 dst 的 stem 重命名）
2. 若只有一侧 sidecar 存在，视为异常，抛 `"incomplete publish bundle ..."` 与 `merge_publish_sidecars` 一致的错误
3. 若两侧都不存在，无操作（不是所有 .pid 都是 publish 产物，直接打开的 SmartPlant 原生 .pid 无 sidecar）
4. 不触动现有 `save_pid_native` 签名（保持调用方稳定）

## 非目标

- 不改 `publish_data_path` / `publish_meta_path` 算法（后缀仍 `_Data.xml` / `_Meta.xml`）
- 不合并 sidecar 到 `.pid` 的自定义 stream（会破坏与 SmartPlant 消费方的交互协议，要做需专项大 Phase）
- 不添加 SQLite / Index 之类 sidecar 元数据库
- 不改 `export_sppid_publish_bundle` 的 sidecar 生成逻辑
- 不调整 `merge_publish_sidecars` 的"一侧缺失抛错"语义

## 实施步骤

### M1 — 扩展 `save_pid_native`（15 min）

在 `src/io/pid_import.rs` 替换现有实现：

```rust
pub fn save_pid_native(path: &Path, source_path: &Path) -> Result<(), String> {
    let package = pid_package_store::get_package(source_path).ok_or_else(|| {
        format!(
            "PID save requires the original .pid file to be opened first (no cached package for {})",
            source_path.display()
        )
    })?;

    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("failed to create destination directory: {e}"))?;
        }
    }

    PidWriter::write_to(&package, &WritePlan::default(), path).map_err(|e| e.to_string())?;

    copy_publish_sidecars_if_present(source_path, path)?;

    Ok(())
}

/// Mirror publish sidecars (`*_Data.xml` / `*_Meta.xml`) from the source
/// `.pid` directory to the destination's directory, renamed to match the
/// destination's stem. A no-op when no sidecars exist next to the source.
///
/// Errors out on an incomplete sidecar pair (only one of the two exists)
/// — mirrors the "both or nothing" contract used by
/// [`merge_publish_sidecars`].
fn copy_publish_sidecars_if_present(src: &Path, dst: &Path) -> Result<(), String> {
    let src_data = publish_data_path(src);
    let src_meta = publish_meta_path(src);
    let has_data = src_data.exists();
    let has_meta = src_meta.exists();

    if !has_data && !has_meta {
        return Ok(());
    }
    if has_data != has_meta {
        return Err(format!(
            "incomplete publish bundle beside {} (expected both {} and {})",
            src.display(),
            src_data.display(),
            src_meta.display()
        ));
    }

    if src == dst {
        // Same-file save: both sidecars already at their target names.
        return Ok(());
    }

    let dst_data = publish_data_path(dst);
    let dst_meta = publish_meta_path(dst);

    std::fs::copy(&src_data, &dst_data)
        .map_err(|e| format!("failed to copy publish Data.xml: {e}"))?;
    std::fs::copy(&src_meta, &dst_meta)
        .map_err(|e| format!("failed to copy publish Meta.xml: {e}"))?;

    Ok(())
}
```

### M2 — 集成测试（30 min）

在 `src/io/pid_import.rs` 的 `#[cfg(test)] mod tests` 追加 3 个测试。pattern 参考已有 `save_pid_native_then_verify_pid_file_always_passes` 的 tempdir 用法。

- `save_pid_native_copies_sidecars_when_both_present`：构造 src.pid + src_Data.xml + src_Meta.xml → save → 断言 dst 旁生成了 dst_Data.xml / dst_Meta.xml 且内容 byte-identical
- `save_pid_native_no_ops_when_sidecars_absent`：只有 src.pid，无 sidecar → save 成功，dst 目录无额外 sidecar
- `save_pid_native_errors_on_incomplete_sidecar_pair`：只有 src_Data.xml 没 Meta → save 返回 Err 含"incomplete publish bundle"

其中第一条必须验证 dst 的 basename 重命名正确（e.g. `src/drawing.pid` + `drawing_Data.xml` → `dst/renamed.pid` + `renamed_Data.xml`）。

### M3 — build + test + CHANGELOG + commit（15 min）

1. `cargo check --all-targets -p H7CAD`
2. `cargo test --lib io::pid_import` —— 至少新增 3 个测试全绿
3. 更新 `CHANGELOG.md` Unreleased 段
4. 更新 `.memory/2026-04-19.md` 记录
5. 使用 PowerShell heredoc 回避模式（写 `.git/COMMIT_MSG_TMP.txt` + `git commit -F`）

## 预计工时

| 步骤 | 估时 |
|---|---|
| 写 plan | 完成 |
| M1 | 15 min |
| M2 | 30 min |
| M3 | 15 min |
| **合计** | **~1 hr** |

## 风险与缓解

| 风险 | 缓解 |
|---|---|
| 新的 `copy_publish_sidecars_if_present` 对 src == dst 情况 double-copy（会失败） | 显式 `if src == dst { return Ok(()) }` 短路；且有单测覆盖 |
| 新路径使 `save_pid_native` 多一个文件 I/O 失败点 | fs::copy 失败走 Err，调用方既有错误路径可处理；不引入静默 swallow |
| sidecar 复制与原文件 mtime 差异导致下游工具误判"新鲜度" | 业务层面 sidecar 本应与 .pid 同时生成/保存；mtime 差异小于 ms 量级可接受 |

## 回滚

单文件改动（`src/io/pid_import.rs` + 同文件单测）。`git revert` 即可。无 DB 迁移，无 public API break（函数签名不变）。

## 下一步候选（仅排队，本计划不做）

- 改进点 1：`SPPID_TOOL_ID` / `SPPID_SOFTWARE_VERSION` 改 `env!` 注入
- 改进点 3：`pid_package_store` 加 LRU 上限
- 改进点 4：提供 headless save path 不依赖 store（大改）
- 与 pid-parse `--apply-plan` CLI 的下游消费对齐（跨仓库）
