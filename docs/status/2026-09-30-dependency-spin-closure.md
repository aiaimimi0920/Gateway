# 2026-09-30：spin 补丁升级验证收口

状态：`spin 0.9.8 -> 0.9.9` 的兼容补丁及防回退合同已完成并重新验证；
**依赖安全门禁、Gateway 整体交付和新版本发布仍未完成**。

## 接续点与修改边界

本记录接续对话 `01a0f1fb-1b21-7b01-823d-a703cfccc2a7` 最后的依赖修复。
上一对话中断后，验证进程已成功结束；本轮核对退出记录并复验当前源码，
没有重复远端合并，也没有接管保留的 S06 Rust/Gemini 范围。

- 根 `Cargo.lock` 由 Cargo 更新，仅改变 `spin` 的版本和 checksum。
- 根锁文件其他 package 记录完全不变；两个独立 Cargo 锁文件保持原 hash。
- 上一对话保存的 crates.io 元数据与本地 crate 归档 checksum 一致；
  补丁的 features、dependencies、Rust 最低版本 `1.38` 和 MIT 许可保持不变。
- `scripts/tests/security-ci-policy.test.mjs` 增加三个 Cargo 锁文件的旧版防回退合同，
  有效行数从 69 增至 81，未新增职责混杂的生产模块或行数例外。
- 防回退回归在升级前因 `spin 0.9.8` 失败；升级后通过。该合同不是漏洞扫描器，
  不会把包撤回等同于漏洞，也不替代 OSV 全锁文件扫描。

原有三个缓存/统计文件的 SHA-256 与上一对话操作前一致。
没有修改凭证、用户数据库、`routes.yaml`、其他子项目或现有发布包；
没有提交、推送、重启 Docker 或添加风险豁免。

## 本轮复验

| 检查 | 实际结果 |
| --- | --- |
| Windows `cargo check --offline --locked --all-targets` | 通过，既有 warning 保留 |
| `local_storage_pipeline_contract` | 3/3 通过，覆盖 SQLite、余额语义和 readiness |
| `image_edit_ingress_limits` | 3/3 通过，覆盖 JSON 和 multipart 输入边界 |
| 安全与发布门禁合同 | 15/15 通过 |
| 行数 checker 测试 | 31/31 通过 |
| ratchet / strict | 通过；2603 个文件、14 个单列不可变运行时资产，受治理源码 `>700=0` |
| `cargo fmt --all -- --check` | 通过 |
| `git diff --check` | 通过 |
| 全 target、全部 Gateway feature 的 `rsa` 反向依赖图 | 退出码 0，`nothing to print` |

`--all-targets` 编译是 Windows 当前 target 的所有 Cargo target 检查，不是跨平台编译。
`cargo tree --target all --all-features` 是依赖图检查，不是 Linux/Docker 运行验收。
编译仍设置 `GATEWAY_PREBUILT_WEB_UI=1`，不能据此声称已构建嵌入新前端的 release。

## 安全扫描的真实结论

本轮通过 Git HTTPS 联网查询 RustSec 上游 HEAD，确认与本地数据库完全一致：
`9b3a3b73a7f42606494c943e95f8196e9994df46`。数据库没有本地改动。
随后用该数据库复扫当前根锁文件，没有 ignore，也没有关闭 yanked 检查。

- `spin 0.9.8` 的 yanked 告警已消失；yanked package 数由 2 降为 1。
- 扫描仍以退出码 **1** 失败，保留 `rsa 0.9.10` / `RUSTSEC-2023-0071`。
  公告仍未列出 patched 版本。当前 Gateway 全 target/all-feature 图未启用它，
  但这不足以把全锁文件门禁改绿，也不证明未来配置、其他构建入口或发布都不可达。
- `lru 0.12.5 <- aws-sdk-s3 1.119.0` 和
  `lru 0.13.0 <- rquest 5.1.0` 保留两个公告对应的四条 unsound 告警。
- `rquest 5.1.0` 仍为 yanked；升级 spin 没有解决其迁移问题。

crates.io API 在本轮额外复查中遇到 TLS 握手失败和读超时。`spin 0.9.9`
的注册表元数据核验引用上一对话已保存的联网证据，不标记为本轮新鲜下载。
没有关闭证书验证、改全局网络配置或以网络错误冒充扫描通过。

## 保留问题与停止条件

上一对话的隔离调查确认：仅关闭 SQLx 默认功能并不能移除锁文件中的 `rsa`；
已检查的 `aws-sdk-s3 1.137.0` 使用 `lru ^0.16.3`，不能直接消除本次全部告警；
已检查的 rquest 5.x 发布均为 yanked。这些版本调查是保存的快照，不是本轮
对“最新上游版本”的新断言。本轮不扩大到 SDK、HTTP/TLS 客户端或工具链迁移。

下一独立批应设计并验证 owning-parent 迁移或依赖移除方案；不能手改锁文件、
盲目跨版本升级或自行批准例外。真实 GitHub OSV/Gitleaks/CodeQL 激活、S06/S18、
Linux/Docker 和新不可变 release 的完整性、原生 UI、本地存储验收仍需分别完成。
本轮只收口上述兼容补丁，不发布安全扫描仍失败的新候选。

本轮证据与精确命令：

`C:\Users\Public\nas_home\AI\GameEditor\linshi\gateway-spin-resume-20260930-063813`

上一对话的隔离调查、修复前失败和归档校验：

`C:\Users\Public\nas_home\AI\GameEditor\linshi\gateway-dependency-closure-20260930-052747`
