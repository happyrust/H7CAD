# CLI Layout 选择 + `--list-layouts` 子命令（R42-A）

> **起稿**：2026-04-25（第四十二轮 · A 阶段）
> **前置**：R36-R38 CLI 批处理路径（PDF / SVG / 多输入 / `--options` JSON
> override）已经稳定。R40/R41 把 SVG/PDF export options 扩到完整 ODA parity。
> **当前缺口**：CLI `export_one` 默认 `scene.current_layout = "Model"`，导出
> Model space。Scene 已经原生支持 layout 切换（`current_layout: String` +
> `layout_names()` + `paper_limits()` 按 layout 切换），但 CLI 没有 flag 让用户
> 选——CI / 自动化管线无法批量导出 paper layouts。
>
> **目标**：给 CLI 加 `--layout NAME` flag 让用户选择 export 哪个 layout，加
> `--list-layouts` 子命令枚举可用 layout，让 shell 脚本可以 fan-out。

---

## 1. 现状

### 1.1 Scene 已经支持的 API（无需新增）

```rust
// src/scene/mod.rs
pub struct Scene {
    pub current_layout: String,       // "Model" | <paper layout name>
    // ...
}

impl Scene {
    pub fn current_layout_block_handle_pub(&self) -> Handle { ... }
    pub fn paper_limits(&self) -> Option<((f64, f64), (f64, f64))> { ... }
    pub fn entity_wires(&self) -> Vec<WireModel> { ... }
    pub fn layout_names(&self) -> Vec<String> { ... }
}
```

`entity_wires()` / `paper_limits()` 已经按 `current_layout` 自动过滤；`layout_names()`
返回 `["Model", "Layout1", "Layout2", ...]`。

### 1.2 CLI 当前流程

```rust
// src/cli.rs:export_one
let mut scene = crate::scene::Scene::new();   // current_layout = "Model"
scene.document = compat;
scene.set_native_doc(native);
let wires = scene.entity_wires();             // ← Model space wires only
```

CLI 没有路径让用户改 `current_layout`。

### 1.3 现有 BatchArgs

```rust
pub enum BatchArgs {
    Help,
    Export {
        format: ExportFormat,
        inputs: Vec<PathBuf>,
        output: ExportTarget,
        options_path: Option<PathBuf>,
    },
}
```

需要扩展：`Export` 加 `layout` 字段；新增 `ListLayouts` 分支。

---

## 2. 范围

| 任务 | 优先级 | 预估 |
|------|-------|------|
| T1 `BatchArgs::Export` 加 `layout: Option<String>` 字段；新增 `BatchArgs::ListLayouts { inputs: Vec<PathBuf> }` 分支 | P0 | 0.2 h |
| T2 `parse_batch_args` 识别 `--layout NAME`（任意位置，与 `--options` 同级正交）；识别 `--list-layouts` 子命令 | P0 | 0.4 h |
| T3 `export_one` 在 `entity_wires()` 之前 set `scene.current_layout`；未知 layout 名时报清晰错误（含可用 layouts 列表） | P0 | 0.3 h |
| T4 `run_list_layouts`：载入每个输入 → 构建 scene → 打印 `layout_names()` 按行；多输入时每个输入前打印 `=== <path> ===` 头 | P0 | 0.3 h |
| T5 单元测试（`src/cli.rs::tests`）：`--layout MODEL` / `--layout "Layout1"` / `--list-layouts` / 与 `--options` 任意顺序 / 与 `--export-*` 任意顺序 / 缺失 layout 名报错 | P0 | 0.5 h |
| T6 集成测试（`tests/cli_batch_export.rs`）：构造一个含 paper layout 的最小 DXF 走完整 CLI 路径；`--list-layouts` 端到端 | P0 | 0.4 h |
| T7 `docs/cli.md` 新建（汇总所有 R36-R42 CLI flag 与例）+ CHANGELOG R42-A 章节 | P0 | 0.3 h |
| T8 跑 `cargo test --locked --workspace --all-targets` + `RUSTFLAGS=-Dwarnings cargo check --locked --workspace --all-targets` | P0 | 0.2 h |

