# DWG CLI 输入正式化（R44-A）

> **起稿**：2026-04-25（第四十四轮 · A 阶段）
> **前置**：R36-R42 CLI 工作链已经基于 `crate::io::load_file_with_native_blocking`
> 加载所有 CAD 输入；该函数早就同时处理 DWG / DXF（`load_file_native_blocking`
> 按扩展名 dispatch 到 `load_dwg_native_blocking` 或 `load_dxf_native_blocking`）。
>
> **当前缺口**：
> 1. CLI `export_one` 的 `let (compat, native, _notices) = ...?` 把
>    `_notices` **丢弃**——DWG 的 fallback advisory / Warning 用户在 GUI
>    里能看到，CLI 路径完全静默
> 2. `HELP_TEXT` 只列 `<INPUT.dxf>` 占位符，没说明 DWG 也支持
> 3. `docs/cli.md` 没 DWG 输入小节
> 4. 没有 CLI DWG 输入的集成测试
>
> **目标**：把已经隐式工作的 DWG CLI 输入路径正式化——surface notices 到
> stderr（与 GUI 平等的 1st-class 体验），加 docs / HELP / 单元测试。

---

## 1. 现状

### 1.1 已经能跑的部分

```rust
// src/cli.rs:export_one
let (compat, native, _notices) = crate::io::load_file_with_native_blocking(input)
    .map_err(|e| format!("failed to load \"{}\": {e}", input.display()))?;
```

`load_file_with_native_blocking(path)` (src/io/mod.rs:146) 调用
`load_file_native_blocking`，后者按扩展名 dispatch：

```rust
match ext.as_str() {
    "dwg" => load_dwg_native_blocking(path),    // ← 已经处理 DWG
    "dxf" => Ok((load_dxf_native_blocking(path)?, Vec::new())),
    "pid" => Ok((pid_import::load_pid_native(path)?, Vec::new())),
    _ => Err(OpenError::UnsupportedExtension { ext }),
}
```

`load_dwg_native_blocking` 走 R39+ 已经成熟的 acadrust + native fallback +
advisory notice 三层路径。CLI 直接复用，**没有任何额外工作**。

### 1.2 缺失：notice 输出

Notices 只在 `_notices` 接收，没人 surface 给用户。`OpenNotice` 已经有
`format_zh()`（中文）方法和 `NoticeSeverity::en_tag()`（英文 tag），CLI
应该用英文 tag（机器可读）。

### 1.3 缺失：HELP / docs / 测试

- `HELP_TEXT` USAGE 段：`h7cad <INPUT.dxf>... --export-pdf` —— 仅 DXF
- `docs/cli.md` Quick start 也只用 `drawing.dxf`
- `tests/cli_batch_export.rs` 全部用 `write_minimal_dxf_to`，无 DWG fixture

---

## 2. 范围

| 任务 | 优先级 | 预估 |
|------|-------|------|
| T1 `OpenNotice::format_en()` 助手（snake/title-case en tag + message），方便 CLI 与未来日志/文件 logger 复用 | P0 | 0.2 h |
| T2 `cli.rs::format_notices_for_cli` 辅助：把 `&[OpenNotice]` 折叠成 stderr 多行字符串（每行 `h7cad: notice [Warning] ...`） | P0 | 0.3 h |
| T3 `export_one` 与 `run_list_layouts` 把 notices 输出到 stderr；多输入时每个 input 的 notices 紧跟在加载后 | P0 | 0.3 h |
| T4 `HELP_TEXT` 加 INPUT FORMATS 段（DWG + DXF + PID 占位）+ DWG NOTES 段说明 fallback 行为 | P0 | 0.3 h |
| T5 `docs/cli.md` 加 "DWG input (R44-A)" 子节：支持版本、fallback 流程、notice 行为 | P0 | 0.3 h |
| T6 单元测试 3 条：`format_en` / `format_notices_for_cli` 空列表 / 多 severity 混合 | P0 | 0.3 h |
| T7 CHANGELOG R44-A 章节 | P0 | 0.2 h |
| T8 跑 `cargo test --locked --workspace --all-targets` + `RUSTFLAGS=-Dwarnings cargo check --locked --workspace --all-targets` | P0 | 0.2 h |

**不纳入**：
- DWG 写出 CLI flag（`--save-dwg`）
- DXF/DWG 在线 round-trip 测试
- PID CLI 输入正式化（R44-B 可选）
- 真实 DWG 文件 fixture 集成测试（合成 DWG 复杂；R44 的现有 advisory /
  fallback 路径已经在 GUI 端被多个真实 fixture 测试覆盖，CLI 路径只是相同
  load 函数的薄包装，无须重复 fixture）

---

## 3. 设计

### 3.1 `OpenNotice::format_en`

`src/io/diagnostics.rs` 加：

```rust
impl OpenNotice {
    /// English-facing one-line form suitable for CLI / log output:
    /// `"[Warning] native DWG fallback opened file ..."`.
    /// Mirrors `format_zh()` in shape, but uses `NoticeSeverity::en_tag()`
    /// so the prefix stays stable and grep-friendly across locales.
    pub fn format_en(&self) -> String {
        format!("[{}] {}", self.severity.en_tag(), self.message)
    }
}
```

`en_tag()` 已经存在（标记为 `#[allow(dead_code)]`），R44-A 把它从 dead-code
转为正式使用——同时移除 `#[allow(dead_code)]` 标注。

### 3.2 `cli.rs` 内部 helper

