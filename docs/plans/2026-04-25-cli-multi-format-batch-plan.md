# Multi-format batch CLI 测试（R44-E）

> **起稿**：2026-04-25（第四十四轮 · E 阶段）
> **前置**：R36 起 CLI 多输入路径已经 per-input dispatch
> （`load_file_with_native_blocking(input)` 在每个 input 独立调用，
> 自动按扩展名分发到 DXF / DWG / PID）；R44-A surface DWG notice，
> R44-B 接入 PID notice，R44-C 加 PID single-input 集成测试 5 条。
>
> **当前缺口**：21 条 cli_batch_export 集成测试里：
> - 16 条 DXF single 或 DXF×N 多输入
> - 5 条 PID single
> - **0 条 PID + DXF 混合多输入**
>
> dispatcher 任何一处 bug（per-input 边界泄漏、notice 错位、output
> 路径误算）都可能让混合格式产出错误的输出文件或 stderr 行——但
> 没有测试守门，会等真实用户上报才发现。
>
> **目标**：本轮新增 5 条 multi-format 端到端测试，从 4 个角度封死
> 混合输入的退化路径：
> 1. 混合 batch PDF 输出
> 2. 混合 batch SVG 输出
> 3. 混合 `--list-layouts` text 模式
> 4. 混合 `--list-layouts --json` 模式
> 5. notice 隔离：PID notice 不污染 DXF 输出，per-input prefix 各自正确
>
> **零代码改动**——纯测试增量。

---

## 1. 现状

### 1.1 dispatcher 已经 per-input

`src/cli.rs::run_batch_export::Export` 分支按 `inputs.iter()` 循环，
每个 input 独立调 `export_one(...)`。`export_one` 内部第一步就是
`crate::io::load_file_with_native_blocking(input)`，按扩展名分发到
DXF / DWG / PID。`run_list_layouts_text` / `run_list_layouts_json`
同形结构。

所以 dispatcher 逻辑上**一定能处理**混合格式 batch——但**没有测试
守门**。

### 1.2 R44-A / R44-B 的 notice 通道

per-input load 后立刻调 `format_notices_for_cli(input, &notices)`：

```rust
let notice_lines = format_notices_for_cli(input, &notices);
if !notice_lines.is_empty() {
    eprint!("{notice_lines}");
}
```

每行 `h7cad: <input>: notice [Tag] message\n`——**input 路径作为前缀**
区分来源。所以混合输入时，PID 的 `[NotImplemented]` 行带 `.pid` 前缀，
DXF 的（如果有）带 `.dxf` 前缀，互不污染。

但**没有测试守门 prefix 隔离**——一个未来的 bug 把
`for input in inputs { ... eprint!(...) }` 改成共享 buffer 就会
silently 错位。

### 1.3 cli_batch_export 已有的多输入测试

`cli_batch_two_dxfs_to_dir` 是唯一现存的 multi-input 测试：DXF×2 →
目录输出。验证基本 multi-input 工作，但**只覆盖单一格式**。

---

## 2. 范围

| 任务 | 优先级 | 预估 |
|------|-------|------|
| T1 `cli_batch_mixed_pid_and_dxf_to_dir_pdf` — 混合 batch PID+DXF → 目录 → 各产生独立 PDF | P0 | 0.4 h |
| T2 `cli_batch_mixed_pid_and_dxf_to_dir_svg` — 同 T1 但 SVG | P0 | 0.2 h |
| T3 `cli_lists_layouts_for_mixed_pid_dxf_text_mode` — `--list-layouts` 多输入 PID+DXF 文本：每个输入按 `=== <path> ===` 分块、PID notice 走 stderr 不污染 stdout | P0 | 0.3 h |
| T4 `cli_lists_layouts_for_mixed_pid_dxf_json_mode` — `--list-layouts --json` 多输入 PID+DXF：JSON `inputs` 数组 2 条，layouts 各自正确 | P0 | 0.3 h |
| T5 `cli_mixed_batch_isolates_pid_notice_from_dxf_input` — 关键 notice 隔离：DXF stderr 无 `[NotImplemented]` 行（PID 才有），且每行 prefix 含对应 input 文件名 | P0 | 0.4 h |
| T6 CHANGELOG R44-E 章节 + `docs/cli.md` Future flags 更新 | P0 | 0.2 h |
| T7 跑 `cargo test --locked --workspace --all-targets` + `RUSTFLAGS=-Dwarnings cargo check --locked --workspace --all-targets` + ReadLints | P0 | 0.2 h |

**不纳入**：
- 真实 `.dwg` fixture 端到端集成测试（R44-D，独立成轮）
- multi-input 失败处理 + partial output（`cli_mixed_failure_keeps_processing_and_reports_nonzero` 已覆盖；R44-E 只关心成功路径）
- 3-format 混合（PID + DXF + DWG）—— DWG 同 fixture 投入问题，本轮
  纯 PID + DXF 已经覆盖 dispatcher 的 per-input 关键性质