**不纳入**：
- 一次 CLI 调用导出多 layout（用 shell 循环 + `--list-layouts` 即可，无需在 binary 里实现 fan-out）
- GUI 集成（GUI 已经有 layout 切换 UI）
- `SvgExportOptions` / `PdfExportOptions` 加 `layout` 字段（CLI flag 已经覆盖；options 字段是 R42-B 可选）
- DWG 输入批处理（`load_file_with_native_blocking` 支持 DWG，但 R36-R37 既定 CLI 只稳定 DXF；DWG 路径在 GUI 里走 acadrust + h7cad-native-dwg fallback，CLI 待 R44+ 解锁）

---

## 3. 设计

### 3.1 `BatchArgs` 扩展

```rust
pub enum BatchArgs {
    Help,
    /// List the available layouts inside one or more input files.
    ListLayouts {
        inputs: Vec<PathBuf>,
    },
    Export {
        format: ExportFormat,
        inputs: Vec<PathBuf>,
        output: ExportTarget,
        options_path: Option<PathBuf>,
        /// R42-A: which layout to export.  `None` defaults to `"Model"`
        /// (matches every pre-R42 invocation byte-for-byte).
        layout: Option<String>,
    },
}
```

### 3.2 `--layout` flag 解析

与 `--options` 完全对称：任何位置出现，紧跟 layout 名（不允许以 `-` 开头）。
若 `--layout` 出现但缺失 value（下一 arg 是 flag 或不存在），返回 `None`
让 main.rs 报错并打 help。

```rust
let layout_flag_idx = args.iter().position(|a| a == "--layout");
let layout_value_idx = layout_flag_idx
    .and_then(|i| args.get(i + 1).map(|v| (i, v)))
    .and_then(|(i, v)| if !v.starts_with('-') { Some(i + 1) } else { None });

let skip_indices: [Option<usize>; 6] = [
    Some(flag_idx),
    output_idx,
    options_flag_idx,
    options_value_idx,
    layout_flag_idx,
    layout_value_idx,
];
```

### 3.3 `--list-layouts` 子命令

最高优先级（仅次于 `--help`），与 `--export-*` 互斥：

```rust
if args.iter().any(|a| a == "--help" || a == "-h") {
    return Some(BatchArgs::Help);
}
if args.iter().any(|a| a == "--list-layouts") {
    let inputs: Vec<PathBuf> = args
        .iter()
        .filter(|s| !s.starts_with('-'))
        .map(PathBuf::from)
        .collect();
    if inputs.is_empty() {
        return None;
    }
    return Some(BatchArgs::ListLayouts { inputs });
}
// ...existing --export-* parsing...
```

### 3.4 `export_one` 行为

```rust
fn export_one(
    input: &Path,
    output: &Path,
    format: ExportFormat,
    options: &LoadedOptions,
    layout: Option<&str>,
) -> Result<(), String> {
    // ... existing load ...
    let mut scene = crate::scene::Scene::new();
    scene.document = compat;
    scene.set_native_doc(native);
    if let Some(name) = layout {
        // Validate against the actual layout list before silently rendering
        // an empty drawing for an unknown name.
        let available = scene.layout_names();
        if !available.iter().any(|n| n == name) {
            return Err(format!(
                "unknown layout \"{}\" in \"{}\".  Available: {}",
                name,
                input.display(),
                available.join(", ")
            ));
        }
        scene.current_layout = name.to_string();
    }
    // ... existing wires + paper_and_offset + export ...
}
```

### 3.5 `run_list_layouts` 实现

```rust
fn run_list_layouts(inputs: &[PathBuf]) -> Result<(), String> {
    let mut had_error = false;
    for (idx, input) in inputs.iter().enumerate() {
        if inputs.len() > 1 {
            if idx > 0 { println!(); }
            println!("=== {} ===", input.display());
        }
        match crate::io::load_file_with_native_blocking(input) {
            Ok((compat, native, _)) => {
                let mut scene = crate::scene::Scene::new();
                scene.document = compat;
                scene.set_native_doc(native);
                for name in scene.layout_names() {
                    println!("{name}");
                }
            }
            Err(e) => {
                had_error = true;
                eprintln!("failed to load \"{}\": {e}", input.display());
            }
        }
    }
    if had_error {
        Err("one or more inputs could not be loaded".into())
    } else {
        Ok(())
    }
}
```

### 3.6 `HELP_TEXT` 扩展

新增段：

