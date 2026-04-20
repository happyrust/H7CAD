# `pid_writer_validate` CLI 落地计划

> 起稿：2026-04-19  
> 依赖：`pid-parse` v0.4.0+ writer 层全部能力（passthrough WritePlan + cfb_write）。
>
> **目标**：在 `pid-parse` 仓库内追加一个独立 binary `pid_writer_validate`，让用户能在不打开 H7CAD GUI 的情况下，对**任意真实 .pid 文件**做：
>
> ```
> parse_package(input) → write_to(temp, passthrough) → re-parse_package(temp) → per-stream byte diff
> ```
>
> 这是 v0.4.0 落地报告里"对 SPPID 宿主完整兼容性需后续验证"的官方 CLI 工具，也是后续 PIDSAVEAS verify 模式 / `tests/writer_real_files.rs` 自动化扩展的底层。

---

## 现状盘点

* `pid-parse` 仓库已有 `src/bin/pid_inspect.rs` 一个 binary（pattern：`std::env::args` + 多 flag）
* `tests/writer_real_files.rs` 已经做"条件性真实文件 round-trip"，但测试形式只 panic-on-fail，无人类可读的差异报告
* `tests/writer_roundtrip.rs` 是内存 fixture round-trip，不接触真实文件
* 缺：能在命令行手动跑、输出可读 diff 的工具

## 用户故事

> "我想验证 H7CAD 保存出来的 PID 文件确实和原文件按 stream 字节级一致。"
> 
> ```
> $ pid_writer_validate test-file/DWG-0201GP06-01.pid
> Reading source ... 69 streams, 4.2 MiB
> Re-emitting via PidWriter ... wrote 4.2 MiB to /tmp/pid-validate-12345.pid
> Re-parsing roundtrip ... 69 streams
> Stream key set: matched (69/69)
> Per-stream byte equality:
>     OK    /TaggedTxtData/Drawing      94 B
>     OK    /TaggedTxtData/General      133 B
>     OK    /PlainSheet/Sheet1          16 B
>     ...
>     69/69 streams identical
> Result: PASS
> ```
>
> 任何 mismatch 都按 path 列出 diff（首个差异 offset + hex 前后各 8 字节）。

## CLI 设计

```
pid_writer_validate <input.pid> [options]

Positional:
    <input.pid>          Source file to round-trip.

Options:
    --out <path>         Where to write the round-trip output. Default:
                         OS temp dir + unique name; auto-deleted unless --keep.
    --keep               Keep --out file on disk after the run.
    --json               Machine-readable JSON report on stdout instead of human format.
    --quiet              Suppress per-stream lines; only print summary + mismatches.
    --max-diff-bytes <N> When showing a mismatched stream, dump up to N bytes around
                         the first difference. Default: 16.
    -h | --help          Print usage.

Exit codes:
    0  All streams matched.
    1  Mismatch (key set or per-stream bytes diverged).
    2  Parse / IO failure.
```

## 报告设计

### Human format（默认）

```
Reading source D:/work/.../X.pid ... 69 streams, 4.2 MiB
Re-emitting via PidWriter (passthrough) ... wrote 4.2 MiB to <out>
Re-parsing roundtrip ... 69 streams

== Stream key set ==
Matched: 69
Only in source: 0
Only in roundtrip: 0

== Per-stream byte equality ==
OK    /TaggedTxtData/Drawing       94 B
OK    /TaggedTxtData/General      133 B
...
Total: 69 matched, 0 mismatched.

Result: PASS (exit 0)
```

如果有 mismatch：

```
DIFF  /Some/Stream  source=128 B  roundtrip=129 B  first diff @offset=42
    source [34..50]: 89 04 22 00 11 22 33 44 55 66 77 88 99 AA BB CC
    output [34..50]: 89 04 22 00 11 22 33 44 99 99 99 99 99 99 99 99

Total: 67 matched, 2 mismatched.
Result: FAIL (exit 1)
```

### JSON format（`--json`）

```json
{
  "source_path": "D:/.../X.pid",
  "output_path": "C:/Temp/pid-validate-12345.pid",
  "source_stream_count": 69,
  "roundtrip_stream_count": 69,
  "matched": 69,
  "mismatched": 0,
  "only_in_source": [],
  "only_in_roundtrip": [],
  "mismatches": [
    { "path": "...", "source_len": 128, "roundtrip_len": 129, "first_diff_offset": 42 }
  ],
  "ok": true
}
```

## 实施步骤

### Step 1 · binary 骨架

`src/bin/pid_writer_validate.rs`：
- `main`：解析 args（`pid_inspect` 同模式：手写 `args.iter().position(...)`，无 clap 依赖）
- 构造 `OutputSpec { path: PathBuf, keep: bool }`：未指定 `--out` → `temp_dir().join("pid-writer-validate-{pid}-{nanos}.pid")`
- 调 `run_validate(...)` → `Result<ValidateReport, ValidateError>`
- 根据 `--json` 或 human format 打印
- 退出码按 `report.ok` / `Err`

### Step 2 · 核心 round-trip + diff

模块内部函数：

