# PIDSAVEAS 扩展 flag（--force / --dry-run）落地计划

> 起稿：2026-04-19  
> 依赖：`PIDSAVEAS <path> [--verify]`（上一轮完成）
>
> **目标**：在 `PIDSAVEAS` 上加两个常用 flag：
>
> - `--force` — 目标路径已存在时，默认拒绝覆盖；加 `--force` 后覆盖
> - `--dry-run` — 不落盘到用户路径，写到 temp 后 verify，再清 temp，报 DRY-RUN PASS/FAIL
>
> 两个 flag 独立可组合（`--verify` 与 `--dry-run` 组合时 dry-run 已隐含 verify，`--verify` 被忽略但不报错）。

---

## 用户故事

> 1. `PIDSAVEAS existing.pid`  
>    → `PIDSAVEAS: destination already exists; use --force to overwrite`
> 2. `PIDSAVEAS existing.pid --force`  
>    → `PIDSAVEAS  saved 4 stream(s) to existing.pid`
> 3. `PIDSAVEAS out.pid --dry-run`  
>    → `PIDSAVEAS  DRY-RUN  4 stream(s) wrote to <temp>; verifying ...`  
>    → `PIDSAVEAS  DRY-RUN  PASS  4 streams matched` （temp 已清理，不落盘）

## 设计

### flag 解析升级

目前 `PIDSAVEAS` 分支用 `strip_suffix("--verify")` 识别 `--verify`。扩展到支持任意顺序的 flag：

```rust
let raw = cmd.strip_prefix("PIDSAVEAS").unwrap_or("").trim();
// Split by whitespace; any token beginning with "--" is a flag, the rest concatenated is path.
```

更稳妥做法：按 token 遍历，把 `--flag` 收集到 `BTreeSet<String>`、非 flag token 拼回 path（保留原空格）。但现实场景 path 含空格的情况罕见，第一版用 **`rsplit_once` 找最后一个非-- token 作 path + 前面所有作 flags** 也太复杂。

**最终方案**（简化）：按空白 split 所有 token；`--force` / `--verify` / `--dry-run` 识别为 flag，剩余**第一个** token 作 path。多个 non-flag token → 取第一个，其它 token 报 warning（第一版可不报）。禁止 path 含空格（和现有 PID 命令一致）。

### 分支决策树

```
parse tokens → { path, flags }
if path empty: usage error

if --dry-run:
    if --force: ignored silently (dry-run never touches real path)
    write to temp
    verify temp
    cleanup temp
    report DRY-RUN PASS/FAIL
else:
    if destination exists && !--force:
        error "destination already exists; use --force to overwrite"
    else:
        save_pid_native
        report success
        if --verify: verify destination
```

### 报告格式

```
PIDSAVEAS  saved 4 stream(s) to out.pid
PIDSAVEAS: destination 'existing.pid' already exists; use --force to overwrite

PIDSAVEAS  DRY-RUN  saved 4 stream(s) to <temp>
PIDSAVEAS  DRY-RUN  PASS  4 streams matched in <temp>
PIDSAVEAS  DRY-RUN  FAIL  1 mismatch(es) in <temp> (matched 3 of 4)
    <stream>  source=... roundtrip=... first diff @ ...
```

## 实施步骤

### Step 1 · 重构参数解析

`PIDSAVEAS` 分支内把 `let rest = …; let (path_str, verify_flag) = strip_suffix …` 替换为：
```rust
let raw = cmd.strip_prefix("PIDSAVEAS").unwrap_or("").trim();
let mut path_opt: Option<String> = None;
let mut verify_flag = false;
let mut force_flag = false;
let mut dry_run_flag = false;
for token in raw.split_whitespace() {
    match token {
        "--verify" => verify_flag = true,
        "--force" => force_flag = true,
        "--dry-run" => dry_run_flag = true,
        _ if token.starts_with("--") => {
            self.command_line.push_error(
                &format!("PIDSAVEAS: unknown flag '{}'", token),
            );
            return Task::none();
        }
        _ if path_opt.is_none() => path_opt = Some(token.to_string()),
        _ => {
            // Extra positional token — first version silently ignores;
            // could tighten later.
        }
    }
}
let path_str = match path_opt { Some(s) => s, None => usage_error };
```

### Step 2 · dry-run 分支

在 path + flag 校验通过后：
```rust
if dry_run_flag {
    let temp = std::env::temp_dir().join(format!(
        "h7cad-pidsaveas-dryrun-{}-{}.pid",
        std::process::id(),
        SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0)
    ));
    if let Err(e) = crate::io::pid_import::save_pid_native(&temp, &source) {
        self.command_line.push_error(&format!("PIDSAVEAS: {e}"));
        return Task::none();
    }
    let stream_count = crate::io::pid_package_store::get_package(&source)
        .map(|p| p.streams.len()).unwrap_or(0);
    self.command_line.push_output(&format!(
        "PIDSAVEAS  DRY-RUN  saved {} stream(s) to {}",
        stream_count, temp.display()
    ));
    match crate::io::pid_import::verify_pid_file(&temp) {
        Ok(report) => {
            if report.ok() {
                self.command_line.push_output(&format!(
                    "PIDSAVEAS  DRY-RUN  PASS  {} streams matched in {}",
                    report.matched, temp.display()
                ));
            } else { /* FAIL output, same as --verify */ }
        }
        Err(e) => { push_error ... }
    }
    let _ = std::fs::remove_file(&temp);
    return Task::none();
}
```

### Step 3 · overwrite 保护

非 dry-run 分支首检：
```rust
if out_path.exists() && !force_flag {
    self.command_line.push_error(&format!(
        "PIDSAVEAS: destination '{}' already exists; use --force to overwrite",
        out_path.display()
    ));
    return Task::none();
}
```

### Step 4 · 测试

H7CAD 端不为命令分支写测试（与其它 dispatch_command 分支一致），但确保底层组合仍过。此前的 `save_pid_native_then_verify_pid_file_always_passes` 已覆盖核心路径；新 flag 都是包装逻辑，不改 helper。**本批不加测试**。

### Step 5 · 落地

- `cargo build` 全绿
- `cargo test io::pid_import` 29/29 仍全绿
- `.memory/2026-04-19.md` 追加段落
- `PIDHELP` 的 `--verify` 描述更新为 `[--verify] [--force] [--dry-run]`

## 不做

1. **dry-run 时的用户可配置 temp 路径**：自动 temp 即可
2. **--dry-run --force 冲突警告**：第一版静默吞掉 force（反正 dry-run 不落真盘）
3. **多位置参数 warning**：额外 token 默默忽略

## 工作量预估

- Step 1 解析重构：10 min
- Step 2 dry-run：15 min
- Step 3 overwrite：5 min
- Step 4-5 落地：10 min

合计 ~40 min。
