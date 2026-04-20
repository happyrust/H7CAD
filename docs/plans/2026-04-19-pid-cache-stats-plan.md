# 开发计划：`pid_package_store` 观察性增强 + `PIDCACHESTATS` 命令

> 起稿：2026-04-19  
> 背景：H7CAD × SPPID 集成分析的改进点 3 ("pid_package_store 无 LRU 上限")。本轮不实现完整 LRU eviction policy（需要设计 hit / recency 策略），先做**可观察性子集**：让用户/调试者能在运行时看到缓存占用，为将来 eviction 决策铺路。

## 动机

当前 `pid_package_store` 是进程级 `Mutex<HashMap<PathBuf, Arc<PidPackage>>>`，永不自动 evict。长期交互打开大量 `.pid` 文件的场景下缓存会持续增长。没有任何一个运行时接口能让用户看到"目前缓存了多少包、占用多少字节"，导致：

- 用户排障时不知道是 H7CAD 吃内存还是别处泄漏
- 开发者做 eviction 决策时缺基线数据
- CI 的 PID 测试无法断言"测后被清理"（单测现在用 `clear_package` 逐个清，但没有全局断言）

## 目标

1. `pid_package_store` 新增 3 个公开 API：
   - `cache_stats() -> PidPackageCacheStats { entry_count, total_stream_bytes }`
   - `cached_paths() -> Vec<PathBuf>`
   - `PidPackageCacheStats` 结构（`Copy + Default + Debug`）
2. H7CAD 命令 `PIDCACHESTATS`：显示条目数、各条目 path + stream 数 + 字节数、总字节数
3. `PIDHELP` 扩充，加入新命令描述

## 非目标

- 不实现 LRU / 容量上限 / 自动 eviction
- 不改 `cache_package` / `get_package` / `clear_package` 签名
- 不引入 `sysinfo` / 实际进程内存采样（太重）
- 不加"缓存已满"事件/回调

## 关键设计

### API 形状

```rust
#[derive(Debug, Clone, Copy, Default)]
pub struct PidPackageCacheStats {
    pub entry_count: usize,
    /// Sum of `RawStream.data.len()` across all cached packages.
    /// Doesn't count Arc overhead, BTreeMap keys, or struct padding —
    /// meant as an **order-of-magnitude** signal, not precise mem usage.
    pub total_stream_bytes: u64,
}

pub fn cache_stats() -> PidPackageCacheStats { /* ... */ }
pub fn cached_paths() -> Vec<PathBuf> { /* ... */ }
```

两个函数短暂持 Mutex（`lock` 时间 O(总 stream 数)），不在热路径调用。

### 命令 UX

```
> PIDCACHESTATS
PIDCACHESTATS  3 entry/entries, 2.35 MB total stream bytes
    [  0]  C:/project/DWG-0201.pid     52 streams   824,320 B
    [  1]  C:/project/DWG-0202.pid     54 streams   857,600 B
    [  2]  C:/temp/scratch.pid          3 streams       128 B
```

排序：cached_paths 返回的 `Vec<PathBuf>` 保证**字典序**（HashMap 本身无序，需手动 sort 保证测试稳定），输出里带序号方便用户引用。

### 字节计量语义（保守）

- 每个 RawStream 只算 `.data.len()`
- 不算：`RawStream.path` / Arc 头 / HashMap bucket overhead / struct padding
- 精度：**±5% 级**——足以识别"50MB vs 50GB"量级，不足以逼近 OS 报出的 RSS
- 避免用 `std::mem::size_of_val` + transitive 遍历，复杂度 / 可预测性不值

## 实施步骤

### M1 — store 新 API（10 min）

在 `src/io/pid_package_store.rs` 追加：

```rust
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PidPackageCacheStats {
    pub entry_count: usize,
    pub total_stream_bytes: u64,
}

pub fn cache_stats() -> PidPackageCacheStats {
    let guard = store().lock().expect("pid_package_store mutex poisoned");
    let mut stats = PidPackageCacheStats::default();
    stats.entry_count = guard.len();
    for pkg in guard.values() {
        for stream in pkg.streams.values() {
            stats.total_stream_bytes = stats
                .total_stream_bytes
                .saturating_add(stream.data.len() as u64);
        }
    }
    stats
}

pub fn cached_paths() -> Vec<PathBuf> {
    let guard = store().lock().expect("pid_package_store mutex poisoned");
    let mut paths: Vec<PathBuf> = guard.keys().cloned().collect();
    paths.sort();
    paths
}

/// Per-entry summary used by the `PIDCACHESTATS` command renderer.
/// Separate from `cache_stats()` (aggregate only) to avoid holding the
/// mutex longer when the caller only wants a one-line summary.
#[derive(Debug, Clone)]
pub struct PidPackageCacheEntrySummary {
    pub path: PathBuf,
    pub stream_count: usize,
    pub stream_bytes: u64,
}

pub fn cached_entry_summaries() -> Vec<PidPackageCacheEntrySummary> {
    let guard = store().lock().expect("pid_package_store mutex poisoned");
    let mut entries: Vec<_> = guard
        .iter()
        .map(|(path, pkg)| {
            let stream_bytes: u64 = pkg
                .streams
                .values()
                .map(|s| s.data.len() as u64)
                .fold(0u64, |a, b| a.saturating_add(b));
            PidPackageCacheEntrySummary {
                path: path.clone(),
                stream_count: pkg.streams.len(),
                stream_bytes,
            }
        })
        .collect();
    entries.sort_by(|a, b| a.path.cmp(&b.path));
    entries
}
```

