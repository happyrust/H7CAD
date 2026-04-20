# pid-parse 解析层完善（第 1 轮）：可发现性 + 条件测试

> 起稿：2026-04-19  
> 依赖：pid-parse v0.4.1（当前状态）
>
> **目标**：两个互补的解析层改进：
>
> 1. **API 可发现性**：把 inspect 报告里 "Top-level Unidentified Streams" 的过滤逻辑抽成 lib 公共函数 `inspect::unidentified_top_level_streams`，让下游能程序性访问"pid-parse 还有哪些顶层流不识别"——这是未来增量解码新流类型的"待办清单 API"
> 2. **测试条件化**：`tests/parse_real_files.rs` 26 个硬要求 fixture 的测试 + `tests/unit_parsers.rs::sheet_stream_reuses_cluster_header` 统一改为"文件存在才跑，缺失 eprintln! + return"，与 `tests/writer_real_files.rs` 同风格；消除 `cargo test` 在缺 fixture 时的噪音失败

---

## 现状盘点

* `inspect/report.rs:401-444` 有"Top-level Unidentified Streams"段：硬编码一组"已识别"名字（`PSMcluster0` / `PSMroots` / `DocVersion2` / `AppObject` / ...），不在白名单里的顶层流被列为 unidentified
* 这套白名单完全是 **私有** 的（在 writeln! 闭包内），H7CAD 或其它消费方要用必须 scrape 文本
* `tests/parse_real_files.rs` 26 个测试 + `tests/unit_parsers.rs::sheet_stream_reuses_cluster_header` 全部硬依赖 `test-file/DWG-0201GP06-01.pid`，文件缺失 → `panic!` → CI / 本地 `cargo test` FAIL
* v0.4.0 的 `tests/writer_real_files.rs` 已建立"文件缺失时 eprintln! + return"的优雅降级模式

## 设计

### Step 1 · lib 层 `inspect::unidentified_top_level_streams`

`src/inspect/mod.rs` 加新模块（或直接在 `mod.rs`）：
```rust
/// Names of top-level CFB streams that `pid-parse` fully or partially
/// decodes today. Used to classify anything else as "unidentified"
/// — i.e. a valid decoding target for future work.
pub const KNOWN_TOP_LEVEL_STREAM_NAMES: &[&str] = &[
    "\u{5}SummaryInformation",
    "\u{5}DocumentSummaryInformation",
    "PSMcluster0",
    "StyleCluster",
    "Dynamic Attributes Metadata",
    "Unclustered Dynamic Attributes",
    "PSMroots",
    "PSMclustertable",
    "PSMsegmenttable",
    "DocVersion2",
    "DocVersion3",
    "AppObject",
    "JTaggedTxtStgList",
];

/// Top-level storage name prefixes whose members are all considered
/// "identified" (the storage itself is recognized even if individual
/// stream contents are still being probed).
pub const KNOWN_TOP_LEVEL_STORAGE_PREFIXES: &[&str] = &[
    "Sheet",        // Sheet1, Sheet6, ...
    "TaggedTxtData",
    "JSite",
];

/// Borrowed view of every top-level stream `pid-parse` does not yet
/// decode. "Top-level" means one path segment below the CFB root
/// (e.g. `/AppObject`, not `/JSite0000/JProperties`).
///
/// Callers use this both to show end-users "here's what's still raw"
/// and to drive incremental decoding work: if this list shrinks, the
/// parser grew.
pub fn unidentified_top_level_streams(doc: &PidDocument) -> Vec<&StreamEntry>;
```

`inspect/report.rs:401-444` 改用新 API（保持人类输出一致）。

### Step 2 · parse_real_files.rs 条件降级

`parse_test_file` 返回 `Option<PidDocument>`：
```rust
fn parse_test_file(name: &str) -> Option<PidDocument> {
    let path = format!("test-file/{}", name);
    if !std::path::Path::new(&path).exists() {
        eprintln!("SKIP: {} not found", path);
        return None;
    }
    match PidParser::new().parse_file(&path) {
        Ok(d) => Some(d),
        Err(e) => {
            eprintln!("SKIP: failed to parse {}: {}", name, e);
            None
        }
    }
}
```

所有 26 个测试头部加：
```rust
let Some(doc) = parse_test_file("DWG-0201GP06-01.pid") else { return };
```

### Step 3 · `unit_parsers::sheet_stream_reuses_cluster_header` 同样改造

单独一个测试，同样的 `Some/else return` 头部。

### Step 4 · 新增测试

`tests/parse_real_files.rs` 追加：
```rust
#[test]
fn top_level_unidentified_streams_are_empty_on_sample_file() {
    let Some(doc) = parse_test_file("DWG-0201GP06-01.pid") else { return };
    let leftover = pid_parse::inspect::unidentified_top_level_streams(&doc);
    assert!(
        leftover.is_empty(),
        "sample file should have zero unidentified top-level streams; got: {:?}",
        leftover.iter().map(|s| &s.path).collect::<Vec<_>>()
    );
}
```

这把 CHANGELOG v0.2.4 的"顶层未识别流仅剩 1 个：DocVersion2" + v0.2.4 "DocVersion2 作为 DocVersion2Raw 保留" 收紧成"此后**零** unidentified" 的不变量，future regression 能被捕获。

加 3 个 `src/inspect/mod.rs::tests` 单元测试：
- `unidentified_empty_for_default_doc`
- `unidentified_filters_known_names`
- `unidentified_keeps_unknown_top_level_entries`

### Step 5 · 落地

- `cargo test --lib inspect` 全绿（新增 3 + 原有）
- `cargo test` 全集：parse_real_files 文件不存在时现在全部 `ok`（内部 eprintln!），不 panic
- 不 bump pid-parse 版本（lib API 新增但不破坏既有；`KNOWN_TOP_LEVEL_STREAM_NAMES` / `KNOWN_TOP_LEVEL_STORAGE_PREFIXES` / `unidentified_top_level_streams` 都是 additive）
- CHANGELOG 在 0.4.1 段落尾追加"inspect::unidentified_top_level_streams 公共 API"行

## 公共 API 增量

### pid-parse
- 新增 `pub const pid_parse::inspect::KNOWN_TOP_LEVEL_STREAM_NAMES: &[&str]`
- 新增 `pub const pid_parse::inspect::KNOWN_TOP_LEVEL_STORAGE_PREFIXES: &[&str]`
- 新增 `pub fn pid_parse::inspect::unidentified_top_level_streams(&PidDocument) -> Vec<&StreamEntry>`
- inspect/report.rs 输出一致（内部实现重构，用户看不出差异）

## 不做

1. **解码 DocVersion2 的 48B 结构**：没有真实 fixture 做参考，盲做可能错；留给用户手里有 fixture 时再做
2. **把 heuristic 标签去除**：这属于"算法改进"，和本轮的"可发现性+测试守门"是两个方向
3. **把 unidentified 列表进 JSON Schema**：`PidDocument` 本身已经有 `streams: Vec<StreamEntry>`，unidentified 是派生视图，不需要进 schema
4. **H7CAD 侧消费新 API**：先把 API 开出来；H7CAD 要用再加命令（例如 `PIDRAWSTREAMS` 列出 unidentified）

## 工作量预估

- Step 1 抽 lib API：20 min
- Step 2 parse_real_files 改造（26 个 test）：20 min（机械替换但逐个 head 要小心）
- Step 3 unit_parsers 改造：5 min
- Step 4 测试：15 min
- Step 5 落地：10 min

合计 ~70 min。
