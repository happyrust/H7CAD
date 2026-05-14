# CLI Layout BC：JSON 字段 override + `--list-layouts --json`（R42-BC）

> **起稿**：2026-04-25（第四十二轮 · BC 阶段）
> **前置**：R42-A（CLI `--layout NAME` flag + `--list-layouts` 子命令）已落地。
>
> **目标**：完成 R42 三件套——
>
> - **B**：让 `--options` JSON 也能 override layout（`SvgExportOptions` /
>   `PdfExportOptions` 加 `layout: Option<String>` 字段）。优先级 CLI flag >
>   JSON > Scene 默认 "Model"。
> - **C**：`--list-layouts --json` 输出 JSON 数组方便 `jq` / 脚本处理；不带
>   `--json` 时仍是行模式（向后兼容）。
>
> 这一轮把 R42 整个三件套收完，使其完全 self-contained。

---

## 1. 现状

### 1.1 R42-A 落地

- `--layout NAME` flag 设 `scene.current_layout = name`
- `--list-layouts` 子命令打印 `layout_names()` 按行
- `BatchArgs::Export` 已有 `layout: Option<String>` 字段（CLI 层）
- `BatchArgs::ListLayouts { inputs }` 子命令分支

### 1.2 缺口

- **B**：JSON `--options` 不能设 layout——脚本必须每次都拼 `--layout NAME` flag，不能把 layout 一次性写进 opts.json 让 CI pipeline 里所有 export 共享
- **C**：`--list-layouts` 输出是行格式，`jq` 没法直接消费；多输入的 `=== <path> ===` 头需要 awk/sed 解析

### 1.3 约束

- 所有 pre-R42-B `--options` 文件一字节不变继续 parse 成功（缺失 `layout` 字段时 `serde(default)` 给 None）
- `--list-layouts` 默认（不带 `--json`）输出**字节级保持**——R42-A 的 `cli_list_layouts_for_minimal_dxf_includes_model` 集成测试是硬性 canary

---

## 2. 范围

| 任务 | 优先级 | 预估 |
|------|-------|------|
| T1 `SvgExportOptions` / `PdfExportOptions` 各加 `pub layout: Option<String>` 字段 + Default impl 设 None | P0 | 0.2 h |
| T2 `LoadedOptions` 暴露 `layout()` 访问器（PDF / SVG 分支统一返回 `Option<&str>`） | P0 | 0.2 h |
| T3 `export_one` 优先级裁决：`cli_layout.or_else(|| opts.layout())` → 一个 `effective_layout` 字符串走 validate + `scene.current_layout` 路径 | P0 | 0.3 h |
| T4 `BatchArgs::ListLayouts` 加 `as_json: bool` 字段；`parse_batch_args` 识别 `--list-layouts --json` 组合 | P0 | 0.3 h |
| T5 `run_list_layouts` 在 `as_json=true` 时输出 JSON：`{"inputs":[{"path":"a.dxf","layouts":["Model","Layout1"]},{"path":"b.dxf","error":"..."}]}` | P0 | 0.4 h |
| T6 单元测试（`src/cli.rs::tests`）：JSON `layout` 加载 / 优先级裁决 / `--list-layouts --json` parser / 缺失 `--json` 仍行格式 | P0 | 0.4 h |
| T7 集成测试（`tests/cli_batch_export.rs`）：JSON layout override / CLI flag 覆盖 JSON / `--list-layouts --json` 输出可被 serde_json 解析 | P0 | 0.4 h |
| T8 更新 `docs/cli.md` + CHANGELOG R42-BC 章节 | P0 | 0.2 h |
| T9 跑 `cargo test --locked --workspace --all-targets` + `RUSTFLAGS=-Dwarnings cargo check --locked --workspace --all-targets` | P0 | 0.2 h |

**不纳入**：
- DWG CLI 输入批处理（R44）
- SVG `<pattern>` 真实 hatch（R43-A）
- Rational NURBS / OLE 对象（R43-B/C）
- Paper Space 多视口合成（R45）

---

## 3. 设计

### 3.1 字段添加

