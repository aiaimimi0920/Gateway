# Gateway HTTP 客户端本地源码切换闭环

接续会话 `01a0f544-d50a-7de1-9631-431919bae8a1` 和用户“继续推进”的授权。
上一阶段 [Gemini 错误接口闭环](2026-10-01-http-gemini-error-closure.md) 已完成，
本轮不重新设计其泛型错误、终态或 legacy 单向适配。

**已在本地 Gateway 源码完成 wreq 切换及限定回归。运行中的服务、现有 EXE、
release、Docker 容器和远程仓库没有更新。该阶段完成不等于完整 HTTP 迁移、
正式产品发布或全项目安全验收。**

## 真实停点与取舍

起点仍为 `rquest 5 / rquest-util 2`、Rust `1.95.0`；候选为
`wreq 0.16.1 / wreq-util 0.2.0`、Rust `1.98.0`。

新 wire 合同确认原生 wreq Windows 自动代理将明确空 `NO_PROXY` 与未设置
混淆，回填注册表 bypass，造成请求绕过测试代理并超时。旧 rquest 仅在
NO_PROXY 不存在时读取 Windows bypass，并非完全忽略系统例外。另有环境/
系统逐协议混合及 CGI 整体禁用的来源差异。原候选失败日志保留为
`candidate-wire.log`，没有将其改写为成功。

本轮联网读取官方 sparse index，最新发布仍为 `wreq 0.16.1` 和
`wreq-util 0.2.0`。没有假称存在已修复新版，也没有关闭 system-proxy feature、
改 fixture bypass 或修改真实代理设置来绕过回归。

采用小型 Gateway factory 和代理来源模块，不维护依赖 fork，不建立大型
Client facade。修复的是 Gateway 默认构建入口，不是原生 wreq 默认行为。
永久合同与后续调用约束见 [HTTP 客户端代理策略](../http-client-proxy-policy.md)。

## 实际接线与源码变化

- `src/http_client.rs` 提供默认 builder/client；生产 12 个 builder 和两个
  Client::new 调用统一接线，示例 producer probe 同步采用默认入口。
- `proxy_policy.rs` 保留有效环境值的大小写优先、协议覆盖 ALL、CGI 跳过
  HTTP 而保留 ALL/HTTPS、整个环境 map 为空才回落 Windows 手动代理。
  Windows 使用只读 open，严格 `ProxyEnable == 1`；bypass 独立读取。
- 明确空 NO_PROXY 阻止系统 bypass；保留旧 normalize、地址首次 LazyLock
  缓存和每次构造读取 bypass。没有输出或持久化真实代理地址/凭证。
- factory 不改变 profile、timeout、redirect、header 或 retry。显式 proxy
  必须先 `.no_proxy()` 清空默认列表；当前生产没有此类额外配置。
- 根 manifest 使用显式、关闭默认 feature 的 wreq aliases；根 lock 由官方
  Cargo 生成并原样复制，没有手改版本、checksum 或 resolution。
- 根工具链、三个 CI/build workflows、Dockerfile、Dockerfile.dev、README
  和对应契约同步为 `1.98.0`。Desktop/local-data detached lock 与低 MSRV
  子 crate 的 manifest 不刷新，不新增 gateway-sqlx 独立 lock。
- 接入已审阅的 URI API、公开 HttpBody 测试和 TLS Profile 适配；安全 Debug
  不打印 HTTP clients、browser executor 地址或 bearer，补充合成 secret 回归。
- 新 Python 契约验收旧/新完整配置窗口；生产未切换时不留下硬编码新版本的
  红测试。Profile 契约仍含负向 mutation，兼容新的 factory 入口。

初始 fixture 的 redirect 默认策略及 Windows 大小写环境键问题已按真实机制
修正；失败日志保留。八个代理父测试逐个启动 ignored 子进程入口并验证 marker
及 `1 passed`，不是缺失业务覆盖的 skip。请求头 8 KiB、Body 16 KiB，deadline、
kill_on_drop、CREATE_NO_WINDOW 和 awaited join 保护清理，没有 detached server task。

## 已运行门禁

Cargo 全部串行、单 job、隔离 target/TEMP/data，生产切换后的 Rust 构建使用
隔离 Rustup `1.98.0`；没有更新正常 Rustup home。除首次必要的官方 lock
resolution 外，编译和 Rust 测试均使用 `--offline --locked`。

