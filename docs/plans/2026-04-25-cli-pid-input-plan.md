# PID CLI 输入正式化（R44-B）

> **起稿**：2026-04-25（第四十四轮 · B 阶段）
> **前置**：R44-A 把 DWG CLI 输入正式化（`OpenNotice::format_en` + CLI
> stderr surfacing + HELP_TEXT INPUT FORMATS / NOTICES 段）。
> R36 起 `load_file_native_blocking` 的 `match ext` 已经把 `.pid` 路径
> 派发到 `pid_import::load_pid_native(path)`——CLI 端**早就能加载**
> `.pid` 文件，只是：
>
> 1. PID 路径返回的 `notices` 永远是 `Vec::new()`——`PidImportSummary`
>    里已有的 `unresolved_relationship_count` / `object_graph_available`
>    等关键诊断信号被 io 层吞掉
> 2. `HELP_TEXT` 的 INPUT FORMATS 段写"CLI batch path currently writes
>    errors when given .pid input (R44-B)"——**实际行为早已不再 error**，
>    描述与代码漂移了至少 R36 一轮
> 3. `docs/cli.md` 没 PID input 子节
> 4. CLI 单元测试无 PID 路径覆盖
>
> **目标**：把已经隐式工作的 PID CLI 输入路径正式化——从
> `PidImportSummary` 派生 `OpenNotice`，与 R44-A DWG 通道走同一处理
> 链；同步修正 HELP_TEXT 与 docs；零 fixture 依赖通过 unit test 完成
> 闭环。真实 `.pid` 端到端集成测试留给 R44-C/D 当 fixture 投入到位
> 时再做。

---

## 1. 现状

### 1.1 已经能跑的部分

```rust
// src/io/mod.rs:188-205 (R36+)
pub fn load_file_native_blocking(
    path: &Path,
) -> Result<(NativeCadDocument, Vec<OpenNotice>), OpenError> {
    let ext = ...;
    match ext.as_str() {
        "dwg" => load_dwg_native_blocking(path),
        "dxf" => Ok((load_dxf_native_blocking(path)?, Vec::new())),
        "pid" => Ok((
            pid_import::load_pid_native(path).map_err(OpenError::from)?,
            Vec::new(),  // ← 全空，PID 诊断信号被吞
        )),
        _ => Err(OpenError::UnsupportedExtension { ext }),
    }
}
```

CLI `export_one` / `run_list_layouts_text` / `run_list_layouts_json`
都通过 `load_file_with_native_blocking` 加载——PID 因此**早就能跑通**：

```bash
h7cad drawing.pid --export-pdf out.pdf      # 已经能产出 PDF
h7cad drawing.pid --list-layouts            # 已经返回 Model
h7cad drawing.pid --export-svg out.svg      # 已经能产出 SVG
```

### 1.2 缺失：notice 输出

`load_pid_native(path)` 内部走 `open_pid` → `parse_package` →
`merge_publish_sidecars` → `derive_layout` → `pid_document_to_bundle`，
最后产出 `PidOpenBundle { native_preview, summary, ... }`。
`PidImportSummary` 已经携带了所有诊断指标：

```rust
pub struct PidImportSummary {
    pub object_count: usize,
    pub relationship_count: usize,
    pub unresolved_relationship_count: usize,  // ← 关键
    pub object_graph_available: bool,           // ← 关键
    pub symbol_count: usize,
    // ...
}
```

但 `load_pid_native` 把 `summary` 丢弃，只保留 `native_preview`：

```rust
pub fn load_pid_native(path: &Path) -> Result<nm::CadDocument, String> {
    Ok(open_pid(path)?.native_preview)  // summary 丢弃
}
```

io 层因此只能 `Vec::new()` 凑合。

### 1.3 缺失：HELP_TEXT / docs 漂移

`HELP_TEXT` INPUT FORMATS 段（src/cli.rs:235-243）：

```
.pid     SmartPlant P&ID — opens via the GUI; CLI batch path
         currently writes errors when given .pid input (R44-B).
```

—— **过期**。CLI 早已不再 error，会正常 export。

`docs/cli.md` 整篇文档没有 PID 子节，对照 R44-A 的"DWG input"小节
缺一对一对称。

### 1.4 缺失：单元测试

- `src/io/pid_import.rs` 的 tests 模块没有"summary → notices"派生路径
- `src/cli.rs` 的 tests 模块的 `format_notices_for_cli_*` 测试不区分
  来源（DWG / PID），只验证格式

---

## 2. 范围

