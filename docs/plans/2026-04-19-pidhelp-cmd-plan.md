# H7CAD PIDHELP 命令落地计划

> 起稿：2026-04-19  
> 依赖：本日完成的完整 PID 命令族（8 个）
>
> **目标**：加一条 `PIDHELP` 命令，在命令行内列出所有 `PID*` 命令及其用法和一行描述。现在 PID 命令族已扩展到 8 个（PIDSETDRAWNO/PIDSETPROP/PIDSETGENERAL/PIDGETPROP/PIDGETGENERAL/PIDLISTPROPS/PIDVERIFY/PIDSAVEAS），用户无可发现性入口。

---

## 用户故事

```
PIDHELP
    PIDHELP  PID metadata commands (8 available)
        Write:
            PIDSETDRAWNO <new>              — shortcut for SP_DRAWINGNUMBER
            PIDSETPROP   <attr> <value...>  — any Drawing-stream SP_* attribute
            PIDSETGENERAL <element> <value...>  — General stream element text
        Read:
            PIDGETPROP    <attr>            — read Drawing attribute
            PIDGETGENERAL <element>         — read General element text
            PIDLISTPROPS                    — dump every Drawing attr + General element
        Integrity:
            PIDVERIFY [<path>]              — round-trip byte-level fidelity check
            PIDSAVEAS <path> [--verify]     — save current PID + optional inline verify
    Notes:
        - All commands require an opened .pid file in the active tab.
        - Edits are metadata-only; native scene changes are not flushed.
```

## 设计

### 命令注册

`src/app/commands.rs`，紧邻 `PIDSAVEAS` 之后：
```rust
cmd if cmd == "PIDHELP" => {
    print_pidhelp(&mut self.command_line);
}
```

### Helper 函数

模块内（commands.rs 顶部）：
```rust
fn print_pidhelp(line: &mut crate::ui::CommandLine) {
    line.push_output("PIDHELP  PID metadata commands (8 available)");
    line.push_info("    Write:");
    line.push_info("        PIDSETDRAWNO <new>              — shortcut for SP_DRAWINGNUMBER");
    line.push_info("        PIDSETPROP   <attr> <value...>  — any Drawing-stream SP_* attribute");
    line.push_info("        PIDSETGENERAL <element> <value...>  — General stream element text");
    line.push_info("    Read:");
    line.push_info("        PIDGETPROP    <attr>            — read Drawing attribute");
    line.push_info("        PIDGETGENERAL <element>         — read General element text");
    line.push_info("        PIDLISTPROPS                    — dump every Drawing attr + General element");
    line.push_info("    Integrity:");
    line.push_info("        PIDVERIFY [<path>]              — round-trip byte-level fidelity check");
    line.push_info("        PIDSAVEAS <path> [--verify]     — save current PID + optional inline verify");
    line.push_info("    Notes:");
    line.push_info("        - All commands require an opened .pid file in the active tab.");
    line.push_info("        - Edits are metadata-only; native scene changes are not flushed.");
}
```

`CommandLine` 结构已经 `pub`（由 mod.rs 能看到）。如果 `push_info` 签名不匹配，在这里直接 inline `push_info` 字符串即可——不抽 helper，直接在 arm 内 N 行 `push_info`。

为稳妥起见，**不抽 helper**，直接在 match arm 内 inline 所有 `push_info`——与现有命令（ALIASEDIT LIST / MLEADERSTYLE help）同风格。

## 实施步骤

### Step 1 · 命令分支

紧接 `PIDSAVEAS` 之后，加 `cmd if cmd == "PIDHELP"` 分支，inline 所有 `push_output` / `push_info`。

### Step 2 · 不写测试

与其它 `dispatch_command` 分支一致——help 命令纯展示逻辑，没有可 verify 的副作用。跳过。

### Step 3 · 落地

- `cargo build` 全绿
- `cargo test io::pid_import` 仍 29/29
- `.memory/2026-04-19.md` 追加段落

## 不做

1. **动态发现命令**（用 reflection 自动列出 PID*）：Rust 无运行时反射；硬编码 help 列表即可，命令族规模小、改动同步不难
2. **PIDHELP <cmd>**（显示单命令详细说明）：第一版就一张总表；单命令详细说明与代码注释重复，延迟到用户真的抱怨再做
3. **`--cheatsheet` flag**（输出 Markdown 格式便于复制到文档）：延迟

## 工作量预估

- Step 1：10 min
- Step 3：5 min

合计 ~15 min。
