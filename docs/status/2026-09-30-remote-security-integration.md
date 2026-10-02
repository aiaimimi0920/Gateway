# 2026-09-30：远端整合与安全发布门禁

状态：本轮远端合并和本地发布门禁修复已完成；**Gateway 整体尚未达到最终交付状态**。

## 代码与边界

- 本地 `main` 从 `8670086` 快进到 `c3a74e2`，合入 7 个提交、30 个文件的变更。
- `origin/codex/security-quality-baseline` 已包含控制台传输可靠性分支。
- 恢复分支：`backup/gateway-before-remote-merge-20260930-040704`。
- 下述 CI 修复、测试和文档是合并后的本地工作区变更，尚未提交或推送。
- 原有三个缓存/统计文件的未提交内容保留；没有重置、清空凭证或替换运行中的版本。

## 本轮补齐的发布边界

原 Security workflow 独立运行，Windows 候选、Docker 和正式 Release 没有依赖它。
现在三个发布入口均通过 `needs: security` 等待安全检查：先解析一次目标 ref，
输出 `source_sha`，再让依赖扫描、秘密扫描和最终构建使用同一 commit。
手工 tag 发布仍解析用户指定的 tag，扫描失败或取消不能绕过门禁。

并发组包含调用方 workflow 名称，避免独立扫描、Windows 和 Docker 相互取消。
调用方只赋予扫描所需权限，没有传入内容或包发布写权限，也没有放宽扫描策略。
详细合同见 [security-release-gates.md](../security-release-gates.md)。

## 新鲜验证

| 检查 | 结果与范围 |
| --- | --- |
| 新发布门禁回归 | 修复前 8/8 失败；修复后与既有安全契约合计 14/14 通过 |
| Workflow 语法 | 校验官方发布 SHA-256 后运行 `actionlint 1.7.12`，四个修改的 workflow 通过 |
| 新锁文件前端 | `linshi` 隔离安装 Vitest 4.1.11；83 文件、377/377 测试与类型检查通过 |
| Windows Rust | `cargo check --offline --locked --all-targets` 通过，仍有 warning |
| Windows Tauri | 独立 manifest 的 `cargo check --offline --locked` 通过 |
| Rust preset 回归 | 7/7 通过，3252 项被过滤；不是完整 Rust 测试套件 |
| Python 聚焦契约 | 初始 60/60 通过；CI 修复后直接相关的 39/39 通过 |
| 发布恢复和并发 | 12/12 通过，使用临时夹具，不是正式包发布 |
| 行数 checker | checker 31/31；最终 ratchet/strict 通过，2603 文件、14 个单列运行时资产 |
| 格式和差异 | Rust/Tauri formatter 检查与 `git diff --check` 通过 |

前端测试没有清空原工作区 `node_modules`；新锁文件的依赖安装和完整测试在隔离目录
中完成。当前工作区依赖目录仍保留原版本。编译的 Windows 结果不能外推为 Linux、
Docker、真实上游调用或原生窗口验收通过。

## 安全审计结果

以下为 spin 补丁更新前的审计快照。后续 `spin 0.9.8 -> 0.9.9`、
防回退合同和当前复扫结果见 [依赖补丁收口](2026-09-30-dependency-spin-closure.md)；
该补丁已消除 spin 的 yanked 告警，但没有使整体安全门禁通过。

联网更新 RustSec 数据库至 `9b3a3b73a7f42606494c943e95f8196e9994df46`，
数据库 commit 时间为 `2026-09-30T09:15:39+02:00`。

- 两个 npm 生产依赖树的实时审计均为 0 个已报告漏洞。
- 根锁文件仍报告 `rsa 0.9.10` 的 `RUSTSEC-2023-0071`，公告没有列出修复版本。
  当前 Windows 默认启用依赖图中没有 `rsa`，不能仅据锁文件宣称正在运行的服务
  暴露了该漏洞；也不能据此把全锁文件安全门禁改成通过。
- 根锁文件另有 `lru` 的 unsound 告警和 `rquest`、`spin` 的 yanked 告警。
  已确认 Windows 图中 `lru 0.12.5` 来自 AWS SDK，`lru 0.13.0` 来自 rquest。
- Tauri 锁文件没有 cargo-audit 所列 vulnerability，但仍有 `glib` unsound 和
  `proc-macro-error` unmaintained 告警；local-data 锁文件没有上述报告项。
- 没有添加公告忽略、虚构审批或降低门禁。cargo-audit/npm 的结果不能替代 GitHub
  上 OSV、Gitleaks、CodeQL 的真实运行结果。

## 后续退出条件

1. 处理依赖保留项和实际启用图中的安全告警；必要的临时例外必须有独立审批和期限。
2. 经授权提交/推送后，验证 GitHub 安全扫描和发布依赖图的真实执行。
3. 继续按当前 S06/S18 计划完成跨模块、Linux/Docker 和运行时验收，不接管保留中的
   Rust/Gemini 源码范围，不用旧发布记录代替新快照验收。
4. 满足门禁后再构建新的不可变 release，完成完整性、原生 UI 和本地存储 smoke。

本轮没有生成新的正式 release，也没有重启现有 Docker 栈。

完整命令、分阶段结果、审计输出、备份和网络重试证据位于：

`C:\Users\Public\nas_home\AI\GameEditor\linshi\gateway-sync-20260930-040704`