```rust
fn run_validate(input: &Path, out_spec: &OutputSpec, max_diff_bytes: usize)
    -> Result<ValidateReport, ValidateError>;

struct ValidateReport {
    source_path: PathBuf,
    output_path: PathBuf,
    source_streams: usize,
    roundtrip_streams: usize,
    matched: usize,
    mismatched: usize,
    only_in_source: Vec<String>,
    only_in_roundtrip: Vec<String>,
    mismatches: Vec<StreamMismatch>,
}

struct StreamMismatch {
    path: String,
    source_len: usize,
    roundtrip_len: usize,
    first_diff_offset: usize,
    source_window: Vec<u8>,
    roundtrip_window: Vec<u8>,
}

impl ValidateReport { fn ok(&self) -> bool }
```

实现：
1. `PidParser::new().parse_package(input)?` → `original`
2. `PidWriter::write_to(&original, &WritePlan::default(), &out_spec.path)?`
3. `PidParser::new().parse_package(&out_spec.path)?` → `roundtrip`
4. `BTreeSet` 对比 keys
5. 对共同 keys 逐个 byte-比较；mismatch 时计算 `first_diff_offset`、抽取 `[max(0,off-N/2) .. off+N/2]` 窗口

### Step 3 · 可读输出

- `print_human(&report)`：按上面格式 `println!`，`OK` 与 `DIFF` 各加色（**不**真用 ANSI escape，普通文本以保 PowerShell 兼容）
- `print_json(&report)`：`serde_json::to_string_pretty(&report)`，给 `ValidateReport` 与子结构 `derive(Serialize)`

### Step 4 · 测试

`tests/writer_validate_cli.rs`（新文件）：
- `validate_passes_on_synthetic_fixture`：创建 cfb fixture（与 writer_roundtrip.rs 同模式）→ 用 `std::process::Command` 调 `cargo_bin!("pid_writer_validate")` → 校验 exit code 0 + stdout 含 "Result: PASS"
- `validate_fails_when_output_was_tampered`：先正常 round-trip → 用 `OpenOptions::write` 改输出文件中间 1 个字节 → 现在 `pid_writer_validate input --out tampered.pid` 因为 source 和 tampered 不同会失败？不对——validate 总是先 write 一遍再比，无法测 "tamper after write"。**改方向**：直接对 `run_validate` 函数（pub helper）做单元测试，构造一个 pre-corrupted 输出场景。或者跳过 fail 测试，只测 happy path（CLI 自动化覆盖到主路径就够）。
- `validate_emits_json_when_flag_set`：调 `--json`，校验 stdout 解析为 JSON 且含 `"ok": true`

注：用 `assert_cmd` 类 crate 通常更好，但 pid-parse 当前没引入；用 `Command::cargo_bin` 通过 `escargot` 也要新依赖。**简化**：在测试里用 `env!("CARGO_BIN_EXE_pid_writer_validate")` env 变量（cargo 自动设置），无需新依赖。

写 2 个端到端测试 + 把 helper（`run_validate`）从 `main` 暴露在同 module 里（`mod helpers; pub fn run_validate(…)`）以便单元测试。

### Step 5 · 落地

- `cargo build --bin pid_writer_validate` 全绿
- `cargo test --test writer_validate_cli` 全绿
- `cargo test` 全集（除 fixture-dependent）保持原状
- `pid-parse/CHANGELOG.md` 在 0.4.1 段尾追加 "新 binary `pid_writer_validate`" 行
- `pid-parse/README.md` 如有 CLI 说明段补一行（如果没有就不动）
- 不 bump 版本（v0.4.1 polish 范围）

## 公共 API 增量

### pid-parse
- 新增 binary `pid_writer_validate`
- 新增公开模块（在 binary 里）：`ValidateReport` / `StreamMismatch` / `run_validate(input, output, max_diff_bytes)`，主要为测试可达；如果想把 helper 抽到 lib 也可，但本计划不强求

### H7CAD
- 无改动

## 显式不做（留给下一迭代）

1. **支持目录批量验证**（`pid_writer_validate <dir>` 递归）：第一版只单文件
2. **应用 metadata edit 后再 validate**（`--edit SP_X=val` 模拟编辑路径）：第一版只 passthrough
3. **diff hex 着色**：纯文本输出，避免 PowerShell ANSI 兼容问题
4. **输出 unified diff 格式**：第一版用窗口式 diff，简单易读

## 风险

- **R1**：CFB 重建不复刻 CLSID / 时间戳 / 物理 sector 顺序 → 整文件字节 diff 必然差异，但 per-stream content diff 应该完全一致。**降级**：报告明确说"per-stream content equality"，不承诺整文件 diff
- **R2**：超大 .pid 文件（百 MB 级）的 byte-level 比较占内存。**降级**：第一版直接全部读入 `Vec<u8>`；后续加 `--streaming` flag
- **R3**：Windows 上 temp dir cleanup 可能因 antivirus 锁失败 → 用 `let _ = std::fs::remove_file(...)` 容错

## 工作量预估

- Step 1：15 min
- Step 2：30 min（diff 窗口提取 + 数据结构）
- Step 3：20 min
- Step 4：25 min
- Step 5：10 min

合计 ~100 min。
