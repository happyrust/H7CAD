# CLI `--options <PATH>` JSON flag（三十八轮）

> **起稿**：2026-04-25（第三十八轮）
> **前置**：三十六轮 CLI `--export-pdf`、三十七轮 `--export-svg` +
> 多输入批处理。两轮都只支持默认 `*ExportOptions`，无法从命令行调整
> monochrome / font / include_hatches 等 GUI 对话框已经暴露的字段。
> **目标**：新增 `--options <path.json>` flag，让 CI / 脚本可以完整
> 复现 GUI 对话框里的任何 options 组合，无须改动 UI。

---

## 1. 现状

R37 落地后 `export_one` 只用默认值：

```rust
ExportFormat::Pdf => {
    let options = PdfExportOptions::default();
    export_pdf_full(&wires, &scene.hatches, ..., &options)?;
}
ExportFormat::Svg => {
    let options = SvgExportOptions::default();
    export_svg_full(&wires, &scene.hatches, ..., &options)?;
}
```

GUI 侧 `PdfExportDialog` / `SvgExportDialog` 可以调 ~11 个布尔/枚举
字段，CLI 侧完全没接入——自动化批处理如果想出黑白图、关掉 hatch
pattern、换字体，目前只能手工改源码重编。

---

## 2. 范围

| 纳入 | 优先级 | 预估 |
|------|-------|------|
| T1 Cargo.toml 添加 `serde = { version = "1", features = ["derive"] }` + `serde_json = "1"` | P0 | 0.1 h |
| T2 `PdfExportOptions` + `PdfFontChoice` + `SvgExportOptions` 添加 `#[derive(Deserialize)]` + struct-level `#[serde(default)]` | P0 | 0.3 h |
| T3 CLI parse：识别 `--options <PATH>`，加入 `BatchArgs::Export { ..., options_path: Option<PathBuf> }` | P0 | 0.3 h |
| T4 `export_one` 读 JSON 并按 format dispatch 到对应 `Options` deserializer | P0 | 0.4 h |
| T5 错误处理：JSON 语法错 / 路径不存在 / 字段类型错都给可读 stderr | P0 | 0.3 h |
| T6 unit + 集成测试（4-5 条，覆盖 parse、读取、override 生效、坏 JSON） | P0 | 0.5 h |
| T7 CHANGELOG + plan 状态 + commit | P0 | 0.2 h |

**不纳入**：
- **多 JSON 合并 / 继承链**（`--options default.json --options override.json`）
  ——典型自动化一个 JSON 就够，延到下一轮视需求
- **CLI flag 单字段覆盖**（`--monochrome=false`）——JSON 已经覆盖全量，
  flag 与 JSON 的优先级协商 scope 爆炸，不做
- **Schema validation**（`jsonschema` 等）——依赖 serde 错误即可，
  `serde_json::Error` 已经携带行/列号
- **PDF + SVG 同时 override**——一次调用只能一个 format

---

## 3. 新 CLI 语法

```
h7cad INPUT.dxf --export-pdf OUTPUT.pdf --options opts.json
h7cad A.dxf B.dxf --export-svg OUTDIR\   --options opts.json
```

`--options` 可以出现在任何位置，flag arg 即路径。多输入场景下所有
输入共用同一份 options（这是常见用法；想每个输入不同 options 的
场景用 shell 循环解决）。

---

## 4. 设计

### 4.1 Cargo.toml

```toml
[dependencies]
# ... existing ...
serde = { version = "1", features = ["derive"] }
serde_json = "1"
```

两个 crate 都已经在 Cargo.lock 里（pid-parse / 多个 vendor_tmp 深依
赖），加到 top-level 只是显式化访问权。编译时 cost 几乎为零。

### 4.2 Option structs

```rust
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(default)]
pub struct PdfExportOptions {
    pub monochrome: bool,
    pub text_as_geometry: bool,
    pub font_family: PdfFontChoice,
    pub font_size_scale: f32,
    pub include_hatches: bool,
    pub hatch_patterns: bool,
    pub include_images: bool,
    pub embed_images: bool,
    pub image_base: Option<PathBuf>,
    pub native_dimension_text: bool,
    pub native_curves: bool,
    pub native_splines: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
pub enum PdfFontChoice { Helvetica, TimesRoman, Courier }
```

`Default` impl 保持不变；`#[serde(default)]` on struct 让未出现的
字段落到 struct-level 的 `Default`——效果等价于 per-field default
但 DRY。`PdfFontChoice` enum 默认走 serde 的 external-tagged
representation（`"TimesRoman"` 字符串即可），与 GUI pill 文字一致。

`SvgExportOptions` 同处理。注意 `font_family` 在 SVG 侧是 `String`
（不是 enum），直接 `Deserialize` 即可。

