# 2026-10-01 HTTP 客户端迁移 Windows 本地候选交付

## 完成范围

在 `2026-10-01-http-production-source-cutover.md` 的源码切换基础上，完成 Jina
示例去硬编码私密凭据、官方 Windows 构建、不可覆盖候选打包、实际包检查、本地
SQLite 运行和原生 WebView 界面检查。此处是**本地手测候选**，不是公开发布或生产部署批准。

交付目录：

```text
C:\Users\Public\nas_home\AI\GameEditor\Neuro\release\Gateway\gateway-wreq-20261001-windows-candidate
```

建议用独立数据入口手测，不直接复用真实 home/.ng：

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File C:\Users\Public\nas_home\AI\GameEditor\linshi\gateway-http-release-20261001\Start-Isolated-Gateway.ps1
```

该入口使用证据目录下的 `manual-data`，不导入已有账号或凭据。新安装沿用当前代码的
本地测试管理密钥 `11011101`；它不是生产环境推荐密码。直接启动包里的 EXE 则仍遵守
项目既有 `.ng` 解析规则，可能使用真实用户数据。

## 源码修复和保护

- `examples/jina_probe.rs` 从 `JINA_API_KEY` 读取凭据，缺失、空值、空白、非 ASCII、
  控制字符或超过 4096 字节时，在构造客户端前以固定无秘密错误退出。
- 两个请求共用标记为 sensitive 的 Authorization header。没有使用旧凭据调用 Jina，
  没有轮换外部真实密钥，也没有改 Git 历史。
- 示例由 38 增至 88 有效行，包括输入校验和 6 个聚焦测试，未超过本地 500 行阈值。
- 构建在 `linshi` 的普通目录源快照运行，不创建 worktree 或网络盘符。
  快照的 3,205 个原始输入在续接时与记录、工作源码逐一匹配。
- 工作仓库的 `routes.yaml` 含非空私密值，保持原样。候选快照只将
  `routes.example.yaml` 原字节作为自己的 `routes.yaml`，不携带真实 `.ng` 或运行数据。
- 本阶段开始的 3,792 个源码输入中，唯一改变的既有文件是 Jina 示例；本检查点为新增文档。
  静态模型表没有改动，也没有为本任务刷新两份 detached Cargo lock。

## 构建与来源

上一轮官方构建的回执为空，父进程已经退出，但 Cargo 子进程仍在工作。
续接时核对 PID、工具链和输出目录后仅等待它完成，没有停止其他构建。
随后串行重新执行官方 builder，使用原缓存、Rust 1.98.0、单 Cargo job，
取得完整 `official-build-r2-receipt.json`，退出 0。

| 文件 | 字节数 | SHA-256 |
| --- | ---: | --- |
| gateway.exe | 60329472 | `06cab028a92dcd236d868d8071f15600f9858e7ec1f6e009b77f31dcee90822d` |
| gateway-ui.exe | 12135424 | `6523c84a103865e5787bc212e07e304f23849a28b0bfa32dc6ddd1b64253ebe9` |

来源指纹：`7627f13917ea96d481e135d5354df2275fd1347eec97c0c422438ca4df8044eb`。
源快照没有 `.git`，包内 revision 因而不能冒充新的 Git commit；原 HEAD 和逐文件
hash 映射在 `source-snapshot.json`。本检查点是构建后的记录，不反向修改已冻结的包。

## 实际验证

| 项目 | 结果和范围 |
| --- | --- |
| Jina 单元 | 6/6，复用本阶段已通过且 hash 未变的证据 |
| Jina 示例二进制提前退出 | missing/empty/invalid 三项均退出 1，监听器请求 0 |
| formatter、checker、ratchet | formatter 通过；checker 31/31；ratchet 通过，源码未变证据复用 |
| 模型卡片/ledger/NVIDIA | 4 个文件 12/12，未更改模型表 |
| 官方构建 | 两个 EXE 完成；前端 typecheck、Web 构建通过；编译仍有既有 warnings |
| npm 生产依赖审计 | worker、desktop 两组均 found 0 vulnerabilities |
| 打包和完整性 | 官方 packager、packaged integrity、UI artifact integrity 全部退出 0 |
| 实际本地运行 | SQLite 首启；零额度先拦截；重启保留 key/余额；路由失败退款；轮换保留余额并撤销旧 token |
| 外部数据库边界 | Redis/PostgreSQL 监听陷阱均无连接；两次 backend 正常 drain |
| 原生启动 | 20 秒存活、自动启动独立 SQLite backend、无包内 .ng、进程清理 |
| 原生 UI | CDP 连接本次 gateway-ui 的 WebView；1280×900、390×844 视口截图和人工检查 |
| UI 交互 | 原生 summary Enter 收起/展开；长名称、未知模型、0 调用和缺失成功率；模型区滚轮滚动 |
| 交付副本 | 858 个文件（含 checksums.sha256）全部复制并逐 hash 核对；8 个旧版本的记录 hash 保持 |

窄视口模型容器 `clientHeight=134`、`scrollHeight=203`，实际滚轮使 `scrollTop`
从约 2.67 增到约 68.67。`documentElement.scrollWidth=375 <= innerWidth=390`。
视口调整不等于 Windows 外框尺寸矩阵验收。UI 使用禁用的合成账号和 loopback 地址，
没有调用真实供应商。没有重跑上一阶段与未变源码对应的 355/69 项完整迁移聚焦矩阵。

## 私密信息与公开发布边界

实际包的 858 个文件，包括 EXE，按 UTF-8 和 UTF-16LE 扫描 3 个已知私密值，命中 0。
没有 `.ng`、真实 `.env`、browser profile 或运行数据；两份 routes 与示例模板匹配。

Gitleaks 使用固定版本和默认规则、无路径排除，原始退出码仍为 **10，7 条告警**：
三个历史文档源码 SHA-256、一个公开工件 SHA-256、RFC6455 示例 nonce、客户端 hCaptcha
sitekey、客户端分发的 Supabase anon key。每条都有原文件 hash 和用途分类；没有修改
scanner 规则、allowlist 或把退出码改为成功。anon key 的签名和实际服务端权限没有验证。
因此只批准本地候选，不宣称公开发布安全门禁已全绿。

历史源码/工作配置中的凭据暴露评估、真实密钥轮换、GitHub 托管安全门禁、真实供应商、
Linux/Docker runtime、macOS、HTTP/2 SETTINGS 等仍需独立验收。
`wreq-rt 0.2.2-rc.4` 仍为 prerelease。模型目录沿用本阶段的联网复核和有边界的
no-order-change 决策，未将旧的 reviewed-at 日期伪装为全表新核实。

## 证据位置

```text
C:\Users\Public\nas_home\AI\GameEditor\linshi\gateway-http-release-20261001
```

关键文件：`official-build-r2-receipt.json`、`canonical-candidate-receipt.json`、
`source-snapshot.json`、`input-preservation.json`、`local-storage-runtime/result.json`、
`native-ui-runtime.json`、`native-visual-checks.json`、`package-private-value-scan.json`、
`package-gitleaks-summary.json`、`package-security-review.json`、`completion.json`。
截图为 `native-wide.png`、`native-narrow.png`、`native-narrow-scrolled.png`。

未提交、推送、更新运行服务、改变 Windows 代理、覆盖旧 release 或删除 linshi 外内容。
