# H7CAD PIDVERIFY 命令落地计划

> 起稿：2026-04-19  
> 依赖：H7CAD `pid_package_store`、pid-parse `PidWriter::write_to` / `parse_package`（同日早些时候完成）
>
> **目标**：让用户在 H7CAD 命令行能直接验证 PID 文件 round-trip 保真——把"保存出去能被 SmartPlant 读"从"另开 CMD 跑 pid_writer_validate"简化为"在 H7CAD 里输 `PIDVERIFY` 一下"。
>
> ```
> OPEN drawing.pid
> PIDSETPROP SP_REVISION 2
> PIDVERIFY                       ← 立即验证：当前 cached 包写出后能完整 round-trip
> SAVEAS drawing-rev2.pid
> PIDVERIFY drawing-rev2.pid      ← 也能验证别处文件
> ```

---

## 现状盘点

* `pid_writer_validate` CLI 已能在命令行外验证；但 H7CAD 用户场景"改了几次后想确认下还能存"必须切窗口
* H7CAD 已有 `pid_package_store::get_package(source)`、`pid_parse::PidParser::parse_package`、`pid_parse::writer::PidWriter::write_to`
* 缺：H7CAD 端的薄包装命令

## 用户故事

> 1. `OPEN drawing.pid`
> 2. `PIDSETPROP SP_REVISION 2`
> 3. `PIDVERIFY`  
>    → `PIDVERIFY  PASS  4 streams matched (Drawing 105 B, General 133 B, Sheet1 16 B, Blob 32 B)`
> 4. 如果 mismatch：`PIDVERIFY  FAIL  1 mismatch in /TaggedTxtData/Drawing (source=105 B roundtrip=42 B first diff @ 8)`

## 设计

### 命令语法

```
PIDVERIFY              ← 验证当前 active tab 的 cached PidPackage
PIDVERIFY <path.pid>   ← 验证给定路径的 .pid 文件（不依赖 cache）
```

### H7CAD 实现

`src/io/pid_import.rs` 加：
```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PidVerifyMismatch {
    pub path: String,
    pub source_len: usize,
    pub roundtrip_len: usize,
    pub first_diff_offset: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PidVerifyReport {
    pub stream_count: usize,
    pub matched: usize,
    pub mismatches: Vec<PidVerifyMismatch>,
    pub only_in_source: Vec<String>,
    pub only_in_roundtrip: Vec<String>,
}

impl PidVerifyReport {
    pub fn ok(&self) -> bool;
}

/// Verify the cached `PidPackage` for `source` round-trips byte-level
/// through `PidWriter`. Writes to a temp file (cleaned up on the way
/// out) and re-parses for comparison.
pub fn verify_pid_cached(source: &Path) -> Result<PidVerifyReport, String>;

/// Verify an arbitrary `.pid` file on disk: parse → write to temp →
/// re-parse → compare. Does not touch the package store.
pub fn verify_pid_file(path: &Path) -> Result<PidVerifyReport, String>;
```

内部公共函数：
```rust
fn compare_streams(original: &PidPackage, roundtrip: &PidPackage) -> PidVerifyReport;
```

### dispatch_command

`src/app/commands.rs`，紧邻 `PIDLISTPROPS`：
```rust
cmd if cmd == "PIDVERIFY" || cmd.starts_with("PIDVERIFY ") => {
    // 解析参数：无 → cached path；有 → 显式 path
    // 校验 .pid 后缀
    // 调用合适 helper
    // 输出 PASS/FAIL + 关键统计
}
```

输出格式（无参数版）：
```
PIDVERIFY  PASS  4 streams matched in cached package <path>
PIDVERIFY  FAIL  1 mismatch in cached package <path>
    /TaggedTxtData/Drawing  source=105 B  roundtrip=42 B  first diff @ 8
```

显式 path 版同形，提示词改 "in <path>"。

### 测试

H7CAD `src/io/pid_import.rs::tests` 新增 3 个：
1. `verify_pid_cached_passes_for_unmodified_fixture`：build fixture → load → verify_pid_cached → PASS + matched=4
2. `verify_pid_cached_passes_after_metadata_edit`：build → load → edit_pid_drawing_attribute → verify → PASS（即 edit 后 cached 包仍能 round-trip 自身字节）
3. `verify_pid_file_passes_for_synthetic_fixture`：build → verify_pid_file（不经过 cache） → PASS

不为 mismatch 路径写测试：现实路径下 mismatch 应该不会发生（writer 是确定性的）；mismatch 报告路径已被 pid_writer_validate 测试覆盖。

### 落地

- `cargo test io::pid_import` 全绿
- `cargo build` 全绿
- 不引入新依赖（temp file 用 `std::env::temp_dir() + 唯一名 + 用完 remove`）
- `.memory/2026-04-19.md` 追加段落

## 不做

1. **PIDVERIFY <path> 含 metadata edit 模拟**：交给 `pid_writer_validate --edit`
2. **PIDSAVEAS --verify 内联**：第一版用户手动 `SAVEAS X` + `PIDVERIFY X` 即可，串两步比塞一个 flag 易读
3. **写后自动 verify** flag：等用户反馈再考虑
4. **多文件批量** verify：交给 `pid_writer_validate <dir>` 未来工作

## 工作量预估

- helpers + compare_streams：25 min
- 命令分支：10 min
- 3 测试：15 min
- 落地：5 min

合计 ~55 min。
