# Issue #3：发布阻止项的安全 findings 审计

本记录是 2026-10-05 的实时状态核查及脱敏分类，不是新的运行时交付、
安全豁免、凭据有效性验证或正式发布批准。关联
[Issue #3](https://github.com/aiaimimi0920/Gateway/issues/3)。

## 权威基线与完成边界

- 当前 Gateway HEAD 与联网读取的远端 main 均为
  `7431ebbae206fdfe81dd1bf75740c0fc928eb97d`。
- Issue 最新 S3 回执仍为
  [5979715515](https://github.com/aiaimimi0920/Gateway/issues/3#issuecomment-5979715515)。
- 本轮直接读取 CI run `37198457299` 最新 jobs，14/14 completed/success。
  Windows Rust、frontend typecheck/build、Tauri wrapper steps 均为 success。
  这是读取既有同提交 CI，不是本轮重跑测试或重新触发 CI。
- 本轮直接读取 Build Windows `37198457543` 与 Docker `37198457628`：
  两个扫描 job 在 `Enforce findings for release callers` failure，实际 build skipped。
- S1/S2/S3 当前源码及保护性测试完成独立只读核查，未发现明确源码不符；
  测试源码存在断言不等于本轮重新执行通过。副作用一次指 adapter execution，
  deadline 指 send stage，不外推为供应商内部物理 HTTP 或整链金额硬额度。
- `MAX_PROVIDER_SERIES=256` 保留 256 个普通 series，加一个 overflow series，
  总数最多 257；Issue 要求保留既有保护及 overflow，不改写成总数不超过 256。

## 同提交扫描 artifact 的验证

通过 authenticated GitHub connector 重新取得两个未过期 artifact，下载后
分别核对 GitHub metadata 中的 SHA-256；未读取或公开秘密值。

| Artifact | ID | 下载 ZIP SHA-256 |
| --- | --- | --- |
| OSV Build Windows | `11302046269` | `2b0c011a1379cc5e098c61d35536008aeaaaeb3809e5fc3990ae5d30c6f5bd95` |
| Gitleaks Build Windows | `11301870910` | `580f2096b2164fd4ddcd318cd04a7528e9af64ae3677a3a81805fd7c90950638` |

原报告的 Linux source URI 仅作一一对应的平台路径转换，随后调用仓库
`scripts/security-report.mjs` 原 validator：五锁 JSON/SARIF 完整覆盖、3 个
OSV occurrence、134 个历史 Gitleaks finding，退出码与报告一致，无 suppression。
未修改 scanner、validator、workflow 或报告中的 finding 集合。

## Gitleaks：不只是已经移走的历史代码

使用官方 Gitleaks 8.24.3 Windows 二进制，执行前验证官方 release checksum：
`3f1a35578631dbfe633cc5b49e6c906e55ff14a4bfd7336a10fb27fe33b6dcd2`。
`git archive HEAD` 导出到独立 linshi 目录；只扫描这个已提交源码快照，不扫描
用户 `.ng`、工作树未提交配置、依赖缓存或真实业务数据。

当前树诊断扫描使用 `dir --redact=100 --exit-code=2 --log-level=error`
和完整 SARIF，exit 2、117 findings；不是 scanner execution failure。
其中：

| 当前提交位置 | Finding 数 | 可以证明的事实 |
| --- | --- | --- |
| `graphify-out/cache/stat-index.json` | 78 | 生成缓存仍被 Git 跟踪，存在匹配；不能默认视为无害或已清理 |
| `routes.yaml` | 7 | 配置仍存在凭据形态匹配；未验证真实凭据是否有效、撤销或属于示例 |
| 其余 20 个源码、测试、脚本及文档路径 | 32 | 完整位置见脱敏报告；不能仅按文件名判定误报 |

对 134 条历史记录的原提交位置只做内存中比对，117 条记录的整段源行仍存在于
当前 HEAD。报告只保留 rule、path、line、commit 和布尔状态；不保留秘密、源片段
或 commit message。余下记录不代表凭据已撤销，当前树零 finding 也不能代替
发布要求的完整 Git 历史扫描。

没有实际调用供应商或验证这些凭据。后续应先确认配置 owner 和凭据性质，
再选择私密保留、迁移、轮换及历史治理；不能为通过扫描把真实 token 拆分拼接。

## GTK 依赖：上游迁移版本存在，但 owning parent 尚未迁移

本轮联网核实 crates.io 官方依赖 API，并独立核对 RustSec 公告。
当前 desktop lock 为 `glib 0.18.5`、`gtk 0.18.2`、`proc-macro-error 1.0.4`。
两条 canonical 公告仍是 `RUSTSEC-2024-0429` 与 `RUSTSEC-2024-0370`。

| 已发布版本 | 官方关键约束 |
| --- | --- |
| `gtk 0.19.0` | `glib ^0.22`、`gtk3-macros ^0.19.0` |
| `tauri 2.12.1` | Linux/BSD `gtk ^0.18`、`webkit2gtk ^2` |
| `wry 0.57.0` | Linux/BSD `gtk ^0.18`、`webkit2gtk =2.0.2` |
| `webkit2gtk 2.0.2` | `glib ^0.18.0`、`gtk ^0.18.0` |
| `glib 0.18.5` | `glib-macros ^0.18`；该宏链与 gtk3-macros 都引入旧 proc-macro-error |

因此仅更新锁文件、增加新版 glib 或升级稳定 Tauri 不能移除旧链。
GLib 官方窄修复可回移实际 UB，但旧版本仍落在公告范围；不能改版本/包名规避
扫描，或把 backport 自动称为 findings-free。整体 parent 迁移或长期维护 fork
需要另外设计、平台编译及维护责任，不在本轮擅自引入。

本地 `cargo tree --locked --offline --target x86_64-pc-windows-msvc -i`
对 glib、proc-macro-error 均为 `nothing to print`；这说明该 target 没有依赖边，
不证明 Linux/BSD desktop 的上游调用不可达，也不改变五锁扫描要求。

官方来源：

- <https://crates.io/api/v1/crates/gtk/0.19.0/dependencies>
- <https://crates.io/api/v1/crates/tauri/2.12.1/dependencies>
- <https://crates.io/api/v1/crates/wry/0.57.0/dependencies>
- <https://crates.io/api/v1/crates/webkit2gtk/2.0.2/dependencies>
- <https://raw.githubusercontent.com/RustSec/advisory-db/main/crates/glib/RUSTSEC-2024-0429.md>
- <https://raw.githubusercontent.com/RustSec/advisory-db/main/crates/proc-macro-error/RUSTSEC-2024-0370.md>

## 证据、下一步与操作边界

证据目录：
`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-issue3-security-20261005-audit-r1`。
关键文件为 `jobs-*.json`、`artifacts-*.json`、`osv/`、`gitleaks/`、
`head-current-tree-status.json`、`redacted-triage.json`、
`routes-redacted-classification.json`、`upstream-dependency-constraints.json`。

本轮没有修改依赖或产品源码，没有重新 build；没有添加安全例外、停止业务进程、
删除缓存、改写 Git 历史、变更真实凭据、提交、推送或更新远端 Issue。
当前本地工作树的其他修改及旧 release 全部保留。只读扫描和文档进度不能宣称
正式发布完成，Issue #3 保持未完成。

后续先确认两类处置范围：凭据轮换及 Git 历史治理的授权，和 GTK parent 迁移、
维护 fork 或限定风险接受的路线。任何例外仍须真实独立批准、明确范围和到期日；
未批准前 release fail closed。不能通过更改扫描范围或放宽门禁制造成功。

本轮本地验证：三个安全契约文件共 30/30 tests passed；Neuro 通用开发规范
契约 passed；Gateway `git diff --check` passed；两份修改文档均为 UTF-8 无 BOM。
源码和 GTK 约束为只读审查，未将这些检查冒充 S1/S2/S3 的新测试执行或新发布。

## 续接：私密配置迁移的隔离兼容验证

本轮继续核对 `src/local_installation.rs:30-42`、
`src/console/runtime.rs:294-326` 和 `docs/local-data-storage.md`。
本地启动由 `GATEWAY_DATA_DIR` 选定完整数据根，并安装该根的 routes/state 环境路径；
已有 active revision 时从持久化 authority 启动，不把 YAML mirror 重新当成权威。
因此不新增重复的配置存储，不用仅替换 YAML 或只改 `GATEWAY_ROUTES_FILE` 冒充迁移。

使用已验证的 r2 本地候选，仅在 linshi 中创建一个 provider、一个账户、一个模型
及一个 alias，凭据全部为合成值；没有读取或复制真实用户 `.ng`。
原实例正常 drain 退出后，把整个测试数据根复制到新的私密副本，11 个文件逐一
验证字节 hash 一致；随后只在副本把 YAML mirror 改为空，验证持久化 authority
不会因 mirror 变化而丢失。实际三个阶段全部通过：

1. 原数据根启动；
2. 完整副本启动，仍读取原 active revision 而非空 mirror；
3. 副本再次重启。

每阶段均实测 SQLite readiness、模型与 alias、provider/credential ID、active revision
保持不变。上游 loopback 监听器没有连接，真实模型调用 0；三个 owned backend 均
正常 drain/exit，最后无残留。原始测试数据完整保留，副本修改也只在 linshi 内。
这是合成数据的迁移前置证据，不是实际账户迁移或上游凭据有效性验收。

最终证据：`private-root-proof-09a9ef3a994b406aa9e4d8d935386536/result.json`，
位于本文 evidence 目录。初轮临时 harness 缺少 `Write-Smoke` 的失败记录保留；
补齐 helper 后成功，再补 metadata/revision 断言通过，不修改产品以掩盖 harness 错误。

此前已请求第一阶段处置同意。用户随后明确回复“我同意你继续操作。同意”，
私密保留、仓库秘密移除及生成缓存取消跟踪现已获批准。凭据轮换、历史重写、
安全例外、提交、推送和部署仍不在范围内；真实备份只留在用户私密 `.ng/backups`，
不放 Git、release 或可发布的验收记录。仍保持发布 fail closed。

## 已批准第一阶段：核心备份通过，完整备份仍未完成

通过现有子进程环境与监听端口核实，正在运行的桌面实例使用用户 home 下的 `.ng`：
local storage、`.ng/local/routes.yaml`、`.ng/local/state`、同根 objects 和 credentials。
仓库 routes 并非该实例的配置，未把两份不同版本的配置混合或覆盖。

| 配置 owner | providers | credentials | model routes | aliases |
| --- | ---: | ---: | ---: | ---: |
| 当前私密 `.ng` | 32 | 192 | 45 | 31 |
| 仓库原 routes | 27 | 27 | 44 | 33 |

管理接口读取的活动修订为 `r27-34cf6278f5a5`，source 为 `recovered`。本轮没有模型调用。

新备份位于 `.ng/backups/gateway-issue3-private-cleanup-<timestamp>`。该新目录关闭
ACL 继承，只允许当前用户、SYSTEM 和 Administrators；原数据根和既有备份权限不变。
完整文件清单、原配置和缓存副本均留在该私密目录，linshi 只写脱敏验收记录。

- 已分别复制并核验原仓库 routes、gitignore、Git index、旧 console state，以及
  544 个 Graphify 缓存文件；没有覆盖或删除源文件。
- 私密核心文件 11051 个、local 文件 145 个逐一 hash 相等，活动修订前后相同。
- 合成锁探针证明 Python 区域锁与 Rust `File::try_lock` 互斥。持有同一 console
  writer lock 时复制 console/YAML，并使用 `sqlite3.Connection.backup` 获取在线快照。
  备份 `integrity_check=ok`，不把源 WAL/SHM 混搭到备份主库；源 DB/WAL/SHM 均保留。
- SQLite 全部 12 个用户表已保留；备份时 request audits 20、credential model states
  12。这些是持久化计数，不是凭据上游有效性证明。
- WebView 已复制并核验 603 个文件，39 个独占文件无法读取，其中包括 Cookie 数据库。
  因此 `coreBackupVerified=true`，但 `fullBackupVerified=false`。

不得将核心备份称为完整 `.ng` 备份。在获得允许、让当前 Gateway 正常退出并补齐
WebView 一致快照前，暂不移除仓库 routes 秘密值。现有 Gateway 和其他服务保持运行。

### 缓存取消跟踪完成

只新增 `/graphify-out/cache/` ignore 规则，并对该路径执行 `git rm --cached -r`。
544 个文件已从 index 移除，但本地文件全部存在、字节 hash 不变；其他 index entries
逐项相等。没有整树 stage、reset 或 clean，没有接管现有 UI/后端 dirty edits。
index deletions 尚未提交，仓库 routes 字节与私密备份原件相同。

本轮本地检查：30/30 安全契约、effective-line ratchet、Neuro 开发规范契约、
worktree/index 的 `git diff --check` 均通过。无产品源码改动或新 build。
取消跟踪不会改写 HEAD 或历史扫描结果，不能据此宣称 Gitleaks 或正式发布通过。

脱敏证据目录：
`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-issue3-private-cleanup-20261005-r1`。
主要记录为 `backup-result.json`、`cache-cleanup-result.json` 和 `lock-compatibility.json`。
下一步需允许当前 Gateway 正常退出、备份完成后重新打开同一版本；不停止其他服务。

## 第一阶段完成回执：正常退出、完整备份、配置清理及同版本重开

用户随后要求“继续完成剩余任务”。2026-10-05 本地时间已执行上轮请求的正常退出
与重开，不再将同一备份步骤重复列为待同意。操作只覆盖当前 Gateway，没有停止其他
服务；没有轮换凭据、改写历史、降低门禁、提交、推送或部署。

### 完整私密快照

先通过管理认证请求 runtime drain，等待后端成功退出，随后只向已核实的 `Gateway`
Tauri 控制台窗口发送正常关闭消息。后端和 UI 的退出码均为 0，WebView 后代进程
全部退出，未走桌面超时强杀分支。原端口释放后才开始新的停止后快照。

新私密备份目录与原部分备份是兄弟目录，ACL 关闭继承，仅当前用户、SYSTEM 和
Administrators 可访问；子文件继承该私密 ACL。具体位置记录在脱敏 evidence 的
`backup-full-location.txt`，原配置、秘密绑定、完整文件清单和扫描原报告不进入 Git。

- 同一停止状态下复制并两次核验 11837 个文件、1172 个目录，包含 640 个 WebView
  文件；不可读取文件为 0，`fullBackupVerified=true`，未拼接不同时间的旧快照。
- 唯一排除目录为 `backups`，避免递归复制备份自身；所有既有备份仍原样保留。
- SQLite 停止后完整原文件留在 raw snapshot；另在私密副本中调用 backup API 得到
  独立可恢复数据库，不改源库或 raw snapshot。完整性为 `ok`，12 个用户表保留。
  request audits 20、credential model states 12；runtime instance 停止后为 0，重开后为 1，
  是正常实例生命周期，不是账户或历史记录丢失。
- 活动修订仍为 `r27-34cf6278f5a5`，revision archive 的 JSON/YAML digest 均通过。
- 20 个唯一 runtime-state 对象引用中 6 个存在、14 个在本次备份前已缺失，源与副本
  的存在性完全相同。完整备份证明所有现存数据保留，不证明缺失引用已修复或所有
  浏览器账号可用；未擅自删改这些引用或调用供应商验证。

### 仓库配置清理

只对 `routes.yaml` 的秘密 scalar 做精确 span 替换，不重新 dump、重排或改写整份 YAML。
私密备份中保留原字节和绑定资料，原运行数据的 `.ng/local/routes.yaml` 没有变化。

| 替换项 | 数量 | 处理 |
| --- | ---: | --- |
| provider api_key | 2 | 受支持的环境引用 |
| credential api_key | 7 | 受支持的环境引用 |
| credential extra_body.token | 1 | 置空，不伪造环境替换支持 |
| credential extra_body.session | 1 | 置空，移除既有但不受支持的环境占位 |

原先已受支持的环境引用保持不变，包括 Cookie 引用。所有非秘密结构和元数据完整
相等，provider 顺序/ID、credential 槽位、supported models、model map、model routes 和
aliases 均保留；仓库计数仍为 27/27/44/33。新增注释明确这是安全目录模板，不是
当前用户的账户库。环境变量未设置时引用仍原样保留，不能将其称作 fail-closed 配置。

已先做合成检查，再用校验过官方 ZIP checksum、且与 ZIP 内可执行文件逐字节相同的
Gitleaks 8.24.3，对原配置和安全候选的两个精确 stdin 输入进行扫描。原配置 exit 2、
7 findings，安全候选 exit 0、0 findings，才原子替换仓库配置。扫描未包含 live `.ng`、
依赖、整个工作树或 Git 历史，也不将配置零 finding 外推为正式发布通过。

### 重开与保护性验证

同一 `gateway-product-20261005-test-scope-r1/gateway-ui.exe` 已重新打开，原生控制台可见。
新 UI PID 36072、子后端 PID 41840；`/readyz` 和管理路由读取均为 200。后端仍使用
用户 home `.ng`、local storage 和同根 routes/state/objects/credentials，没有新建 portable
`.ng`。活动修订、权威 document digest、私密 routes digest 和 32/192/45/31 的计数相同。

11009 个 credentials、objects、profiles 和 connection 文件逐一与停止快照 hash 相同。
本任务发起模型调用为 0，request audits 仍为 20。旧部分备份和完整备份均保留，既有
544 个 cache 文件仍在本地；本轮没有再次修改 index 或其他人的 dirty 源码和报告。

聚焦安全契约 30/30、行数 checker 自测 31/31、ratchet、Neuro 通用规范契约及两仓库
`git diff --check` 已完成。没有产品源码修改或新的构建，使用原版本复核重开效果。
首次临时 preflight 对 revision 对象形状的假设、scanner ZIP/EXE checksum 口径、
PowerShell 回执中的布尔语法和 checker 错误目录均已纠正；没有用空测试集合当成功，
没有重复启动实例。最终回执记录真实输入、计数和通过结果，不覆盖前阶段记录。

主要证据：`normal-stop-result.json`、`full-backup-result.json`、
`config-sanitization-result.json`、`config-gitleaks-result.json`、`restart-verification.json`、
`first-stage-final-verification.json`，位于前述 private-cleanup evidence 目录。

本阶段完成不等于 Issue #3 或严格公开发布完成：已提交 HEAD/历史未改变，既有其他
Gitleaks findings 和 GTK 依赖路线仍需处理；凭据轮换、历史重写及外部发布须另外批准。