- `--options` JSON 与混合输入的交互（`--options` 是 shared-across-inputs
  的，dispatcher 行为与单输入一致；R44-E 不引入新链路）

---

## 3. 设计

### 3.1 cli_batch_mixed_pid_and_dxf_to_dir_pdf

```rust
#[test]
fn cli_batch_mixed_pid_and_dxf_to_dir_pdf() {
    use std::fs;

    let pid_id = std::process::id();
    let tmp = std::env::temp_dir();
    let dxf_input = tmp.join(format!("h7cad_r44e_{pid_id}_mixed_in.dxf"));
    let pid_input = tmp.join(format!("h7cad_r44e_{pid_id}_mixed_in.pid"));
    let out_dir = tmp.join(format!("h7cad_r44e_{pid_id}_mixed_pdf_out/"));

    write_minimal_dxf_to(&dxf_input);
    build_minimal_pid_fixture(&pid_input);
    let _ = fs::remove_dir_all(&out_dir);
    fs::create_dir_all(&out_dir).expect("create out dir");

    let output = Command::new(binary_path())
        .arg(&dxf_input)
        .arg(&pid_input)
        .arg("--export-pdf")
        .arg(&out_dir)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .expect("spawn h7cad mixed batch PDF");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "mixed batch PDF exit: {:?}\nstderr: {stderr}",
        output.status.code()
    );

    let dxf_pdf = out_dir.join(
        dxf_input.file_stem().unwrap().to_string_lossy().to_string() + ".pdf"
    );
    let pid_pdf = out_dir.join(
        pid_input.file_stem().unwrap().to_string_lossy().to_string() + ".pdf"
    );
    let dxf_bytes = fs::read(&dxf_pdf).expect("DXF→PDF should exist");
    let pid_bytes = fs::read(&pid_pdf).expect("PID→PDF should exist");
    assert!(dxf_bytes.starts_with(b"%PDF-"));
    assert!(pid_bytes.starts_with(b"%PDF-"));

    let _ = fs::remove_file(&dxf_input);
    let _ = fs::remove_file(&pid_input);
    let _ = fs::remove_dir_all(&out_dir);
}
```

### 3.2 cli_batch_mixed_pid_and_dxf_to_dir_svg

完全同形，断言 `<svg` 字样而非 `%PDF-` magic。

### 3.3 cli_lists_layouts_for_mixed_pid_dxf_text_mode

`--list-layouts` 在多输入时按 `=== <path> ===` 分块（cli.rs:341-345）：

```
=== /tmp/.../mixed.dxf ===
Model

=== /tmp/.../mixed.pid ===
Model
```

stderr 应有 PID 路径的 `[NotImplemented]` 行；DXF 路径的 stderr
应无任何 notice（DXF 路径返回 `Vec::new()` notices）。

```rust
#[test]
fn cli_lists_layouts_for_mixed_pid_dxf_text_mode() {
    // ... setup dxf_input + pid_input ...
    let output = Command::new(binary_path())
        .arg(&dxf_input).arg(&pid_input).arg("--list-layouts")
        .stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped())
        .output().expect("spawn");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    let dxf_name = dxf_input.file_name().unwrap().to_string_lossy();
    let pid_name = pid_input.file_name().unwrap().to_string_lossy();

    assert!(stdout.contains(&format!("=== {} ===", dxf_input.display())),
            "stdout must contain DXF block header; got: {stdout}");
    assert!(stdout.contains(&format!("=== {} ===", pid_input.display())),
            "stdout must contain PID block header; got: {stdout}");
    assert!(stdout.lines().filter(|l| l.trim() == "Model").count() >= 2,
            "Model must appear at least twice (one per input); got: {stdout}");

    // PID notice on stderr only, mentioning PID input only
    let notice_lines: Vec<&str> = stderr.lines()
        .filter(|l| l.contains("notice ["))
        .collect();
    assert!(notice_lines.iter().any(|l| l.contains(pid_name.as_ref())),
            "PID notice must be present in stderr referencing PID file; got: {stderr}");
    assert!(notice_lines.iter().all(|l| !l.contains(dxf_name.as_ref())),
            "no notice line should reference DXF input; got: {stderr}");
    // ... cleanup ...
}
```

### 3.4 cli_lists_layouts_for_mixed_pid_dxf_json_mode

JSON 输出 `inputs` 数组 2 条，每条 `path` 字段对应输入路径，`layouts`
都含 "Model"。stderr 同样隔离 PID notice。

```rust
let parsed: serde_json::Value = serde_json::from_str(&stdout)?;
let inputs = parsed["inputs"].as_array().expect("array");
assert_eq!(inputs.len(), 2);
// inputs[0] = DXF, inputs[1] = PID（command-line 顺序保留）
assert!(inputs[0]["path"].as_str().unwrap().ends_with(".dxf"));
assert!(inputs[1]["path"].as_str().unwrap().ends_with(".pid"));
// layouts 都含 Model
for entry in inputs {
    let layouts = entry["layouts"].as_array().expect("layouts");
    assert!(layouts.iter().any(|v| v == "Model"));
}
```