| 任务 | 优先级 | 预估 |
|------|-------|------|
| T1 `pid_import::pid_summary_to_notices(&summary) -> Vec<OpenNotice>` 纯函数（unresolved_count > 0 / object_graph_available == false → notices） | P0 | 0.3 h |
| T2 `pid_import::load_pid_native_with_notices(path) -> Result<(CadDocument, Vec<OpenNotice>), String>` 包装（沿用现有 open_pid，附加 T1 诊断派发） | P0 | 0.2 h |
| T3 `io::mod.rs::load_file_native_blocking` PID 分支改用 T2 | P0 | 0.2 h |
| T4 `HELP_TEXT` 修正 PID 描述（撕掉"writes errors"，改成 R44-B 状态描述） | P0 | 0.2 h |
| T5 `docs/cli.md` 加 "PID input (R44-B)" 子节，与 "DWG input (R44-A)" 平行 | P0 | 0.3 h |
| T6 单元测试 4 条：summary→notice 派生（unresolved>0 / graph 缺 / 双触发 / 全干净） | P0 | 0.4 h |
| T7 CLI 测试 1 条：mock notice 经 `format_notices_for_cli` 输出 PID 风格 stderr | P0 | 0.2 h |
| T8 CHANGELOG R44-B 章节 | P0 | 0.2 h |
| T9 跑 `cargo test --locked --workspace --all-targets` + `RUSTFLAGS=-Dwarnings cargo check --locked --workspace --all-targets` + ReadLints | P0 | 0.2 h |

**不纳入**：
- 真实 `.pid` fixture 集成测试（仓库无 fixture，构造 `.pid` 需要 CFB
  + 上百 stream 实际不可手写。R44-C / R44-D 待用户提供 fixture 时再做）
- 把 PID 写出（`--save-pid` 等）纳入 CLI（PID 写出走 `pid_import::save_pid_native`，
  当前 CLI 无 `--save-*` flag 任何路径）
- 真实 `.dwg` fixture 集成测试（R44-A 计划文件 §7 列的另一项；同 fixture 投入问题，
  延后）
- PID parser 自身的诊断升级（pid_parse 仓的事）

---

## 3. 设计

### 3.1 `pid_summary_to_notices`：纯函数派生

放在 `src/io/pid_import.rs`，**对外 pub**——便于未来 GUI 端 notice
panel 也复用同一派发逻辑。

```rust
/// **R44-B**: derive non-fatal `OpenNotice`s from a `PidImportSummary`.
///
/// PID parsing itself succeeds in `open_pid` (or it would have returned
/// `Err`), but a successfully parsed PID can still surface diagnostic
/// signals worth showing the user:
///
/// - `unresolved_relationship_count > 0` ⇒ some relationships in the
///   document point at endpoints we could not resolve (missing object,
///   placeholder GUID, …). They are dropped from the layout.  Tagged
///   `Warning` so the CLI prints a one-liner; users can re-export
///   without these relationships affecting downstream PDF / SVG.
/// - `!object_graph_available` ⇒ the package is missing or has a
///   damaged `object_graph` stream; layout falls back to a static grid.
///   Tagged `NotImplemented` to mirror the GUI's notice panel labelling
///   for "feature recognised but the data needed is absent".
///
/// Returns an empty `Vec` when the summary is fully clean — the
/// caller can splice unconditionally.
pub fn pid_summary_to_notices(summary: &PidImportSummary) -> Vec<OpenNotice> {
    let mut notices = Vec::new();
    if summary.unresolved_relationship_count > 0 {
        notices.push(OpenNotice::new(
            NoticeSeverity::Warning,
            format!(
                "PID has {} unresolved relationship{} (dropped from layout)",
                summary.unresolved_relationship_count,
                if summary.unresolved_relationship_count == 1 { "" } else { "s" },
            ),
        ));
    }
    if !summary.object_graph_available {
        notices.push(OpenNotice::new(
            NoticeSeverity::NotImplemented,
            "PID object graph stream missing; layout falls back to grid",
        ));
    }
    notices
}
```

需要在 `src/io/pid_import.rs` 顶部 import `crate::io::diagnostics::{NoticeSeverity, OpenNotice}`.

### 3.2 `load_pid_native_with_notices`

```rust
/// **R44-B**: PID load returning both the native document and the
/// notice vector derived from `PidImportSummary`.  Mirrors
/// `load_dwg_native_blocking`'s signature shape so io-mod's
/// `load_file_native_blocking` can dispatch uniformly.
pub fn load_pid_native_with_notices(
    path: &Path,
) -> Result<(nm::CadDocument, Vec<OpenNotice>), String> {
    let bundle = open_pid(path)?;
    let notices = pid_summary_to_notices(&bundle.summary);
    Ok((bundle.native_preview, notices))
}
```