```rust
/// **R44-A**: format a slice of OpenNotice for stderr output.  Returns
/// an empty string when `notices` is empty so callers can splice
/// unconditionally without worrying about extra blank lines.
fn format_notices_for_cli(input: &Path, notices: &[OpenNotice]) -> String {
    if notices.is_empty() {
        return String::new();
    }
    let mut out = String::new();
    for n in notices {
        let _ = writeln!(out, "h7cad: {}: notice {}", input.display(), n.format_en());
    }
    out
}
```

`stderr` 输出格式：

```
h7cad: drawing.dwg: notice [Warning] native DWG fallback opened file after acadrust failed: ... (recovered 84 entities)
h7cad: drawing.dwg: notice [NotImplemented] some DWG record skipped: ...
```

### 3.3 派发到 export_one + run_list_layouts

```rust
fn export_one(input, output, format, options, layout) -> Result<(), String> {
    // ... existing load ...
    let (compat, native, notices) = crate::io::load_file_with_native_blocking(input)
        .map_err(|e| format!("failed to load \"{}\": {e}", input.display()))?;

    // R44-A: surface notices to stderr so DWG fallback / Warning info
    // reaches the user just like the GUI's notice panel does.
    let notice_lines = format_notices_for_cli(input, &notices);
    if !notice_lines.is_empty() {
        eprint!("{}", notice_lines);  // already includes trailing \n per line
    }

    // ... rest of existing function ...
}
```

`run_list_layouts_text` 与 `run_list_layouts_json` 同样处理：load 后立刻
surface notices（JSON 模式下 notices 写到 stderr，stdout 仍是干净 JSON）。

### 3.4 `HELP_TEXT` 扩展

```
INPUT FORMATS:
    .dxf     AutoCAD DXF — fully supported (R36+)
    .dwg     AutoCAD DWG — supported via acadrust primary reader,
             with `h7cad-native-dwg` AC1015 fallback when the primary
             rejects the bytes (R44-A).  Fallback success surfaces a
             [Warning] notice on stderr; both paths still produce a
             valid PDF / SVG.

NOTICES (R44-A):
    DWG opens may surface non-fatal notices on stderr:
      h7cad: <input>: notice [Warning] native DWG fallback opened ...
      h7cad: <input>: notice [NotImplemented] feature X skipped ...
    Notices are *additive* — they never block the export.  Pipe stderr
    to /dev/null (or NUL on Windows) if your CI doesn't want to see
    them.
```

### 3.5 `docs/cli.md` 更新

新增 "DWG input (R44-A)" 小节：

- 文件扩展名 `.dwg` 自动按 DWG 路径加载
- acadrust 主路径 + h7cad-native-dwg AC1015 fallback
- fallback 成功时 stderr 有 `[Warning]` 行，stdout / 输出文件不受影响
- fallback 失败时返回原 acadrust 错误，整体 exit 1

---

## 4. 测试

### 4.1 单元测试（`src/io/diagnostics.rs::tests`）

新增 1 条：

- `open_notice_format_en_uses_severity_tag` — `Warning + "msg"` →
  `"[Warning] msg"`；其它 3 个 severity 也覆盖

### 4.2 单元测试（`src/cli.rs::tests`）

新增 2 条：

- `format_notices_for_cli_empty_returns_empty_string`
- `format_notices_for_cli_multi_severity_emits_one_line_each` — 一个
  Warning + 一个 NotImplemented → 输出 2 行，各含对应 tag 与 input path

### 4.3 集成测试

不新增——CLI DWG load 路径只是 `load_file_with_native_blocking` 薄包装，
GUI 端已经被 R39 + R41 多个 fixture 覆盖；R44-A 增加 fixture 投入产出比低，
留给 R44-B 系统化处理（届时也可以考虑用合成 AC1015 fixture）。

---

## 5. 验收

```bash
cargo check -p H7CAD                                # 零新 warning
cargo test --bin H7CAD io::diagnostics              # +1 R44-A 全绿
cargo test --bin H7CAD cli::                        # 33 → 35 全绿
cargo test --bin H7CAD                              # 505 → 508 全绿
cargo test --test cli_batch_export                  # 16 / 16 不变
cargo test --locked --workspace --all-targets       # 全绿
RUSTFLAGS=-Dwarnings cargo check --locked --workspace --all-targets  # 零新 warning
```

手动：找一个本地 .dwg 文件 → `h7cad.exe drawing.dwg --export-pdf out.pdf`
→ 应正常产 PDF；如果该 DWG 是 AC1015 走 fallback，stderr 应输出
`h7cad: drawing.dwg: notice [Warning] native DWG fallback opened ...`。

---

## 6. 状态

- [x] 计划定稿（2026-04-25）
- [x] T1 `OpenNotice::format_en`
- [x] T2 `format_notices_for_cli` helper
- [x] T3 `export_one` + `run_list_layouts` (text + json) surface notices
- [x] T4 `HELP_TEXT` 加 INPUT FORMATS + NOTICES 段
- [x] T5 `docs/cli.md` 加 DWG 子节
- [x] T6 单元测试 3 条
- [x] T7 CHANGELOG R44-A 章节
- [x] T8 全量验证

---

## 7. 下轮方向

- **R44-B**：PID CLI 输入正式化（如 `pid-parse` 仓的 CLI 已经有对应能力，
  H7CAD CLI 可以镜像）+ 真实 DWG fixture 集成测试
- **R43-A2**：SVG `<pattern>` 元素优化
- **R43-B**：Rational NURBS（degree-2/3 + 非 unit weight）
- **R43-C**：OLE 对象（OLE2_FRAME entity）
- **R45**：Paper Space 多视口合成（`setupActiveLayoutViews` parity）
