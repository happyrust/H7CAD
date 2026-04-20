# H7CAD PIDSAVEAS 命令落地计划

> 起稿：2026-04-19  
> 依赖：H7CAD `pid_import::save_pid_native` / `verify_pid_file`（同日早些时候完成）
>
> **目标**：在 H7CAD 命令行引入专用的 `PIDSAVEAS` 命令，原子地完成"另存 PID 到目标路径 + 可选立刻 round-trip 验证"闭环：
>
> ```
> PIDSAVEAS new.pid             ← 仅保存
> PIDSAVEAS new.pid --verify    ← 保存后立即 round-trip 验证输出文件
> ```
>
> 对比现有路径：目前"另存为 .pid"靠通用 SAVEAS → `helpers.rs::save_active_tab_to_path` 的 `.pid` 分支识别；用户要验证还得再手动 `PIDVERIFY new.pid`。新命令把两步合一。

---

## 现状盘点

* `pid_import::save_pid_native(path, source_path)`：从 cache 取 PidPackage，`PidWriter::write_to` 写入 path
* `pid_import::verify_pid_file(path)`：对磁盘任意 .pid 做 round-trip 验证（不依赖 cache）
* `helpers.rs::save_active_tab_to_path`：通用 SAVE/SAVEAS 入口，已有 PID 分支
* 缺：把二者串成一个用户可直接调用的命令，并提供 `--verify` flag

## 用户故事

> 1. `OPEN drawing.pid`
> 2. `PIDSETPROP SP_REVISION 2`
> 3. `PIDSAVEAS drawing-rev2.pid --verify`  
>    → `PIDSAVEAS  saved 4 streams to drawing-rev2.pid`  
>    → `PIDVERIFY  PASS  4 streams matched in drawing-rev2.pid`
> 4. （失败路径）`PIDSAVEAS /nonexistent/dir/x.pid` → 清晰的错误行

## CLI 语法

```
PIDSAVEAS <path>          ← 必需；保存当前 active tab 的 cached PidPackage
PIDSAVEAS <path> --verify ← 保存后立即对输出文件跑 verify_pid_file
```

参数规则：
- `<path>` 必须以 `.pid`（大小写不敏感）结尾，否则 push_error
- `<path>` 可含空格（用 `splitn(2, ' ')` + 末尾 `--verify` 识别）。简化：先识别 `--verify` 是否作为**最后一个 token** 存在，然后把 command 剩余部分（去掉 `--verify`）trim 作为 path
- 空路径 → push_error usage

## 设计

### 命令注册

`src/app/commands.rs`，在 `PIDVERIFY` 之后插入：
```rust
cmd if cmd == "PIDSAVEAS" || cmd.starts_with("PIDSAVEAS ") => {
    // 1. 解析 args：path + optional --verify
    // 2. 校验 .pid 后缀 + active tab 有 current_path + source is .pid
    // 3. 调 save_pid_native(&out_path, &source)
    // 4. 报 "PIDSAVEAS  saved N streams to <out>" （N 来自 cached package）
    // 5. 如果 --verify：调 verify_pid_file(&out_path)，报 PASS / FAIL 详情
    // 6. tab.dirty = false（保存成功意味着脏状态清空；现在 UI 可继续编辑）
}
```

### 输出格式

成功（无 verify）：
```
PIDSAVEAS  saved 4 streams to D:\out\drawing-rev2.pid
```

成功 + verify PASS：
```
PIDSAVEAS  saved 4 streams to D:\out\drawing-rev2.pid
PIDVERIFY  PASS  4 streams matched in D:\out\drawing-rev2.pid
```

成功 + verify FAIL：
```
PIDSAVEAS  saved 4 streams to D:\out\drawing-rev2.pid
PIDVERIFY  FAIL  1 mismatch(es) in D:\out\drawing-rev2.pid (matched 3 of 4)
    /TaggedTxtData/Drawing  source=105 B  roundtrip=42 B  first diff @ 8
```

失败：
```
PIDSAVEAS: path must end in .pid; got 'drawing.dwg'
PIDSAVEAS: cannot create destination directory: ...
PIDSAVEAS: ...
```

### Stream 数量

保存后的 cached package 通过 `pid_package_store::get_package(source)` 读 stream 数。save_pid_native 本身不返回这个信息，可以读一次 `arc.streams.len()`。

## 实施步骤

### Step 1 · 命令注册

`src/app/commands.rs`，紧邻 `PIDVERIFY` 分支后：
```rust
cmd if cmd == "PIDSAVEAS" || cmd.starts_with("PIDSAVEAS ") => {
    let rest = cmd.strip_prefix("PIDSAVEAS").unwrap_or("").trim();
    let (path_str, verify_flag) = if let Some(stripped) = rest.strip_suffix("--verify") {
        (stripped.trim().to_string(), true)
    } else {
        (rest.to_string(), false)
    };
    // path_str 空 → error usage
    // path_str 后缀校验 .pid
    // active tab current_path 读取 + source .pid 校验
    // pid_import::save_pid_native(...) + stream_count 查询
    // --verify → verify_pid_file(...)
    // push_output / push_error
    // tab.dirty = false on success
}
```

### Step 2 · 报告辅助

不做 helper 函数，直接在 match arm 里 inline 报告——与其它 PID 命令风格一致。

### Step 3 · 测试

H7CAD `#[cfg(test)] mod tests` 新增 3 个：
- `pidsaveas_command_path_not_pid_is_test_of_helpers`：验证 `save_pid_native` 对非 cached 路径仍按现有语义（这个其实现有 `save_pid_without_prior_open_returns_explicit_error` 已覆盖，直接跳过）
- 但 dispatch_command 本身没测试覆盖（与其它命令分支相同），所以**本批不新增命令级测试**。命令逻辑 = `save_pid_native + verify_pid_file` 两个 helper 调用；二者都有独立测试覆盖
- **替代**：加一个 helper 级组合测试验证"save+verify 应该总过"这个前提：
  - `save_pid_native_then_verify_pid_file_always_passes`：build fixture → load → save_pid_native(dst) → verify_pid_file(dst) → 必 PASS
  - 这是本批唯一新测试

### Step 4 · 落地

- `cargo build` 全绿
- `cargo test io::pid_import` 全绿（含新测试）
- `.memory/2026-04-19.md` 追加段落

## 不做

1. `PIDSAVEAS` 覆盖写保护（`--force` flag）：第一版 `PidWriter::write_to` 已经能覆盖已有文件；要避免 overwrite 的警告留给下一轮
2. `PIDSAVEAS --help` 子命令：与其它命令一致依赖全局 `PIDHELP` 命令（未来工作）
3. 相对路径解析：`PidWriter::write_to` 接受任意 Path，相对路径会基于 cwd 解析；第一版不做额外规范化
4. Undo 栈集成：与 PIDSETPROP 等保持一致——metadata-only 编辑不入历史栈

## 工作量预估

- Step 1：15 min
- Step 2：同 Step 1
- Step 3：10 min
- Step 4：5 min

合计 ~30 min。比其它迭代小，因为底层 helper 齐了。
