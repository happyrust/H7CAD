# PID CLI 集成测试（R44-C）

> **起稿**：2026-04-25（第四十四轮 · C 阶段）
> **前置**：R44-B 把 PID CLI 输入正式化（pid_summary_to_notices +
> load_pid_native_with_notices + io-mod dispatch + HELP_TEXT 修正 +
> 6 条 unit test）。R44-B 计划文件 §7 列的 R44-C 是"真实 `.pid`
> 端到端集成测试"——但仓库无 `.pid` fixture。
>
> **本轮做法**：用 `cfb` crate（H7CAD 已有 dev-dep）合成最小 PID
> CFB 在 `tests/cli_batch_export.rs` 里跑端到端 CLI 验证，**无外部
> fixture 依赖**。这等于把 `src/io/pid_import.rs::tests::build_fixture_pid`
> 的合成模板（已经被 70+ unit test 反复验证）抬到集成测试层级。
>
> **目标**：让 CLI 集成测试套首次覆盖 PID 路径（之前 19 条 cli_batch_export
> 测试全是 DXF 路径），杜绝 R44-B 链路的隐形退化（io-mod / cli /
> pid_import 任一动了 PID dispatch 都会被 CI 立刻抓到）。

---

## 1. 现状

### 1.1 `tests/cli_batch_export.rs` 全是 DXF

- 16 条集成测试，输入要么是 DXF 字符串（`write_minimal_dxf_to`），要
  么是不存在路径（错误处理）
- 没有任何一条 PID 路径覆盖

### 1.2 `src/io/pid_import.rs::tests::build_fixture_pid` 已成熟

```rust
fn build_fixture_pid(path: &std::path::Path) {
    let mut cfb = ::cfb::create(path).expect("create fixture cfb");
    cfb.create_storage("/TaggedTxtData").unwrap();
    cfb.create_storage("/PlainSheet").unwrap();
    cfb.create_storage("/UnknownStorage").unwrap();

    let drawing = b"<?xml version=\"1.0\"?><Drawing>...</Drawing>";
    cfb.create_stream("/TaggedTxtData/Drawing").unwrap()
        .write_all(drawing).unwrap();
    // ... General / Sheet / Blob streams ...
    cfb.flush().unwrap();
}
```

由 70+ pid_import unit test 反复验证可被 `PidParser::parse_package`
正常吃下。可以 1:1 复制到集成测试。

### 1.3 R44-B notice 路径需要端到端守门

R44-B 的 6 条 unit test 覆盖 `pid_summary_to_notices` 派发逻辑 +
`format_notices_for_cli` 行格式，但**两者拼接到 main → cli::parse →
load_file_with_native_blocking → format_notices_for_cli → eprintln!**
的整链路没有端到端验证。一个集成测试可以一次性守住整链。

---

## 2. 范围

| 任务 | 优先级 | 预估 |
|------|-------|------|
| T1 `tests/cli_batch_export.rs` 加 `build_minimal_pid_fixture(path: &Path)` helper（4 streams: Drawing/General/Sheet/Blob） | P0 | 0.3 h |
| T2 `cli_writes_pdf_for_minimal_pid` 集成测试：合成 PID → `--export-pdf` → assert exit 0 + PDF magic | P0 | 0.3 h |
| T3 `cli_exports_svg_for_minimal_pid` 集成测试：同 T2 但 SVG | P0 | 0.2 h |
| T4 `cli_lists_layouts_for_minimal_pid_text_mode` 集成测试：`--list-layouts` → assert exit 0 + stdout 含 "Model" | P0 | 0.2 h |
| T5 `cli_lists_layouts_for_minimal_pid_json_mode` 集成测试：`--list-layouts --json` → assert exit 0 + 合法 JSON + `inputs[0].layouts` 含 "Model" | P0 | 0.3 h |
| T6 `cli_pid_input_surfaces_warning_for_unresolved_relationships` 端到端 notice 测试：合成 PID → CLI 跑通 → assert stderr 含 `[Warning]` 或 `[NotImplemented]`（合成 PID 因无 ObjectGraph 必触发 NotImplemented） | P0 | 0.4 h |
| T7 CHANGELOG R44-C 章节 + `docs/cli.md` Future flags 更新（标 R44-C ship） | P0 | 0.2 h |
| T8 跑 `cargo test --locked --workspace --all-targets` + `RUSTFLAGS=-Dwarnings cargo check --locked --workspace --all-targets` + ReadLints | P0 | 0.2 h |

**不纳入**：
- 真实 `.dwg` fixture 集成测试（R44-D 同 fixture 投入问题，独立成轮；
  AC1015 / R12 / 各种 ACAD 版本的合法二进制需要复杂工具链或第三方
  样本）
- PID 写出 CLI 测试（当前 CLI 无 `--save-*` flag）
- Multi-PID + multi-DXF 混合输入测试（语义清晰但本轮覆盖 single-PID
  四个 flag path 已经达到 R44-C 目标——杜绝 PID 路径的"零集成测试"
  状态。混合输入是 R36 / R37 早就有的能力，多输入路径已经被 DXF 测
  试覆盖；混合不会引入新的 dispatch 逻辑）
