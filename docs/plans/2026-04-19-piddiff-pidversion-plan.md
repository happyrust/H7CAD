# H7CAD PIDDIFF + PIDVERSION 命令落地计划

> 日期：2026-04-19  
> 依赖：pid-parse origin/main @ 51e7a28（v0.3.12），含：
> - `pid_parse::package::{PackageDiff, diff_packages}`
> - `pid_parse::inspect::diff::render(&PackageDiff) -> String`
> - `pid_parse::model::{DocVersion2, DocVersion2Record}`
> - `pid_parse::parsers::doc_version2::op_type_label(u8) -> String`
> - `PidDocument.doc_version2_decoded: Option<DocVersion2>` 字段
>
> **目标**：消费远程 main 带来的两个新 API 给 H7CAD 加两条命令：
> - `PIDDIFF <a.pid> <b.pid>` — 对比两个 .pid 的 package 差异
> - `PIDVERSION` — 显示当前 cached PID 的 DocVersion2 版本日志

命令族扩到 **16 个**（含 PIDHELP）。

---

## 用户故事

```
# 对比修改前后两个文件
PIDDIFF drawing.pid drawing-rev2.pid
    PIDDIFF  2 diff(s) between drawing.pid and drawing-rev2.pid
        root CLSID: match
        summary: 1 only-in-a / 0 only-in-b / 1 modified
        (followed by indented details from render)

# 看当前 PID 的保存历史
PIDVERSION
    PIDVERSION  4 version records in drawing.pid
        [1] SaveAs v144 (12/29/25 09:21)
        [2] Save   v77  (03/16/26 14:08)
        [3] Save   v144 (03/16/26 14:12)
        [4] Save   v77  (04/19/26 09:45)
```

## 设计

### H7CAD 端 API 增量

`src/io/pid_import.rs`：
```rust
/// Owned projection of one DocVersion2 record for command-line display.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PidVersionRecord {
    pub op_type: u8,
    pub op_label: String,       // "SaveAs" / "Save" / "0xNN"
    pub version: u32,
}

/// Outcome of [`list_pid_versions`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PidVersionLog {
    pub magic_u32_le: u32,
    pub reserved_all_zero: bool,
    pub records: Vec<PidVersionRecord>,
}

/// Read the DocVersion2 structured log from the cached PidPackage.
/// Returns `Ok(None)` when the file has a `/DocVersion2` stream in raw
/// form but no structured decode (older format / layout unknown).
/// Returns `Err` when the cache is missing or the doc has no DocVersion2.
pub fn list_pid_versions(source: &Path) -> Result<Option<PidVersionLog>, String>;

/// Parse both paths via `parse_package`, run `diff_packages`, and return
/// the rendered human-readable diff text + a `has_differences` flag.
/// Does not consult or mutate the package cache.
pub fn diff_pid_files(a: &Path, b: &Path) -> Result<(bool, String), String>;
```

### 命令注册

紧邻 `PIDVERIFY`：
```rust
// PIDDIFF <a> <b>
cmd if cmd == "PIDDIFF" || cmd.starts_with("PIDDIFF ") => {
    let parts = cmd.strip_prefix("PIDDIFF").unwrap_or("").trim().split_whitespace().collect::<Vec<_>>();
    if parts.len() != 2 { usage error; return; }
    // both must be .pid
    match diff_pid_files(&PathBuf::from(parts[0]), &PathBuf::from(parts[1])) {
        Ok((has_diff, text)) => {
            push_output("PIDDIFF  {result} between <a> and <b>");
            for line in text.lines() { push_info line; }
        }
        Err(e) => push_error;
    }
}

// PIDVERSION
cmd if cmd == "PIDVERSION" => {
    // active tab's cached PID
    match list_pid_versions(&source) {
        Ok(Some(log)) => {
            push_output("PIDVERSION  {N} version records in {source}");
            for (i, r) in log.records.iter().enumerate() {
                push_info("    [{idx}] {op_label:6} v{version}");
            }
        }
        Ok(None) => push_output("PIDVERSION: no structured DocVersion2 decoded (raw stream present)");
        Err(e) => push_error;
    }
}
```

### PIDHELP 扩充

新增两行到 Integrity 组：
```
PIDDIFF <a.pid> <b.pid>             byte-level diff between two PID packages
PIDVERSION                          DocVersion2 structured save history
```

命令计数 14 → 16。

### 测试

#### H7CAD 单测（5 个）
1. `diff_pid_files_reports_no_difference_for_identical_fixtures`：build 同一 fixture 两份 → 调用 → `has_diff=false` + "(no differences)"
2. `diff_pid_files_reports_modified_stream`：build fixture a + b，b 修改 Drawing 字节 → `has_diff=true` + 文本含 "Modified"
3. `diff_pid_files_errors_on_non_pid`：传 .dwg 路径 → 明确错误
4. `list_pid_versions_returns_none_without_decoded_field`：synthetic 包 `doc_version2_decoded=None` → Ok(None)
5. `list_pid_versions_returns_records_when_decoded`：synthetic 包 `doc_version2_decoded=Some(DocVersion2{...})` → Ok(Some(log)) + records 映射正确

### 落地

- cargo build + cargo test io::pid_import 全绿
- .memory/2026-04-19.md 追加段落
- H7CAD 提交到 codex/pid-workbench 分支，push

## 不做

1. **PIDDIFF --json 模式** — 第一版只 human
2. **PIDDIFF 显示 timestamp 列** — DocVersion2 本身不带 timestamp（timestamp 在 DocVersion3）
3. **PIDVERSION 对比 DocVersion3** — 两个版本日志的关系留给 pid_inspect 专用工具
4. **PIDDIFF 支持 cached 对比** — 第一版只读两个磁盘文件

## 工作量预估

- helpers：15 min
- 命令分支 ×2：15 min
- 测试 5 个：20 min
- PIDHELP + 落地：10 min

合计 ~60 min。