### M2 — store 单测（10 min）

追加 3 条：

- `cache_stats_starts_empty_and_reflects_inserts`：先 `clear_all_for_test`（新增 test-only helper）→ stats = (0, 0)；insert 1 包 → (1, N 字节)；insert 2 包 → (2, 2N 字节)；clear → 减回
- `cached_paths_returns_lexicographic_order`：插 3 个路径乱序 → 返回顺序字典递增
- `cached_entry_summaries_counts_streams_and_bytes`：插 1 包 5 stream 总 1000 字节 → summary 报 (5, 1000)

但 **注意**：测试共享 OnceLock 静态 store，并行测试下 `cache_stats()` 会看到其他测试的条目。解决：测试前记录 baseline，断言 **delta** 而非绝对值；或者用 `unique_path("stats-xxx")` 确保 path 独特，并精确清理自己插入的条目后断言 delta 为 0。后者更清晰。

### M3 — PIDCACHESTATS 命令（20 min）

在 `src/app/commands.rs` 的 PID 命令 match 区加：

```rust
cmd if cmd == "PIDCACHESTATS" => {
    let stats = pid_package_store::cache_stats();
    self.command_line.push_output(&format!(
        "PIDCACHESTATS  {} entry/entries, {} total stream bytes ({:.2} MB)",
        stats.entry_count,
        stats.total_stream_bytes,
        stats.total_stream_bytes as f64 / 1_048_576.0,
    ));
    for (idx, entry) in pid_package_store::cached_entry_summaries().into_iter().enumerate() {
        self.command_line.push_info(&format!(
            "    [{idx:>3}]  {}  {} stream(s)  {} B",
            entry.path.display(),
            entry.stream_count,
            entry.stream_bytes,
        ));
    }
}
```

### M4 — PIDHELP 扩充（5 min）

在 `PIDHELP` 的 Integrity / Report 段之间插入：

```rust
self.command_line.push_info("    Observability:");
self.command_line.push_info(
    "        PIDCACHESTATS                        show in-memory PidPackage cache occupancy",
);
```

从 18 → 19 命令。

### M5 — CHANGELOG + 编译 + test + commit + push（10 min）

1. `cargo check --bin H7CAD`
2. `cargo test --bin H7CAD io::pid_package_store` （应有 4+3=7 单测，全绿）
3. `cargo test --bin H7CAD io::pid_import` （零回归 65/65）
4. CHANGELOG.md Unreleased 追加中文条目
5. commit + push

## 预计工时

| 步骤 | 估时 |
|---|---|
| 写 plan | 完成 |
| M1 | 10 min |
| M2 | 10 min |
| M3 | 20 min |
| M4 | 5 min |
| M5 | 10 min |
| **合计** | **~55 min** |

## 风险与缓解

| 风险 | 缓解 |
|---|---|
| 共享静态 store 让并行单测互相看到对方条目 | `cache_stats` 断言**delta**而非绝对值；测试末尾 `clear_package` 清理自己 |
| `cache_stats()` / `cached_entry_summaries()` 持 Mutex 时间随包数量线性增长 | 只遍历 stream.data.len()，不复制 bytes；10 个 100-stream 包 ≈ 1000 次加法，μs 级 |
| 未来实现 LRU 时要改 store 结构 | 新 API 只读，不假设内部存储形态；迁移到 `LruCache` 时语义等价替换即可 |

## 回滚

2 个文件改动（`pid_package_store.rs` + `commands.rs` 加 match arm + `CHANGELOG.md`）。`git revert` 即可。无 API 破坏（纯增量）。

## Next 排队

- 真 LRU：在 store 里加 `MAX_ENTRIES` 常量 + `Mutex<LruCache>` 或自维护 access 时间戳
- `PIDCACHECLEAR <path>` 命令：手动 evict
- `PIDCACHECLEARALL` 命令：清空整个缓存
- 与 pid-parse `pid_inspect --round-trip --verify` 的 telemetry 对齐