### 3.5 cli_mixed_batch_isolates_pid_notice_from_dxf_input

R44-E 的关键守门——验证 per-input notice prefix 隔离：

```rust
#[test]
fn cli_mixed_batch_isolates_pid_notice_from_dxf_input() {
    // ... setup dxf_input + pid_input + out_dir ...

    let output = Command::new(binary_path())
        .arg(&dxf_input).arg(&pid_input)
        .arg("--export-pdf").arg(&out_dir)
        .stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped())
        .output().expect("spawn");

    assert!(output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);

    let dxf_name = dxf_input.file_name().unwrap().to_string_lossy();
    let pid_name = pid_input.file_name().unwrap().to_string_lossy();

    // Every notice line must carry an input-path prefix.
    let notice_lines: Vec<&str> = stderr.lines()
        .filter(|l| l.contains("notice ["))
        .collect();

    // R44-B: synthesised PID without ObjectGraph emits exactly 1
    // [NotImplemented] notice; DXF path emits 0 notices.
    assert!(!notice_lines.is_empty(),
            "expected at least one PID notice; got stderr: {stderr}");
    for line in &notice_lines {
        assert!(line.contains(pid_name.as_ref()),
                "every notice line must prefix PID file (DXF must not appear); \
                 got: {line}");
        assert!(!line.contains(dxf_name.as_ref()),
                "DXF input must never appear in notice lines; got: {line}");
    }
    // ... cleanup ...
}
```

---

## 4. 测试

### 4.1 集成测试（`tests/cli_batch_export.rs`）

新增 5 条：
- `cli_batch_mixed_pid_and_dxf_to_dir_pdf`
- `cli_batch_mixed_pid_and_dxf_to_dir_svg`
- `cli_lists_layouts_for_mixed_pid_dxf_text_mode`
- `cli_lists_layouts_for_mixed_pid_dxf_json_mode`
- `cli_mixed_batch_isolates_pid_notice_from_dxf_input`

### 4.2 单元测试

不新增（dispatcher 在 unit 层已有 `parse_multi_input_*` 测试覆盖；
R44-E 是端到端层）。

---

## 5. 验收

```bash
cargo check -p H7CAD                                # 零新 warning（无代码改动）
cargo test --test cli_batch_export                  # 21 → 26 全绿（+5 R44-E）
cargo test --bin H7CAD                              # 514 / 514 不变
cargo test --locked --workspace --all-targets       # 全绿
RUSTFLAGS=-Dwarnings cargo check --locked --workspace --all-targets  # 零新 warning
```

ReadLints 改动文件零 lint：
- `tests/cli_batch_export.rs`
- `CHANGELOG.md`
- `docs/cli.md`
- `docs/plans/2026-04-25-cli-multi-format-batch-plan.md`

手动：跑 `cargo test --test cli_batch_export -- --nocapture cli_mixed_batch_isolates` →
应在 cargo 输出里看到捕获的 `h7cad: /tmp/.../mixed.pid: notice
[NotImplemented] PID object graph stream missing; layout falls back
to grid` 行。

---

## 6. 状态

- [x] 计划定稿（2026-04-25）
- [x] T1 `cli_batch_mixed_pid_and_dxf_to_dir_pdf`
- [x] T2 `cli_batch_mixed_pid_and_dxf_to_dir_svg`
- [x] T3 `cli_lists_layouts_for_mixed_pid_dxf_text_mode`
- [x] T4 `cli_lists_layouts_for_mixed_pid_dxf_json_mode`
- [x] T5 `cli_mixed_batch_isolates_pid_notice_from_dxf_input`
- [x] T6 CHANGELOG R44-E 章节 + docs/cli.md Future flags 更新
- [x] T7 全量验证

---

## 7. 下轮方向

- **R44-D**：真实 `.dwg` fixture 端到端集成测试（仓库无 fixture，需
  从外部源获取 / 合成 AC1015 二进制；与 R44-C / R44-E 同形但 fixture
  投入显著更大）
- **R43-A2**：SVG `<pattern>` 元素优化（PatFamily.dx==0 子集 pattern
  化；当前评估 PDF 对称性 / patternTransform 跨浏览器兼容性 / non-convex
  boundary fill 三个开放问题需要先做技术调研，独立成轮 R43-A2.0）
- **R43-B**：Rational NURBS（degree-2/3 + 非 unit weight）
- **R43-C**：OLE2_FRAME entity 渲染
- **R45**：Paper Space 多视口合成（`setupActiveLayoutViews` parity）
- **R46**：将 `build_minimal_pid_fixture` 抬到 `h7cad-native-testkit`
  里供 GUI / facade / 多个集成测试共用（如果 R44-D 也需要类似 fixture
  helper）