`load_pid_native(path)` (老签名) 保留——GUI 端 / 既有调用者不破。

### 3.3 io::mod.rs 接入

```rust
pub fn load_file_native_blocking(
    path: &Path,
) -> Result<(NativeCadDocument, Vec<OpenNotice>), OpenError> {
    // ...
    match ext.as_str() {
        "dwg" => load_dwg_native_blocking(path),
        "dxf" => Ok((load_dxf_native_blocking(path)?, Vec::new())),
        "pid" => pid_import::load_pid_native_with_notices(path).map_err(OpenError::from),
        _ => Err(OpenError::UnsupportedExtension { ext }),
    }
}
```

CLI 自动通过 `format_notices_for_cli`（R44-A）surface 到 stderr——零
CLI 改动需要。

### 3.4 HELP_TEXT 修正

src/cli.rs:235-243 改成：

```
INPUT FORMATS (R44-B):
    .dxf     AutoCAD DXF — fully supported (R36+).
    .dwg     AutoCAD DWG — supported via acadrust as the primary reader,
             with `h7cad-native-dwg` AC1015 fallback when the primary
             rejects the bytes.  Fallback success surfaces a
             [Warning] notice on stderr; both paths produce the same
             PDF / SVG output.
    .pid     SmartPlant P&ID — fully supported (R44-B).  Loads via
             `pid_parse` and renders the derived layout (objects,
             relationships, fallback grid).  Unresolved relationships
             and missing object-graph streams surface as
             [Warning] / [NotImplemented] notices on stderr.
```

注意 R44-B 这一轮把 INPUT FORMATS 段的版本标识从 `(R44-A)` bumps 到
`(R44-B)`——pid 行升级了语义，整段标识跟新。

### 3.5 `docs/cli.md` 新增小节

`## DWG input (R44-A)` 后，新增对称的 `## PID input (R44-B)` 子节：

```markdown
## PID input (R44-B)

The CLI accepts SmartPlant `.pid` inputs as transparently as
`.dxf` / `.dwg`:

    h7cad drawing.pid --export-pdf out.pdf
    h7cad drawing.pid --list-layouts
    h7cad drawing.pid --export-svg out.svg

PID opens go through `pid_parse`'s `PidParser::parse_package` followed
by sidecar merge (`_Data.xml` / `_Meta.xml`) and layout derivation.
The resulting native document carries the laid-out objects,
relationships, and a fallback grid when the object graph is missing.

### Notice output (R44-B)

Successful PID loads can still surface non-fatal notices when the
parsed package has incomplete diagnostic signals:

    h7cad: drawing.pid: notice [Warning] PID has 5 unresolved relationships (dropped from layout)
    h7cad: drawing.pid: notice [NotImplemented] PID object graph stream missing; layout falls back to grid

Same redirect rules as the DWG path apply (pipe stderr to /dev/null
or NUL to silence).
```

紧跟原 DWG 子节的"## R41 SVG compression"之前。

---

## 4. 测试

### 4.1 pid_import 单元测试 (`src/io/pid_import.rs::tests`)

新增 4 条：

```rust
#[test]
fn pid_summary_to_notices_clean_summary_returns_empty() {
    let s = clean_summary();  // helper: counts=0, graph available
    assert!(pid_summary_to_notices(&s).is_empty());
}

#[test]
fn pid_summary_to_notices_unresolved_emits_warning() {
    let mut s = clean_summary();
    s.unresolved_relationship_count = 3;
    let n = pid_summary_to_notices(&s);
    assert_eq!(n.len(), 1);
    assert_eq!(n[0].severity, NoticeSeverity::Warning);
    assert!(n[0].message.contains("3 unresolved relationships"));
}

#[test]
fn pid_summary_to_notices_unresolved_singular_uses_singular_noun() {
    let mut s = clean_summary();
    s.unresolved_relationship_count = 1;
    let n = pid_summary_to_notices(&s);
    assert!(n[0].message.contains("1 unresolved relationship "),
            "singular form expected, got: {}", n[0].message);
    assert!(!n[0].message.contains("relationships"));
}

#[test]
fn pid_summary_to_notices_missing_graph_emits_not_implemented() {
    let mut s = clean_summary();
    s.object_graph_available = false;
    let n = pid_summary_to_notices(&s);
    assert_eq!(n.len(), 1);
    assert_eq!(n[0].severity, NoticeSeverity::NotImplemented);
    assert!(n[0].message.contains("object graph"));
}

#[test]
fn pid_summary_to_notices_emits_both_when_both_signals_set() {
    let mut s = clean_summary();
    s.unresolved_relationship_count = 7;
    s.object_graph_available = false;
    let n = pid_summary_to_notices(&s);
    assert_eq!(n.len(), 2);
    let sevs: Vec<NoticeSeverity> = n.iter().map(|x| x.severity).collect();
    assert!(sevs.contains(&NoticeSeverity::Warning));
    assert!(sevs.contains(&NoticeSeverity::NotImplemented));
}
```