```rust
// src/io/svg_export.rs
#[serde(default)]
pub struct SvgExportOptions {
    // ... existing ...
    /// **R42-B**: optional layout name to export.  `None` (default) leaves
    /// the layout decision to the CLI `--layout` flag, which in turn
    /// defaults to `"Model"` when neither is set.  Set this in your
    /// `--options` JSON to bake a layout choice into a reusable config
    /// file:
    ///
    /// ```json
    /// { "color_policy": "grayscale", "layout": "Layout1" }
    /// ```
    ///
    /// CLI flag `--layout` always wins when both are present.
    pub layout: Option<String>,
}
```

PDF 端镜像同样字段。Default impl 都加 `layout: None`。

### 3.2 优先级裁决

```rust
// src/cli.rs::export_one
fn export_one(
    input: &Path,
    output: &Path,
    format: ExportFormat,
    options: &LoadedOptions,
    cli_layout: Option<&str>,    // R42-A flag
) -> Result<(), String> {
    // ... load + scene init ...

    // R42-B: CLI flag wins over JSON; both default to None which falls
    // through to Scene's "Model" default for byte-identical pre-R42 output.
    let effective_layout: Option<&str> = cli_layout.or_else(|| options.layout());
    if let Some(name) = effective_layout {
        let available = scene.layout_names();
        if !available.iter().any(|n| n == name) {
            return Err(format!(
                "unknown layout \"{}\".  Available: {}",
                name, available.join(", ")
            ));
        }
        scene.current_layout = name.to_string();
    }
    // ... rest unchanged ...
}
```

`LoadedOptions::layout()` 访问器：

```rust
impl LoadedOptions {
    fn layout(&self) -> Option<&str> {
        match self {
            LoadedOptions::Pdf(o) => o.layout.as_deref(),
            LoadedOptions::Svg(o) => o.layout.as_deref(),
        }
    }
}
```

### 3.3 `--list-layouts --json`

`BatchArgs::ListLayouts` 扩展：

```rust
pub enum BatchArgs {
    // ...
    ListLayouts {
        inputs: Vec<PathBuf>,
        as_json: bool,
    },
}
```

Parser 识别：

```rust
if args.iter().any(|a| a == "--list-layouts") {
    let as_json = args.iter().any(|a| a == "--json");
    let inputs: Vec<PathBuf> = args
        .iter()
        .filter(|s| !s.starts_with('-'))
        .map(PathBuf::from)
        .collect();
    if inputs.is_empty() { return None; }
    return Some(BatchArgs::ListLayouts { inputs, as_json });
}
```

JSON 输出 schema：

```json
{
  "inputs": [
    { "path": "drawing.dxf", "layouts": ["Model", "Layout1", "Layout2"] },
    { "path": "broken.dxf",  "error":  "failed to load: ..." }
  ]
}
```

每个输入要么有 `layouts: Vec<String>` 要么有 `error: String`。这样 `jq` 能
做 `.inputs[] | select(.layouts) | .layouts[]` 这种过滤。

实现：

```rust
fn run_list_layouts(inputs: &[PathBuf], as_json: bool) -> Result<(), String> {
    if !as_json {
        // R42-A behaviour, byte-identical
        return run_list_layouts_text(inputs);
    }

    #[derive(serde::Serialize)]
    struct InputResult {
        path: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        layouts: Option<Vec<String>>,
        #[serde(skip_serializing_if = "Option::is_none")]
        error: Option<String>,
    }
    #[derive(serde::Serialize)]
    struct Output {
        inputs: Vec<InputResult>,
    }

    let mut had_error = false;
    let mut results = Vec::with_capacity(inputs.len());
    for input in inputs {
        match crate::io::load_file_with_native_blocking(input) {
            Ok((compat, native, _)) => {
                let mut scene = crate::scene::Scene::new();
                scene.document = compat;
                scene.set_native_doc(native);
                results.push(InputResult {
                    path: input.display().to_string(),
                    layouts: Some(scene.layout_names()),
                    error: None,
                });
            }
            Err(e) => {
                had_error = true;
                results.push(InputResult {
                    path: input.display().to_string(),
                    layouts: None,
                    error: Some(format!("{e}")),
                });
            }
        }
    }
    let out = Output { inputs: results };
    println!(
        "{}",
        serde_json::to_string_pretty(&out)
            .map_err(|e| format!("failed to serialise list-layouts JSON: {e}"))?
    );
    if had_error {
        Err(format!("{} of {} inputs could not be loaded", inputs.len(), inputs.len()))
    } else {
        Ok(())
    }
}
```

注意：JSON 模式下错误 input 在数组里仍占一格（带 `error` 字段），exit code
为 1，但 stdout 仍是合法 JSON——这样 `jq` 不会因为部分失败就 parse 失败。

### 3.4 `HELP_TEXT` 扩展

```
LAYOUT SELECTION (R42):
    --layout NAME        Export a specific paper layout instead of Model
                         space.  CLI flag wins over the JSON layout field.
    --list-layouts       Print available layouts (one per line by default).
    --list-layouts --json
                         Print as a JSON document for jq/awk consumption:
                         { "inputs": [{ "path": ..., "layouts": [...] }] }
                         Failed inputs appear with { "error": "..." }.