| 门禁 | 实际结果及回执名 |
| --- | --- |
| 旧客户端扩展 connection/proxy wire | 12/12；`old-wire-proxy-expanded` |
| 候选 Gateway 默认入口 wire | 12/12；`candidate-wire-proxy-compat` |
| 候选最终代理来源/系统 socket 单元 | 6/6；`candidate-proxy-unit-final` |
| 生产聚焦单元 | 355/355；`production-focused-unit` |
| 生产 11 个 integration targets | 69/69；`production-integration-final` |
| 生产默认 feature 全目标编译 | 退出 0；`production-default-all-targets` |
| 生产关闭默认 feature 全目标编译 | 退出 0；`production-disabled-all-targets` |
| 生产 formatter | 退出 0；`production-format-final` |
| Python HTTP/profile/standalone/Docker/AWS/SQLx 契约 | 53/53，无 skip；`production-python-contracts` |
| checker 测试与 ratchet | 31/31、退出 0；`production-checker-tests`、`production-line-ratchet` |
| Security CI/release 契约 | 15/15；`production-security-contracts` |
| 三个 workflow actionlint 语法 | 退出 0；`production-workflow-syntax` |
| Neuro 通用规范契约 | 退出 0；`neuro-standard-contract` |
| 完整生产根 lock 审计 | 452 包、漏洞 0、warnings 空；`production-root-audit-receipt.json` |
| 回退演练 | 2/2；部分恢复、重试、外部编辑拒绝，未回退生产 |

生产 integration 分布为：Bedrock 13、Body 5、分类 4、TLS 3、network timeout 1、
pipeline send 7、typed SSE 4、stream observation 13、observation limits 7、
connection 4、proxy 8。生产 unit 包含前阶段 291 个相关测试及新代理、Debug、
OAuth、quota、Splitter 和直接相邻 owner。没有执行完整 Rust runtime suite。

两个全目标门禁针对真实生产 checkout，包含候选缺少的历史 Jina 示例；是
Windows Rust targets 编译，不是 Linux/macOS 交叉平台 runtime 验收。
关闭默认 feature 的门禁是编译，不冒充该配置下的完整 runtime 测试。
Compiler/linker 仍有既有 warnings；审计 warnings 空不表示编译零 warning。

## 官方公告与输入保护

最终于 `2026-10-01T10:11:24Z` 联网复核官方 RustSec main，与新建 clean 数据库
同为 `3461c0d8f85d084552dd999c58d97c7123a9e0fd`，没有修改旧公告缓存。
于 `2026-10-01T10:11:26Z` 对生产根 lock 执行无 ignore、无 target 过滤的
`cargo audit --no-fetch --deny warnings --db <verified-db> --file <root-lock> --format json`。
根 lock SHA-256：
`38644dff07c6616c6d81d3da8a8b95ff3dcc3c366aaeba6664bc9e36d5e0c1ac`。

本轮起点 3,782 个原输入中仅 38 个授权 owner 改变，3,744 个原输入保持 hash；
没有缺失或非授权改动。新增十个限定 source/test/document 文件，明细见
`final-input-preservation.json`。两份 detached lock 原字节保持。
Responses → OpenAI 的 500 行状态机未改，不更新行数策略、baseline 或例外。
Gateway 与 Neuro 的 diff check 均通过；两仓原有 dirty 状态保留，不提交/推送。

所有新增和实质修改源码不超过 500 有效行，UTF-8 无 BOM。关键 owner：
factory 9、代理模块 179、代理单元 177、Debug 43、proxy 合同 169、connection
合同 169、Python cutover 契约 90。最高的相邻 standalone 契约仍为 484，
Canvas send 为 463，无新大小例外。

## 回退、交付和未验收边界

证据根目录：
`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-http-cutover-20261001`。
含原字节 snapshot、全输入 hash、官方 index/公告核实、失败/成功日志、每项
回执、计划/落地 hashes、最终输入保护和 `rollback.py`。回退默认仅预览；
真正恢复须原字节/新字节 hash 相符，拒绝覆盖后续编辑，支持部分恢复和重试，
保留新增但不再引用的文件，不删除 linshi 外内容。本轮只预览、演练，没有执行回退。

本轮没有调用真实提供方、使用真实 token、改 `.ng`/数据库、改 Windows 代理，
也没有创建网络盘符、停止其他构建、部署、commit/push 或生成/替换正式 release。
`wreq-rt 0.2.2-rc.4` 仍是 prerelease，不得称为全 stable 图。HTTP/2 SETTINGS、
完整 router/provider、Linux/Docker runtime、Desktop 二进制及正式发布仍未验收。
macOS 的原生系统代理委托路径也不宣称与旧客户端完全等价。

只读发现未触及的 `examples/jina_probe.rs` 含疑似硬编码 API 密钥。本轮未打印到
交付文档、未复制到候选、未联网验证或调用，也未擅自修改/轮换。正式公开发布
前须单独清理源代码与评估历史暴露、轮换；Cargo 依赖审计不能覆盖这类秘密问题。
