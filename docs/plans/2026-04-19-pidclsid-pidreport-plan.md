# H7CAD PIDCLSID + PIDREPORT 命令落地计划

> 日期：2026-04-19  
> 依赖：pid-parse `origin/main @ 51e7a28`（v0.3.12），含 `PidPackage.root_clsid` +
> `PidPackage.storage_clsids`（非空 CLSID 映射）字段
>
> **目标**：给 H7CAD 加两条互补命令：
> - `PIDCLSID` — 消费远程新 CLSID 字段，诊断输出 root + non-root storage CLSID
> - `PIDREPORT` — 聚合已有命令，一条命令出"PID 健康体检"报告
>
> 命令族扩到 **18 个**（含 PIDHELP）。

---

## 设计

### PIDCLSID

显示当前 cached PID 的 root_clsid 和非默认 storage_clsids 列表，用于：
- 与 SmartPlant 原生模板文件对比 CLSID 是否匹配（诊断"为什么 SmartPlant 打不开"）
- 在 PIDDIFF 辅助手段：看到 CLSID 差异时定位具体出自哪个 storage

输出：
```
PIDCLSID  root={12345678-...-...-...}  6 non-root storages with CLSID
    /JSite0    {abcd1234-...-...}
    /JSite1    {abcd1234-...-...}
    ...
```

如果 `root_clsid.is_none()` → `(none)`；如果 `storage_clsids.is_empty()` → "0 non-root storages"。

### PIDREPORT

一条命令跑完：基本信息 + 图统计 + 未识别流 + 版本历史 + 关键 metadata + round-trip
验证，全部一次性输出。用户打开 PID 后先输 `PIDREPORT`，5 秒内看完整体状态。

输出（分段）：
```
=== PID Health Report: <path> ===

[Basic]
  Streams:         69
  Object graph:    yes (82 objects, 64 relationships)
  CLSID root:      {...}
  DocVersion2:     decoded, 4 version records

[Metadata]
  SP_DRAWINGNUMBER: DWG-0201GP06-01
  SP_PROJECTNUMBER: SQLPlant1401
  SP_REVISION:      0
  (+ 剩余顶层 SP_ 属性)

[Graph]
  Objects:         82
  Relationships:   64 (40 fully / 23 partially / 1 unresolved)

[Integrity]
  Round-trip:      PASS  69 streams matched
  Unidentified:    0 top-level streams

[Version history]
  [1] SaveAs v144
  [2] Save   v77
  [3] Save   v144
```

### H7CAD 端 API 增量

`src/io/pid_import.rs`：
```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PidClsidInfo {
    pub root_clsid: Option<String>,           // hex string or None
    pub non_root: Vec<(String, String)>,      // (path, clsid_hex)
}

pub fn read_pid_clsid(source: &Path) -> Result<PidClsidInfo, String>;

/// Aggregate report结构 — 命令层自己排版输出。
#[derive(Debug, Clone)]
pub struct PidHealthReport {
    pub stream_count: usize,
    pub graph_stats: Option<PidGraphStats>,        // None if no object_graph
    pub drawing_attrs: Vec<(String, String)>,      // listing.drawing_attributes (sorted)
    pub general_elements: Vec<(String, String)>,
    pub verify_ok: bool,
    pub verify_matched: usize,
    pub verify_total: usize,
    pub unidentified: Vec<UnidentifiedStreamInfo>,
    pub version_log: Option<PidVersionLog>,
    pub root_clsid: Option<String>,
}

pub fn build_pid_health_report(source: &Path) -> Result<PidHealthReport, String>;
```

`build_pid_health_report` 内部组合已有 helpers：
- `pid_graph_stats` (可选)
- `list_pid_metadata` → drawing/general pairs
- `verify_pid_cached`
- `list_pid_unidentified_cached`
- `list_pid_versions`
- 新 `read_pid_clsid` 的 root_clsid
- `arc.streams.len()` 直接拿

全部失败宽容：子模块错误不阻断 report 生成（graph_stats=None / version_log=None 等），命令层按 None 分支展示"(not available)"。

### 命令注册

- `PIDCLSID` 紧邻 `PIDVERSION`
- `PIDREPORT` 紧邻 `PIDHELP`（作为头牌命令）

### PIDHELP 扩充

新增两行：
```
Integrity:
    PIDCLSID                  CLSID diagnostic: root + non-root storages
Report:
    PIDREPORT                 one-shot PID health check (basic + metadata + graph + integrity)
```

标题命令计数 16 → 18。

### 测试

H7CAD 单测（3 个）：
1. `read_pid_clsid_returns_none_for_default_package`：synthetic PidPackage (root_clsid=None, storage_clsids empty) → Ok(PidClsidInfo { root_clsid: None, non_root: [] })
2. `read_pid_clsid_returns_populated_fields`：synthetic 填 root_clsid + 2 个 storage_clsid → 验证映射
3. `build_pid_health_report_aggregates_from_cached_package`：build fixture + load → 验证 report 非空 + stream_count=4 + graph_stats 可能 None（fixture 无 P&IDAttributes）

### 落地

- cargo test io::pid_import 全绿（60/60 预期）
- cargo build 全绿
- commit + push main
- .memory 追加段落

## 不做

1. PIDREPORT --json 模式
2. PIDREPORT 过滤段（`--sections=graph,integrity`）
3. PIDCLSID 对比模板的 CLSID 差异（那是 PIDDIFF 的范围）

## 工作量预估

- Step 1 helpers：25 min
- Step 2 命令分支 ×2：20 min
- Step 3 测试 3 个：15 min
- Step 4 PIDHELP + 落地：10 min

合计 ~70 min。