- `--options` JSON 与 PID 的交互测试（`--options` 是格式无关的，已
  被 DXF 测试覆盖；PID 路径不引入新的 JSON parse 逻辑）

---

## 3. 设计

### 3.1 `build_minimal_pid_fixture` helper

放在 `tests/cli_batch_export.rs` 文件靠尾的 helper 区（与
`write_minimal_dxf_to` 平级）。直接复制 `src/io/pid_import.rs::tests::build_fixture_pid`
的 4-stream 合成逻辑——这个模板已经被 70+ unit test 验证可被
`PidParser::parse_package` 正常吃下。

```rust
fn build_minimal_pid_fixture(path: &std::path::Path) {
    use std::io::Write as _;

    if path.exists() {
        std::fs::remove_file(path).expect("clean fixture path");
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).expect("ensure tmp parent");
    }

    let mut cfb = cfb::create(path).expect("create fixture cfb");
    cfb.create_storage("/TaggedTxtData").unwrap();
    cfb.create_storage("/PlainSheet").unwrap();
    cfb.create_storage("/UnknownStorage").unwrap();

    let drawing = b"<?xml version=\"1.0\"?>\
        <Drawing><Tag SP_DRAWINGNUMBER=\"FX-CLI-001\"/></Drawing>";
    cfb.create_stream("/TaggedTxtData/Drawing")
        .unwrap()
        .write_all(drawing)
        .unwrap();

    let general = b"<?xml version=\"1.0\"?>\
        <General><FilePath>C:/cli-fixture.pid</FilePath></General>";
    cfb.create_stream("/TaggedTxtData/General")
        .unwrap()
        .write_all(general)
        .unwrap();

    let sheet: Vec<u8> = (0u8..16).collect();
    cfb.create_stream("/PlainSheet/Sheet1")
        .unwrap()
        .write_all(&sheet)
        .unwrap();

    let blob: Vec<u8> = (0u8..32).map(|i| i.wrapping_mul(7).wrapping_add(3)).collect();
    cfb.create_stream("/UnknownStorage/Blob")
        .unwrap()
        .write_all(&blob)
        .unwrap();

    cfb.flush().unwrap();
}
```

**重要**：合成 PID 不带 `ObjectGraph` stream，所以
`PidImportSummary::object_graph_available == false` →
R44-B 必发 `[NotImplemented] PID object graph stream missing` notice。
T6 直接利用这一性质做 stderr 断言。

### 3.2 cli_writes_pdf_for_minimal_pid

模板 mirror `cli_writes_pdf_for_minimal_dxf`：

```rust
#[test]
fn cli_writes_pdf_for_minimal_pid() {
    let tmp = std::env::temp_dir();
    let pid_path = tmp.join(format!("h7cad_cli_test_{}_input.pid", std::process::id()));
    let pdf_path = tmp.join(format!("h7cad_cli_test_{}_pid_out.pdf", std::process::id()));

    build_minimal_pid_fixture(&pid_path);
    let _ = std::fs::remove_file(&pdf_path);

    let output = Command::new(binary_path())
        .arg(&pid_path)
        .arg("--export-pdf")
        .arg(&pdf_path)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .expect("spawn h7cad for PID export");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "PID --export-pdf exit code: {:?}\nstderr: {stderr}",
        output.status.code()
    );
    let bytes = std::fs::read(&pdf_path).expect("output pdf should exist");
    assert!(
        bytes.starts_with(b"%PDF-"),
        "expected PDF magic header, got {:?}",
        &bytes[..8.min(bytes.len())]
    );

    let _ = std::fs::remove_file(&pid_path);
    let _ = std::fs::remove_file(&pdf_path);
}
```

### 3.3 cli_exports_svg_for_minimal_pid

同形，断言 `bytes.starts_with(b"<?xml")` + `bytes.windows(4).any(|w| w == b"<svg")`。

### 3.4 cli_lists_layouts_for_minimal_pid_text_mode

```rust
#[test]
fn cli_lists_layouts_for_minimal_pid_text_mode() {
    let tmp = std::env::temp_dir();
    let pid_path = tmp.join(format!("h7cad_cli_test_{}_layouts.pid", std::process::id()));
    build_minimal_pid_fixture(&pid_path);

    let output = Command::new(binary_path())
        .arg(&pid_path)
        .arg("--list-layouts")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .expect("spawn h7cad --list-layouts on PID");

    assert!(output.status.success(), "stderr: {}",
            String::from_utf8_lossy(&output.stderr));
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.lines().any(|l| l.trim() == "Model"),
        "expected 'Model' in --list-layouts output, got: {stdout}"
    );

    let _ = std::fs::remove_file(&pid_path);
}
```

### 3.5 cli_lists_layouts_for_minimal_pid_json_mode