### 4.3 CLI parse

`BatchArgs::Export` 加第四字段：

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

`parse_batch_args` 循环时 `--options` flag 的下一个 arg 记录为
`options_path`（与 output arg 的「紧跟 flag」风格一致）；并把 flag
+ value 的 index 从 inputs 采集里排除。

### 4.4 `export_one` dispatch

```rust
fn export_one(input, output, format, options_path: Option<&Path>) -> Result<(), String> {
    // load DXF → scene 同 R37 ...
    match format {
        Pdf => {
            let options = load_pdf_options(options_path)?;
            export_pdf_full(&wires, ..., &options)?;
        }
        Svg => {
            let options = load_svg_options(options_path)?;
            export_svg_full(&wires, ..., &options)?;
        }
    }
}

fn load_pdf_options(path: Option<&Path>) -> Result<PdfExportOptions, String> {
    match path {
        None => Ok(PdfExportOptions::default()),
        Some(p) => {
            let bytes = std::fs::read(p).map_err(|e|
                format!("cannot open options file \"{}\": {e}", p.display()))?;
            serde_json::from_slice(&bytes).map_err(|e|
                format!("invalid JSON in options file \"{}\": {e}", p.display()))
        }
    }
}
```

SVG 路径对称。

### 4.5 Help text 扩展

`HELP_TEXT` 新增段落：

```
OPTIONS FILE (optional):
    --options <PATH>     JSON file overriding any PdfExportOptions /
                         SvgExportOptions field.  Missing fields fall
                         back to the built-in default.  See docs for
                         the full schema.
```

---

## 5. 测试

### 5.1 单元（`src/cli.rs`）

- `parse_recognises_options_flag`：`--options opts.json` 被 parse 成
  `options_path = Some("opts.json")`
- `parse_options_flag_coexists_with_multi_input_dir`：多输入 + dir
  output + options 三者共存
- `parse_without_options_flag_has_none`：老调用不受影响，`options_path: None`

### 5.2 集成（`tests/cli_batch_export.rs`）

- `cli_export_pdf_with_options_json_monochrome_false`：写一份 JSON
  `{"monochrome": false}`，和默认 monochrome=true 比较输出字节不同
  （足够证明 options 确实被 apply；更精确的像素比较留给 Phase 4）
- `cli_export_with_bogus_options_exits_one`：指向不存在的 JSON 文件
  → 退出码 1 + stderr 含 "cannot open"
- `cli_export_with_malformed_options_exits_one`：提供 `{not valid json`
  → 退出码 1 + stderr 含 "invalid JSON"

### 5.3 Options struct roundtrip（新增 `src/io/pdf_export.rs` / `src/io/svg_export.rs`）

- `pdf_options_deserialize_partial_keeps_defaults`：只给
  `{"monochrome": false}`，其他字段等于 `PdfExportOptions::default()`
  各字段
- `pdf_options_deserialize_font_family_enum`：`{"font_family": "TimesRoman"}`
  得到 `PdfFontChoice::TimesRoman`
- `svg_options_deserialize_partial_keeps_defaults`：对称

---

## 6. 验收

```bash
cargo check -p H7CAD                  # 零新 warning
cargo test --bin H7CAD cli::          # 13 → 16 unit tests 全绿
cargo test --bin H7CAD io::pdf_export # 既有 fixture + 2 新 options 测试
cargo test --bin H7CAD io::svg_export # 既有 fixture + 1 新 options 测试
cargo test --test cli_batch_export    # 7 → 10 integration 全绿
cargo test --bin H7CAD                # 412 → 418+ 总数全绿
```

手动：
```powershell
echo '{"monochrome": false, "font_family": "TimesRoman"}' > opts.json
h7cad.exe a.dxf --export-pdf a.pdf --options opts.json
# 打开 a.pdf 确认：颜色保留 / 字体变 Times
```

---

## 7. 状态

- [x] 计划定稿（2026-04-25）
- [x] T1 Cargo.toml 加 serde / serde_json
- [x] T2 Options struct 加 Deserialize + `#[serde(default)]`
- [x] T3 CLI parse 扩展 `--options`（顺序无关；`BatchArgs::Export.options_path` 字段）
- [x] T4 `LoadedOptions` batch-level 预加载 + dispatch
- [x] T5 错误处理：`cannot open options file "…": …` / `invalid JSON in options file "…": …`
- [x] T6 测试（cli::tests 13→22 unit，cli_batch_export 7→10 integration；
  全量 bin 412→421 全绿；roundtrip 通过 unit 侧的 partial-override 测试覆盖）
- [x] T7 CHANGELOG（commit 随本轮一起落地）