```

---

## 4. 测试

### 4.1 单元测试（`src/cli.rs::tests`）

新增 4 条：

- `parse_list_layouts_with_json_flag` — `--list-layouts --json INPUT.dxf` → `as_json: true`
- `parse_list_layouts_without_json_defaults_text_mode` — `--list-layouts INPUT.dxf` → `as_json: false`
- `load_pdf_options_json_layout_field_deserialises` — `{"layout":"L1"}` → `opts.layout == Some("L1")`
- `load_svg_options_json_layout_field_deserialises` — 同上 SVG

既有 `BatchArgs::ListLayouts` 构造点（`parse_recognises_list_layouts_subcommand`
等 3 条）需更新匹配 `as_json: false` 字段。

### 4.2 集成测试（`tests/cli_batch_export.rs`）

新增 3 条：

- `cli_export_pdf_with_layout_in_options_json_succeeds` — 写 `{"layout":"Model"}` 到 opts.json，传 `--export-pdf --options opts.json` 不带 `--layout`，应正确导出
- `cli_layout_flag_overrides_json_layout_field` — 同时存在 `--layout Model` 和 JSON `{"layout":"NoSuchLayout"}`，CLI 应胜出 → 成功
- `cli_list_layouts_json_output_is_valid_json_with_expected_schema` — `--list-layouts --json INPUT.dxf` → stdout 用 `serde_json::from_str` parse 成功，`.inputs[0].layouts` 含 "Model"

### 4.3 既有测试调整

`BatchArgs::ListLayouts { inputs }` → `BatchArgs::ListLayouts { inputs, as_json: false }`，
影响 R42-A 的 3 条 parser 单元测试，需要小幅更新（`as_json: false` 字面值或
helper 函数）。

---

## 5. 验收

```bash
cargo check -p H7CAD                                # 零新 warning
cargo test --bin H7CAD cli::                        # 29 → 33 全绿
cargo test --test cli_batch_export                  # 13 → 16 全绿
cargo test --bin H7CAD                              # 491 → 495 全绿
cargo test --locked --workspace --all-targets       # 全绿
RUSTFLAGS=-Dwarnings cargo check --locked --workspace --all-targets  # 零新 warning
```

手动：
- `echo '{"layout":"Model"}' > opts.json && h7cad.exe drawing.dxf --export-pdf --options opts.json out.pdf` → 正确导出
- `h7cad.exe drawing.dxf --list-layouts --json | jq -r '.inputs[0].layouts[]'` → 应输出 layout 名（每行一个，与 jq 文本流交互）

---

## 6. 状态

- [x] 计划定稿（2026-04-25）
- [x] T1 双 Options 加 `layout` 字段
- [x] T2 `LoadedOptions::layout()` 访问器
- [x] T3 优先级裁决在 `export_one`
- [x] T4 `ListLayouts` 加 `as_json` 字段 + parser 识别
- [x] T5 `run_list_layouts` JSON 输出
- [x] T6 单元测试 4 + 既有 3 调整
- [x] T7 集成测试 3
- [x] T8 docs + CHANGELOG
- [x] T9 全量验证

---

## 7. 下轮方向

- **R43-A**：SVG `<pattern>` 真实 hatch（替换 R34/R35 的 polyline pattern）
- **R43-B**：Rational NURBS（degree-2/3 + 非 unit weight）
- **R43-C**：OLE 对象（OLE2_FRAME entity 解析与渲染）
- **R44**：DWG CLI 输入批处理路径
- **R45**：Paper Space 多视口合成（`setupActiveLayoutViews` parity）