JSON parse 用 `serde_json::from_str::<serde_json::Value>` 验证 schema，
断言 `obj["inputs"][0]["layouts"]` 含 "Model"。

### 3.6 cli_pid_input_surfaces_warning_for_unresolved_relationships

实际上合成 PID 的关键差异点是 `object_graph_available == false`，
不是 unresolved_relationship_count。改名 / 调整断言以反映真实信号：

```rust
#[test]
fn cli_pid_input_surfaces_notimplemented_for_missing_object_graph() {
    let tmp = std::env::temp_dir();
    let pid_path = tmp.join(format!("h7cad_cli_test_{}_notice.pid", std::process::id()));
    let pdf_path = tmp.join(format!("h7cad_cli_test_{}_notice.pdf", std::process::id()));
    build_minimal_pid_fixture(&pid_path);

    let output = Command::new(binary_path())
        .arg(&pid_path)
        .arg("--export-pdf")
        .arg(&pdf_path)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .expect("spawn h7cad PID notice test");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "expected exit 0 (notice should not block export); stderr: {stderr}"
    );

    // R44-B: a synthesised PID without an ObjectGraph stream
    // must surface `[NotImplemented] PID object graph stream missing`
    // on stderr.  This is the end-to-end gate for the
    // pid_summary_to_notices → format_notices_for_cli → eprintln!
    // path that R44-B's unit tests cover only piecewise.
    assert!(
        stderr.contains("[NotImplemented]"),
        "expected '[NotImplemented]' tag in stderr, got: {stderr}"
    );
    assert!(
        stderr.contains("object graph"),
        "expected 'object graph' in stderr, got: {stderr}"
    );
    assert!(
        stderr.contains(
            pid_path.file_name().unwrap().to_string_lossy().as_ref()
        ),
        "expected fixture path in stderr line prefix, got: {stderr}"
    );

    let _ = std::fs::remove_file(&pid_path);
    let _ = std::fs::remove_file(&pdf_path);
}
```

---

## 4. 测试

### 4.1 集成测试（`tests/cli_batch_export.rs`）

新增 5 条：
- `cli_writes_pdf_for_minimal_pid`
- `cli_exports_svg_for_minimal_pid`
- `cli_lists_layouts_for_minimal_pid_text_mode`
- `cli_lists_layouts_for_minimal_pid_json_mode`
- `cli_pid_input_surfaces_notimplemented_for_missing_object_graph`

### 4.2 单元测试

不新增（R44-B 已经把 unit 层覆盖到底；R44-C 是端到端层）。

---

## 5. 验收

```bash
cargo check -p H7CAD                                # 零新 warning
cargo test --test cli_batch_export                  # 16 → 21 全绿（+5 R44-C）
cargo test --bin H7CAD                              # 514 / 514 不变（unit 层零改动）
cargo test --locked --workspace --all-targets       # 全绿
RUSTFLAGS=-Dwarnings cargo check --locked --workspace --all-targets  # 零新 warning
```

ReadLints 改动文件零 lint：
- `tests/cli_batch_export.rs`
- `docs/cli.md`
- `CHANGELOG.md`
- `docs/plans/2026-04-25-cli-pid-integration-tests-plan.md`

手动：跑 `cargo test --test cli_batch_export -- --nocapture cli_pid_input_surfaces_notimplemented` →
应在 cargo 输出里看到捕获的 `[NotImplemented] PID object graph stream
missing; layout falls back to grid` stderr 行。

---

## 6. 状态

- [x] 计划定稿（2026-04-25）
- [x] T1 `build_minimal_pid_fixture` helper
- [x] T2 `cli_writes_pdf_for_minimal_pid`
- [x] T3 `cli_exports_svg_for_minimal_pid`
- [x] T4 `cli_lists_layouts_for_minimal_pid_text_mode`
- [x] T5 `cli_lists_layouts_for_minimal_pid_json_mode`
- [x] T6 `cli_pid_input_surfaces_notimplemented_for_missing_object_graph`
- [x] T7 CHANGELOG R44-C 章节 + docs/cli.md Future flags 更新
- [x] T8 全量验证

---

## 7. 下轮方向

- **R44-D**：真实 `.dwg` fixture 端到端集成测试（仓库无 fixture，需
  从外部源获取 / 合成 AC1015 二进制；与 R44-C 同形但 fixture 投入
  显著更大）
- **R43-A2**：SVG `<pattern>` 元素优化（替换 polyline × N，文件 5-10×
  压缩；需要研究 Inkscape / 浏览器 patternUnits=userSpaceOnUse 兼容性）
- **R43-B**：Rational NURBS（degree-2/3 + 非 unit weight）
- **R43-C**：OLE2_FRAME entity 渲染
- **R45**：Paper Space 多视口合成（`setupActiveLayoutViews` parity）
- **R46**：将 `build_minimal_pid_fixture` 抬到 `h7cad-native-testkit`
  里供 GUI / facade / 多个集成测试共用（如果 R44-D 也需要类似 fixture
  helper）
