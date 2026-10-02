# 2026-10-02：浏览器 TLS 依赖修复与 GitHub 构建门禁

接续 [PC2 Linux 候选](2026-10-01-http-pc2-linux-candidate.md) 的官方
Linux/Docker 构建及 19 项隔离公开 API 验收任务。
本检查点修复浏览器 TLS 的已确认依赖问题；**没有完成 Linux release 构建、
镜像发布或 19 项运行验收，也没有批准或启用安全例外。**

## 真实远端停点

此前源码已正常推送到 Gateway 独立仓库 `main`：
`3ba6f2994d532042125cc60d9be88437eddf8ed9`。
本轮联网读取该提交的 GitHub Actions，而不是把本地工作流配置视为远端通过。

- [CI 36950909386](https://github.com/aiaimimi0920/Gateway/actions/runs/36950909386)
  的 Linux、Windows job 都在 browser worker 的 `audit:prod` 失败，
  尚未进入产品 Rust 编译。
- [Security 36950910058](https://github.com/aiaimimi0920/Gateway/actions/runs/36950910058)
  的 coverage 和 Gitleaks 通过，OSV 检出三个 canonical finding；
  reporter 按 `fail-on-vuln=true` 正确失败，不是工具异常。
- Windows 候选和 Docker 的实际 build job 被安全依赖阻断而 skipped。
  CodeQL 成功不等于产品编译或发布成功。

## 最小修复

官方公告 `GHSA-86w9-cpqp-85rv` / `CVE-2026-85393` 影响
`node-forge <=1.4.0`；本轮联网核实没有列出的修复版本。
`selfsigned 3.0.1` 将它引入生产依赖。

将直接依赖精确固定为 `selfsigned 5.5.0`，通过 npm 在 `linshi` 的隔离
owner 更新 lockfile。旧依赖记录中只有根声明和 selfsigned 改变；
移除 node-forge，新增依赖均属于新的证书实现链。未使用 `audit fix --force`，
未降低审计级别，也没有把生产依赖移入 devDependencies。

5.x 使用 native crypto，`generate()` 返回 Promise，且移除了 `days`。
TLS owner 和唯一生产 server 调用点改为 `async` / `await`；显式传入
notBeforeDate 和 30 天后的 notAfterDate，保留 SHA-256、RSA 2048、原 SAN、
文件名、存储路径、生成/复用标记和不完整证书对的处理方式。
既有文件仍按原始 PEM 字节加载，不改写旧 PKCS#1 私钥。

测试验证 HTTPS 构造等待证书完成，异步失败向主调用传播，不创建 HTTPS server。
真实 loopback HTTPS 请求仅信任该测试生成的证书，保留证书链和 hostname 校验；
没有关闭 TLS 验证，也没有调用真实浏览器或付费 provider。

## 新鲜本地验证

| 检查 | 实际结果 |
| --- | --- |
| 修复前配置/server 测试与生产审计 | 22/22 通过；audit exit 1、2 high |
| 隔离新锁文件安装 | 官方 `npm ci --no-audit --no-fund` 通过 |
| TLS 配置、server 和 HTTP 相邻测试 | 52/52 通过，包含 30 天有效期、旧私钥复用和真实 HTTPS |
| browser worker `audit:prod` | exit 0，0 vulnerabilities |
| coverage/release 安全契约 | 16/16 通过，保留五个锁和 fail-on-finding |
| 行数 checker 与 ratchet | checker 31/31，ratchet exit 0 |
| nested worker package 合同 | 1/1 通过，不是正式 release 构建 |
| 静态检查 | 六个修改的 JS 文件 `node --check`、Rust formatter、`git diff --check` 通过 |

使用 GitHub 官方 release 元数据核验 OSV-Scanner 2.5.1 Windows 二进制
SHA-256 `25e42f5ef6711fd8c0fb45390972205891dd44c6bd02ac93f0f63e8e98d9bfb6`，
检查其实际版本后，在线扫描同一份五锁清单。扫描 exit 1；node-forge 已消除，
剩余仅是桌面锁图的下列两条 canonical finding，没有添加 suppression。

## 剩余 GTK 链与审批边界

- `glib 0.18.5`：`RUSTSEC-2024-0429` / `GHSA-wrw7-89jp-8q8g`。
  缺陷位于 VariantStrIter 的 C out-argument；修复版从 0.20.0 起，
  GTK3/Tauri 的 `^0.18` 链不能直接选入。上游有窄源码修复，但没有兼容的
  已发布 0.18 patch。直接添加 0.20 或升级当前稳定 Tauri 都不能清除旧链。
- `proc-macro-error 1.0.4`：`RUSTSEC-2024-0370`，停止维护公告，无修复版。
  glib-macros 和 gtk3-macros 都引入它，不能只处理一个 parent。

独立只读核查确认 GTK 链属于 Linux/BSD desktop target。
当前 Windows target 的两条 Cargo inverse tree 都为 `nothing to print`。
这不证明 Linux desktop 的受影响上游调用完全不可达。

已请求用户审批仅针对这两条公告、到 2026-10-31 自动失效的限定例外：
只允许 Windows desktop 和 Linux headless/Docker 候选，在解除或重新复核前
禁止 Linux/BSD desktop 发布；五锁仍全部扫描，其他 finding 仍阻断。
**审批未落实前不创建 OSV ignore、不虚构 reviewer，release 保持 fail closed。**

本轮没有清空工作区依赖、重置 Git、改 PC2 网络或重启业务容器。
新的源码与 lockfile 不自动进入 PC2 原 2,418 文件冻结快照；后续正式构建必须
重新冻结输入，使用新的不可变候选 ID 并执行原 19 项 API 断言。

临时安装、完整日志、在线扫描 JSON 和独立 GTK 核查位于
`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-dependency-gates-20261002/`。
