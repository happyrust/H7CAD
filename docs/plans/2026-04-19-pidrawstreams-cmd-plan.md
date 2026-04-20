# H7CAD PIDRAWSTREAMS 命令落地计划

> 起稿：2026-04-19  
> 依赖：pid-parse `inspect::unidentified_top_level_streams` 新公共 API（上一轮完成）
>
> **目标**：把上一轮刚开放的"pid-parse 未识别顶层流"API 接到 H7CAD 命令行，作为第一个真实消费者验证 API 设计是否顺手。用户/开发者可在命令行直接看到"当前 PID 还有哪些流 pid-parse 没解码"：
>
> ```
> PIDRAWSTREAMS                 ← active tab cached 包
> PIDRAWSTREAMS <path.pid>      ← 任意磁盘文件
> ```

---

## 用户故事

> 开发场景："我想为 pid-parse 加一个新流的解码器，但不确定样本里有没有这种流。"
> 1. OPEN sample.pid
> 2. `PIDRAWSTREAMS`  
>    → `PIDRAWSTREAMS  0 unidentified top-level stream(s) in <path>`（样本已完整识别）
>
> 或在某些真实文件上：
> 1. `PIDRAWSTREAMS C:/other.pid`  
>    → `PIDRAWSTREAMS  2 unidentified top-level stream(s) in C:\other.pid`  
>    → `    /MysteryA  1024 B  magic=0x12345678 'xxxx'`  
>    → `    /MysteryB  48 B`

## 设计

### H7CAD API 增量

`src/io/pid_import.rs`：
```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnidentifiedStreamInfo {
    pub path: String,
    pub size: u64,
    pub magic_u32_le: Option<u32>,
}

/// List top-level CFB streams in the cached `PidPackage` for `source`
/// that `pid-parse` does not yet recognize. Soft `Result<_, String>` so
/// the command-line layer can surface the cache-miss error verbatim.
pub fn list_pid_unidentified_cached(source: &Path) -> Result<Vec<UnidentifiedStreamInfo>, String>;

/// Parse `path` fresh and list the unidentified top-level streams.
/// Does not consult or mutate the package store.
pub fn list_pid_unidentified_file(path: &Path) -> Result<Vec<UnidentifiedStreamInfo>, String>;
```

两个 helper 的主体：parse / fetch cached → `pid_parse::inspect::unidentified_top_level_streams(&doc)` → map 成 `UnidentifiedStreamInfo`。H7CAD 这层之所以定义自己的 `UnidentifiedStreamInfo` 而非直接返回 `Vec<&StreamEntry>`：命令层常用 `Clone + Eq`，借用到 lib 内部 `StreamEntry` 的生命周期会让命令调度麻烦。

### 命令注册

紧邻 `PIDVERIFY`（命令语义相近：都是"看当前 PID 的整包状态"）。
```rust
cmd if cmd == "PIDRAWSTREAMS" || cmd.starts_with("PIDRAWSTREAMS ") => {
    let arg = cmd.split_once(' ').map(|(_, r)| r.trim()).unwrap_or("").to_string();
    // target = cached 或 explicit path
    // is_pid 检查
    // 调对应 helper
    // 输出：N unidentified + 缩进详情
}
```

输出格式：
```
PIDRAWSTREAMS  0 unidentified top-level stream(s) in <target>
PIDRAWSTREAMS  2 unidentified top-level stream(s) in <target>
    /MysteryA  1024 B  magic=0x12345678 'xxxx'
    /MysteryB  48 B
```

Magic 显示：复用 pid_parse 已导出的 `parsers::magic::magic_tag(u32)` —— 但这个函数是否 pub？需要检查。如果不 pub，就只显示 hex。

### PIDHELP 更新

追加行：
```
PIDRAWSTREAMS [<path>]                list top-level streams not yet decoded by pid-parse
```

### 测试

H7CAD `#[cfg(test)] mod tests` 新增 2 个：
1. `list_pid_unidentified_cached_returns_empty_for_known_streams`：build fixture（含 Drawing/General/Sheet/Blob 四个流）→ load → `list_pid_unidentified_cached` → 断言 Blob 是唯一 unidentified（`/UnknownStorage/Blob` 这个顶层名是 UnknownStorage 前缀，不在 KNOWN_TOP_LEVEL_STORAGE_PREFIXES 里）
2. `list_pid_unidentified_file_works_without_cache`：同 fixture，不 load cache，直接 `list_pid_unidentified_file` 验证

等等：本轮刚加的 `KNOWN_TOP_LEVEL_STORAGE_PREFIXES` 只覆盖 `Sheet` / `TaggedTxtData` / `JSite`；我的 H7CAD fixture 里的 `/UnknownStorage/Blob` 会被列为 "unidentified"（因为 `/UnknownStorage/Blob` 不是顶层流，它是 Blob 位于 UnknownStorage 下）。等等——`unidentified_top_level_streams` 过滤条件是 `!path.contains('/')` 即**只看顶层流**（根下一级的 stream 才会被纳入视野）。`/UnknownStorage/Blob` 包含 `/` 所以不是顶层流，过滤器排除它。

那 H7CAD fixture 的情况是：
- `/TaggedTxtData/Drawing` → 不是顶层（含 `/`）→ 过滤器排除
- `/TaggedTxtData/General` → 同上
- `/PlainSheet/Sheet1` → 不是顶层
- `/UnknownStorage/Blob` → 不是顶层

即：**fixture 根本没有顶层流**，所以 `list_pid_unidentified_*` 总返回空向量。这是 fixture 设计的结果，不是 bug。

为了有意义地测试 "unidentified 被正确列出"，需要 fixture 有**顶层流**。我可以改 `build_fixture_pid_with_unidentified` helper：在 cfb::create 后直接 create_stream 在根目录（`/MysteryTop`），不经过 storage。

新 fixture 函数：
```rust
fn build_fixture_pid_with_toplevel_unknown(path: &Path) {
    ... 正常 build ...
    cfb.create_stream("/MysteryTop").unwrap();  // 根级 stream
}
```

然后测试：
- 调 list_* → 断言返回包含 `/MysteryTop`

### 落地

- `cargo test io::pid_import` 全绿（含新 2 个测试）
- `cargo build` 全绿
- PIDHELP 更新（命令计数从 9 到 10）
- `.memory/2026-04-19.md` 追加段落

## 公共 API 增量

### H7CAD `io::pid_import`
- 新增 `pub struct UnidentifiedStreamInfo { path, size, magic_u32_le }`
- 新增 `pub fn list_pid_unidentified_cached(source) -> Result<Vec<…>, String>`
- 新增 `pub fn list_pid_unidentified_file(path) -> Result<Vec<…>, String>`

### H7CAD `dispatch_command`
- 新增分支 `PIDRAWSTREAMS` / `PIDRAWSTREAMS <path>`
- `PIDHELP` 描述更新

## 不做

1. **每流的 hex preview**：当前只 path + size + magic；想看 hex 用 `pid_inspect` CLI
2. **推导建议 magic tag**：`pid_parse::parsers::magic::magic_tag` 如果 pub 就用；否则只 hex

## 工作量预估

- helpers：15 min
- 命令分支：10 min
- 2 测试：15 min
- 落地：5 min

合计 ~45 min。
