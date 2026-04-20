# 本地 pid-parse sync 到 origin/main + 验证 H7CAD 兼容

> 日期：2026-04-19  
> 前提：sync-report 第二轮确认远程 main 为 superset
>
> **目标**：把本地 pid-parse 工作树从 `codex/pid-workbench` (d6ddeb2) 升级到
> `origin/main` (51e7a28, v0.3.12)，并验证 H7CAD 的 14 个 PID 命令族在新
> pid-parse 上继续编译、测试通过。

---

## 风险点

远程 main 相对 codex/pid-workbench 多了：
- `src/writer/xml_edit.rs`（新模块，与 `metadata_helpers.rs` 可能功能重叠）
- 各 `src/writer/*.rs` 内部结构（函数签名可能升级）
- `src/parsers/doc_version2.rs`（新模块，纯增量）
- DocVersion2 结构体 + `PidDocument.doc_version2_decoded` 字段（纯增量）
- CI、docs、fmt 整体清理

**主要风险**：H7CAD `src/io/pid_import.rs` 调用 pid-parse 写入/读取 API，
如果远程升级了 writer 端函数签名，H7CAD 可能不再编译。

## 实施步骤

### Step 1 · 本地 pid-parse 切到 origin/main

```bash
cd pid-parse
git fetch origin
git switch main  # 如果 main 不存在则 git switch -c main origin/main
git reset --hard origin/main
```

**不** rebase codex/pid-workbench 上去（那会把 `d6ddeb2` 再推一次；远程 main
已经合并过，rebase 没意义）。codex/pid-workbench 分支保留在 remote 作历史。

### Step 2 · 验证 pid-parse 自身

```bash
cargo test --lib          # 全绿？
cargo test --tests        # 含 parse_real_files 条件降级
cargo build --bin pid_inspect --bin pid_writer_validate
```

### Step 3 · 验证 H7CAD 编译

```bash
cd ../H7CAD
cargo build --message-format=short
```

**预期失败场景**：
- 如果 H7CAD 用 `pid_parse::writer::set_drawing_attribute` 但远程 main 把它
  改名成 `pid_parse::writer::xml_edit::replace_simple_tag_text`（从 blob
  diff 看 xml_edit.rs 的函数名是这个）
- 如果 writer 某个签名加参数
- 如果某结构体字段增删

失败时**具体错误列表**驱动下面 Step 4 的修复。

### Step 4 · 修复 H7CAD 调用面

按 Step 3 的报错逐一处理：
- 函数重命名 → 替换调用点
- 签名参数增加 → 补默认参数
- 结构体字段变化 → 调整投影代码

每处改动保持 semantic 不变，只适配 API 表面。

### Step 5 · 验证 H7CAD 测试

```bash
cargo test io::pid_import  # 之前 52/52 应继续 52/52
```

### Step 6 · 决定是否提 PR 更新 H7CAD codex 分支

- 如果 Step 4 有 H7CAD 改动 → 新 commit 到 codex/pid-workbench，push
- 如果 Step 4 无改动 → 不提交

### Step 7 · 同步面板

---

## 不做

1. **不** rebase codex/pid-workbench 到 origin/main（无意义）
2. **不** 删 codex/pid-workbench 分支（保留作历史）
3. **不** 在 pid-parse 开新 PR（远程已 superset）

## 工作量预估

- Step 1 + 2：5 min
- Step 3 + 4：30 min（含实际修复）
- Step 5 + 6：10 min
- Step 7：5 min

合计 ~50 min。