```
LAYOUT SELECTION (R42-A):
    --layout NAME        Export a specific layout instead of Model space.
                         NAME is case-sensitive and must match a value
                         returned by --list-layouts.  Default: "Model".
    --list-layouts       Print available layouts for each input (one per
                         line) and exit.  Use this to discover which NAMEs
                         to feed into --layout.

EXAMPLES:
    h7cad drawing.dxf --list-layouts
    h7cad drawing.dxf --export-pdf --layout "Layout1" out.pdf
    for L in $(h7cad drawing.dxf --list-layouts); do
        h7cad drawing.dxf --export-pdf --layout "$L" "out_$L.pdf"
    done
```

---

## 4. 测试

### 4.1 单元测试（`src/cli.rs::tests`）

新增 7 条：

- `parse_recognises_layout_flag` — `--layout Layout1` 出现在 `--export-*` 后
- `parse_layout_flag_can_appear_before_export_flag` — `--layout L --export-pdf out.pdf input.dxf`
- `parse_layout_flag_value_must_not_start_with_dash` — `--layout --export-pdf` 视为 layout 缺失 → 仍可正确解析其余 args（layout = None）
- `parse_recognises_list_layouts_subcommand` — `--list-layouts INPUT.dxf`
- `parse_list_layouts_with_multiple_inputs` — `--list-layouts A.dxf B.dxf`
- `parse_list_layouts_takes_priority_over_export_flag` — 同时存在 `--list-layouts --export-pdf` → ListLayouts 胜（list-layouts 是诊断，导出的输出会有歧义）
- `parse_layout_coexists_with_options_flag` — `--layout L1 --options o.json --export-pdf out.pdf in.dxf`

### 4.2 集成测试（`tests/cli_batch_export.rs`）

新增 3 条：

- `cli_list_layouts_for_minimal_dxf_includes_model` — 用 `make_minimal_dxf()` 构造一个没有 paper layout 的 DXF，断言 stdout 含 "Model"
- `cli_export_pdf_with_unknown_layout_fails_with_diagnostic` — 用同一份 DXF，传 `--layout DoesNotExist` → 退出码 1 + stderr 含 "unknown layout" + "Available:"
- `cli_export_pdf_with_layout_model_succeeds_byte_identical_to_no_flag` — 同份 DXF 导出两次：一次 `--export-pdf out_a.pdf`，一次 `--export-pdf out_b.pdf --layout Model`，两个文件字节级一致（除可能的 PDF metadata timestamp 字段）

### 4.3 既有测试保持不变

R36-R38 的 7 条 cli 单元 + 10 条集成测试全部不动（layout 字段 = None 时行为字节级保持）。

---

## 5. 验收

```bash
cargo check -p H7CAD                                # 零新 warning
cargo test --bin H7CAD cli::                        # 22 → 29 全绿
cargo test --test cli_batch_export                  # 10 → 13 全绿
cargo test --bin H7CAD                              # 484 → 491 全绿
cargo test --locked --workspace --all-targets       # 全绿
RUSTFLAGS=-Dwarnings cargo check --locked --workspace --all-targets  # 零新 warning
```

手动：
- `h7cad.exe drawing.dxf --list-layouts` 应输出 layout 名按行（至少 "Model"）
- `h7cad.exe drawing.dxf --export-pdf --layout WrongName` 应退出 1，stderr 含 `unknown layout "WrongName"` 与可用 layouts 列表

---

## 6. 状态

- [x] 计划定稿（2026-04-25）
- [x] T1 `BatchArgs` 扩展
- [x] T2 parser 接入 `--layout` / `--list-layouts`
- [x] T3 `export_one` set `current_layout` + 未知名报错
- [x] T4 `run_list_layouts` 实现
- [x] T5 单元测试 7 条
- [x] T6 集成测试 3 条
- [x] T7 `docs/cli.md` + CHANGELOG R42-A
- [x] T8 全量验证

---

## 7. 下轮方向

- **R42-B**（可选）：`SvgExportOptions` / `PdfExportOptions` 加 `layout: Option<String>` 字段，让 `--options` JSON 也能 override layout（与 CLI flag 合并优先级：CLI flag > JSON > 默认）
- **R42-C**（可选）：`--list-layouts --json` 输出 JSON 数组，便于脚本 `jq` 处理
- **R43**：OLE 对象 + Rational NURBS + SVG `<pattern>` 真实 hatch
- **R44**：DWG 输入批处理路径（CLI 解锁 DWG 输入）