`clean_summary()` helper 返回所有字段都干净的 baseline summary（测试
模块本地）。

### 4.2 CLI 单元测试 (`src/cli.rs::tests`)

新增 1 条：

```rust
#[test]
fn format_notices_for_cli_handles_pid_style_notices() {
    use crate::io::{NoticeSeverity, OpenNotice};
    let path = Path::new("drawing.pid");
    let notices = vec![
        OpenNotice::new(NoticeSeverity::Warning,
                       "PID has 3 unresolved relationships (dropped from layout)"),
        OpenNotice::new(NoticeSeverity::NotImplemented,
                       "PID object graph stream missing; layout falls back to grid"),
    ];
    let out = format_notices_for_cli(path, &notices);
    let lines: Vec<&str> = out.lines().collect();
    assert_eq!(lines.len(), 2);
    assert!(lines[0].starts_with("h7cad: drawing.pid: notice [Warning]"));
    assert!(lines[0].contains("3 unresolved relationships"));
    assert!(lines[1].starts_with("h7cad: drawing.pid: notice [NotImplemented]"));
    assert!(lines[1].contains("object graph"));
}
```

### 4.3 集成测试

不新增——CLI PID load 路径只是 `load_file_with_native_blocking` 薄包装，
io-mod 已经被多个 unit test 覆盖；R44-B 的核心增量（summary →
notices 派生）已经被 4.1 完整 unit test 覆盖。

真实 `.pid` 端到端集成测试（构造一个 minimal `.pid` 走 CFB / parse /
layout / export pipeline）超出本轮 scope（构造合法 PID CFB 的工具不
存在，需另开 plan）。

---

## 5. 验收

```bash
cargo check -p H7CAD                                # 零新 warning
cargo test --bin H7CAD io::pid_import               # +5 R44-B 全绿
cargo test --bin H7CAD io::diagnostics              # 9 / 9 不变（不受影响）
cargo test --bin H7CAD cli::                        # 35 → 36 全绿
cargo test --bin H7CAD                              # +6 全绿（pid_import 5 + cli 1）
cargo test --test cli_batch_export                  # 16 / 16 不变（DXF 路径 0 PID 触发）
cargo test --locked --workspace --all-targets       # 全绿
RUSTFLAGS=-Dwarnings cargo check --locked --workspace --all-targets  # 零新 warning
```

ReadLints 改动文件零 lint：
- `src/io/pid_import.rs`
- `src/io/mod.rs`
- `src/cli.rs`
- `docs/cli.md` (markdown，不参与 lint，仅审阅)

手动：找一个本地 `.pid`（GUI 已能打开的）→
`h7cad drawing.pid --export-pdf out.pdf` → 应正常产 PDF；如果 PID
有 unresolved 关系，stderr 应输出
`h7cad: drawing.pid: notice [Warning] PID has N unresolved relationships ...`

---

## 6. 状态

- [x] 计划定稿（2026-04-25）
- [x] T1 `pid_summary_to_notices`
- [x] T2 `load_pid_native_with_notices`
- [x] T3 io-mod PID dispatch
- [x] T4 `HELP_TEXT` PID 描述更正
- [x] T5 `docs/cli.md` 加 PID input 子节
- [x] T6 pid_import 5 条单元测试
- [x] T7 cli 1 条单元测试
- [x] T8 CHANGELOG R44-B 章节
- [x] T9 全量验证

---

## 7. 下轮方向

- **R44-C**：真实 `.pid` 端到端集成测试（待 fixture 投入到位；可能需要
  从 pid-parse 仓借 binary fixture，或编写最小 PID generator）
- **R44-D**：真实 `.dwg` 端到端集成测试（同 fixture 投入问题）
- **R43-A2**：SVG `<pattern>` 元素优化（替换 polyline × N，文件 5-10×
  压缩；需要研究 Inkscape / 浏览器 patternUnits=userSpaceOnUse 兼容性）
- **R43-B**：Rational NURBS（degree-2/3 + 非 unit weight）
- **R43-C**：OLE2_FRAME entity 渲染
- **R45**：Paper Space 多视口合成（`setupActiveLayoutViews` parity）
