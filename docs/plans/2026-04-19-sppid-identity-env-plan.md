# 开发计划：SPPID 身份字符串去硬编码（保守版）

> 起稿：2026-04-19  
> 背景：今日 H7CAD × SPPID 集成分析的改进点 1。`SPPID_TOOL_ID = "H7CAD"` / `SPPID_SOFTWARE_VERSION = "0.1.3"` 两个常量与 `Cargo.toml` 双写，升级版本时容易漂移。本轮聚焦**最小风险**改造：ToolID 绑 `CARGO_PKG_NAME`（零下游风险），Version 保留硬编码但加 drift-detection 单测防漂移。

## 动机

`src/io/pid_import.rs:148-149`:

```rust
const SPPID_TOOL_ID: &str = "H7CAD";
const SPPID_SOFTWARE_VERSION: &str = "0.1.3";
```

两者都被 `build_publish_data_xml` / `build_publish_meta_xml` 注入 publish 产物（L1931-1933 / L1997-1999），最终出现在 SPPID 消费方读的 `*_Data.xml` / `*_Meta.xml` 里。

**漂移风险**：`Cargo.toml` 的 `name` 或 `version` 若改动，这两常量若忘改，publish 产物会携带错误身份 → 下游审计/版本统计失真，甚至被 SPPID 消费方按字符串匹配拒收。

## 保守原则

- **`CARGO_PKG_NAME`（`"H7CAD"`）零下游风险** → 绑 `env!` 宏自动注入
- **`CARGO_PKG_VERSION`（`"0.1.3"`）存在下游风险**：SPPID 消费方可能按精确字符串匹配校验，Cargo 版本升级 → publish Meta.xml 的 `SoftwareVersion` 值跟随变 → 未知兼容性后果。**不自动绑定**，改为**加 drift detection 单测**：断言 `SPPID_SOFTWARE_VERSION == env!("CARGO_PKG_VERSION")`，下次 `cargo release` 忘同步 → CI 失败提醒。

## 目标

1. `SPPID_TOOL_ID` 改为 `const SPPID_TOOL_ID: &str = env!("CARGO_PKG_NAME");`
2. 新增单测 `sppid_software_version_tracks_cargo_pkg_version`，断言两者字符串相等
3. 现有 2 个引用点（L1933 / L1999）透明受益，无需改

## 非目标

- 不改 `SPPID_SOFTWARE_VERSION` 的声明方式
- 不引入 build.rs
- 不改 `SPPID_DATA_COMPONENT_SCHEMA` / `SPPID_META_COMPONENT_SCHEMA` / `SPPID_BRAN_BLOCK_NAME` / `SPPID_REL_*`（这些是协议固定值，与 H7CAD 自身版本无关）
- 不跨 crate 传递（H7CAD 仍然单 binary）

## 实施步骤

### M1 — `SPPID_TOOL_ID` env! 注入（5 min）

改 `src/io/pid_import.rs:148`:

```rust
// 原
const SPPID_TOOL_ID: &str = "H7CAD";

// 新
/// SPPID publish 产物（Data.xml / Meta.xml）里 `ToolID` 字段的值。
/// 绑 `CARGO_PKG_NAME` 自动跟随 Cargo.toml `[package].name`，避免
/// 改 crate 名称时漂移。若未来需要与产品名称分离，显式赋字面量即可。
const SPPID_TOOL_ID: &str = env!("CARGO_PKG_NAME");
```

### M2 — 版本漂移检测单测（10 min）

在 `src/io/pid_import.rs` 的 `#[cfg(test)] mod tests` 追加：

```rust
#[test]
fn sppid_software_version_tracks_cargo_pkg_version() {
    // SPPID_SOFTWARE_VERSION 被写入 publish Data.xml / Meta.xml 的
    // `SoftwareVersion` 字段，SPPID 消费方可能按精确字符串匹配校验。
    // Cargo.toml [package].version 升级时必须同步更新该常量；此测试
    // 让遗忘同步在 CI 显性失败，而不是在生产 .pid 里悄悄漂移。
    assert_eq!(
        super::SPPID_SOFTWARE_VERSION,
        env!("CARGO_PKG_VERSION"),
        "SPPID_SOFTWARE_VERSION drift: Cargo.toml version changed but the \
         publish identity constant was not updated. Either bump \
         SPPID_SOFTWARE_VERSION to match, or (if the SPPID consumer requires \
         a frozen value) move the drift-detection doc-comment accordingly."
    );
}

#[test]
fn sppid_tool_id_matches_crate_name() {
    // 回归守护：env!("CARGO_PKG_NAME") 绑定生效，SPPID_TOOL_ID 的值
    // 就是 Cargo.toml [package].name（目前为 "H7CAD"）。
    assert_eq!(super::SPPID_TOOL_ID, env!("CARGO_PKG_NAME"));
    assert_eq!(super::SPPID_TOOL_ID, "H7CAD");
}
```

### M3 — CHANGELOG + commit + push（10 min）

1. `cargo test --bin H7CAD io::pid_import::tests::sppid` —— 2 新测试通过
2. `cargo check --bin H7CAD` —— 整体编译 OK
3. `CHANGELOG.md` 未发布段追加
4. commit（PowerShell heredoc-free via tmp 文件）
5. `git push origin main`

## 预计工时

| 步骤 | 估时 |
|---|---|
| 写 plan | 完成 |
| M1 | 5 min |
| M2 | 10 min |
| M3 | 10 min |
| **合计** | **~25 min** |

## 风险与缓解

| 风险 | 缓解 |
|---|---|
| Cargo.toml 某天把 `name` 改成 `"h7cad"`（小写）会导致 SPPID_TOOL_ID 大小写变 | 若发生，drift-test `sppid_tool_id_matches_crate_name` 的第 2 条 assert 会失败，提醒人工决策。此为可控风险。 |
| `env!` 宏对 Cargo workspace 内部 crate 的行为在 rustc 版本间是否稳定 | `env!("CARGO_PKG_NAME")` / `CARGO_PKG_VERSION` 是 cargo 长期稳定契约（1.0 就有），无风险。 |
| SPPID 消费方缓存 ToolID="H7CAD"，若未来改 Cargo name 变别的 → 下游打碎 | 保守做法：**不**改 Cargo name（本轮也没打算改）。绑 env! 只是自动化，不主动推动 name 变更。 |

## 回滚

单文件（`src/io/pid_import.rs`）+ 2 个测试。`git revert` 即可。

## Next 排队（不在本计划）

- 改进点 3: `pid_package_store` LRU 上限
- 改进点 5: `.gitattributes` 解决 `pid_package_store.rs` CRLF 伪 modified（低价值）
- SPPID_BRAN_ATTRIBUTES schema versioning（需要 SPPID 新版本实际驱动）
