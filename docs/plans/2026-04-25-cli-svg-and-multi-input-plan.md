# CLI `--export-svg` + multi-input batch（三十七轮）

> **起稿**：2026-04-25（第三十七轮）
> **前置**：三十六轮落地 `h7cad INPUT.dxf --export-pdf OUT.pdf` 单文件
> headless PDF 导出。本轮做两个对称扩展：**加 SVG 路径** + **支持一次
> 传多个输入文件**。CI 批处理脚本可以用一条命令导出整包 DXF。
> **目标**：让 `h7cad` 的 CLI 能力覆盖 PDF 和 SVG 两种导出，并且
> multi-input 场景也是一等公民。

---

## 1. 现状

R36 落地后 CLI 支持：

```
h7cad INPUT.dxf --export-pdf [OUTPUT.pdf]
h7cad --help
```

`BatchArgs::ExportPdf { input, output }` 只能绑定一对输入输出，不能
列出多个。SVG 完全没接入（只有 GUI）。

---

## 2. 范围

| 纳入 | 优先级 | 预估 |
|------|-------|------|
| T1 重构 `BatchArgs`：`ExportPdf / ExportSvg` 两变体，各自接 `inputs: Vec<PathBuf>` + `output: ExportTarget` | P0 | 0.3 h |
| T2 `ExportTarget` 枚举：`SameStem`（推导）/ `File(PathBuf)`（单文件 output）/ `Dir(PathBuf)`（输出到目录） | P0 | 0.2 h |
| T3 `parse_batch_args` 解析新语法 + 严格校验（多输入时 output 必须是 dir 或推导） | P0 | 0.5 h |
| T4 `run_batch_export` 循环处理，累积失败计数；任一失败退出码 1 但继续处理剩余 | P0 | 0.4 h |
| T5 `--export-svg` 调 `export_svg_full` 默认 options | P0 | 0.3 h |
| T6 unit tests + 集成测试（covering 单文件 / 多文件 / 目录 output / SVG 路径） | P0 | 0.4 h |
| T7 CHANGELOG + commit + push | P0 | 0.1 h |

**不纳入**：
- `--plot-style` / `--options <json>` 等高级 flag，留给后续轮
- CLI 覆盖 `--export-pdf --export-svg` 同时触发（可以但 scope 爆炸，
  典型用法不这样用）

---

## 3. 新 CLI 语法

```
h7cad INPUT.dxf --export-pdf OUTPUT.pdf         # 单文件 → 单文件
h7cad INPUT.dxf --export-pdf                    # 单文件 → 推导（INPUT.pdf）
h7cad A.dxf B.dxf C.dxf --export-pdf OUTDIR/    # 多文件 → 目录
h7cad A.dxf B.dxf --export-pdf                  # 多文件 → 推导（各自 .pdf）
h7cad INPUT.dxf --export-svg OUTPUT.svg         # SVG 同理
h7cad --help
```

**约束**：当输入 ≥ 2 且 `--export-*` 后跟了一个路径，该路径必须是
已存在的目录（否则报错退出 1）。允许末尾加 `/` 强制识别为目录。

---

## 4. 设计

### 4.1 `BatchArgs` 重构

```rust
pub enum BatchArgs {
    Help,
    Export {
        format: ExportFormat,
        inputs: Vec<PathBuf>,
        output: ExportTarget,
    },
}

pub enum ExportFormat { Pdf, Svg }

pub enum ExportTarget {
    /// Use each input's stem + correct extension in the input's directory.
    SameStem,
    /// Exactly one input → this file path is the output.
    File(PathBuf),
    /// Any number of inputs → output into this directory with inferred stems.
    Dir(PathBuf),
}
```

### 4.2 `parse_batch_args` 流程

1. `--help`/`-h` 最高优先级
2. 识别 `--export-pdf` / `--export-svg`，决定 `format`
3. 跳过 flag arg 和「紧跟 flag 的 output 候选 arg」，其余非 flag args 收集为 `inputs`
4. `inputs.len() == 0` ⇒ `None`（不是 batch 模式）
5. output 候选 arg 处理：
   - 无 → `ExportTarget::SameStem`
   - 存在且指向已有目录（或以 `/`、`\` 结尾）⇒ `Dir(path)`
   - 否则 ⇒ `File(path)`；且当 `inputs.len() > 1` 时返回 parse 错误
     （在 `parse_batch_args` 阶段通过 `Option<Result<BatchArgs, String>>`
     暴露……简化起见先只返回 `Option<BatchArgs>`，把「多输入 + 单文件
     output」的非法性在 `run_batch_export` 运行期 reject）

### 4.3 `run_batch_export` 流程

```rust
match args {
    Help => print help,
    Export { format, inputs, output } => {
        let mut failed = 0;
        for input in &inputs {
            let out_path = resolve_output(input, &output, format, inputs.len());
            match export_one(input, &out_path, format) {
                Ok(()) => eprintln!("h7cad: {input} → {out_path}"),
                Err(e) => { eprintln!("h7cad: {input} failed: {e}"); failed += 1; }
            }
        }
        if failed > 0 { return Err(format!("{failed} of {total} failed")); }
    }
}
```

`export_one` 根据 format 调 `export_pdf_full` 或 `export_svg_full`，都
用 `::default()` options。

### 4.4 `resolve_output`

```rust
fn resolve_output(input: &Path, target: &ExportTarget, format: ExportFormat, total: usize) -> PathBuf {
    match target {
        SameStem => input.with_extension(ext_for(format)),
        File(path) => path.clone(),  // caller should have rejected multi-input here
        Dir(dir) => {
            let stem = input.file_stem().unwrap_or_default();
            dir.join(format!("{}.{}", stem.to_string_lossy(), ext_for(format)))
        }
    }
}
```

---

## 5. 测试

### 单元测试（新增到 `src/cli.rs`）

- `parse_multi_input_with_dir_output`
- `parse_single_input_export_svg_uses_file_target`
- `parse_multi_input_no_output_uses_same_stem`
- `parse_rejects_empty_after_flag_when_only_flags`（edge case）

### 集成测试（`tests/cli_batch_export.rs` 追加）

- `cli_exports_svg_for_minimal_dxf`（--export-svg 路径，验证 output
  包含 `<svg`）
- `cli_batch_two_dxfs_to_dir`（两个 DXF → 一个目录，验证目录里有
  两个 .pdf）
- `cli_mixed_failure_keeps_processing`（故意混一个不存在的输入，
  断言另一个成功且退出码 1）

---

## 6. 验收

```bash
cargo check -p H7CAD                  # 零新 warning
cargo test --bin H7CAD cli::          # 7 → 11 unit tests 全绿
cargo test --test cli_batch_export    # 4 → 7 integration 全绿
cargo build -p H7CAD --release        # binary 可运行
```

手动：
```powershell
h7cad.exe a.dxf b.dxf --export-pdf out_dir\
h7cad.exe a.dxf --export-svg a.svg
```

---

## 7. 状态

- [x] 计划定稿（2026-04-25）
- [x] T1-T3 重构 BatchArgs / parse（`ExportFormat` + `ExportTarget` + `parse_batch_args` 支持 `--export-svg` 与多输入）
- [x] T4 run_batch_export 循环（多输入累计失败 + `resolve_output` 三模式分发）
- [x] T5 SVG 路径接入（`export_svg_full` + `SvgExportOptions::default()`）
- [x] T6 测试（unit 13 条 + 集成 7 条，`cargo test --bin H7CAD` 406→412 全绿）
- [x] T7 CHANGELOG（commit/push 留待单独一轮）
