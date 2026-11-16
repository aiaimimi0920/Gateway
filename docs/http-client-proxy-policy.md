# Gateway HTTP 客户端与默认代理合同

根 Gateway 使用 `wreq 0.16.1 / wreq-util 0.2.0`，保留源码中的
`rquest`、`rquest-util` consumer aliases。工具链、根 manifest、CI 和 Docker
构建入口统一为 Rust `1.98.0`。Desktop 的独立 reqwest 客户端和 detached
锁文件不属于这次替换。

## 默认构建入口

生产默认客户端统一经过 `crate::http_client::builder()` 或
`crate::http_client::client()`。该入口只配置代理，不增加 TLS emulation、
header、redirect、整体 timeout 或 retry；这些仍由原调用方负责。

例如 Upstream 的 Chrome136 与 plain client、Splitter 的 Chrome131 profile
仍分别保留。Splitter streaming proxy 仍不继承 readiness 的短整体 timeout。
Profile 名称和 ClientHello 测试不等于完整 JA3/JA4、HTTP/2 SETTINGS 或真实
提供方的 fingerprint 验收。

代理选择由 `src/http_client/proxy_policy.rs` 维护。它使用客户端公开 API，
不 fork 依赖，不修改进程环境或 Windows 设置。管理界面可持久化本网关的默认出站策略。

## 管理界面的出站设置

“设置 / 网络 / 代理”提供自动、关闭、自定义。自动沿用下述发现规则；关闭不使用
默认代理；自定义仅接受无账号密码的 HTTP(S) 代理 origin，不支持 SOCKS、路径或
查询参数。自定义绕过 localhost、127.0.0.1、::1，不继承环境代理及 NO_PROXY。
策略保存在 console state 目录的 `outbound-proxy.json`，保存后重启该网关生效，
响应的 `restartRequired` 标识待生效状态；已有连接池不热切换。

`GET/PUT /v1/internal/gateway/console/proxy/settings` 与
`POST /v1/internal/gateway/console/proxy/test` 均要求管理权限并遵守远程访问策略。
测试仅用未保存草稿访问固定 HTTPS 测试站点，不保存设置、不调用模型、不携带
服务商凭据。测试最多两个并发且有连接/总超时；不能代表每个服务商均可访问。

默认 Rust HTTP、native Codex 和 native Antigravity 传输采用该策略；调用方显式
指定的代理或 `.no_proxy()` 保持优先。独立浏览器 worker 的代理参数不在此范围。
桌面连接探测和浏览器访问网关不受此设置控制。

桌面的“网关地址”支持本机端口和远程 HTTP(S) origin；本机端口改变只重启客户端
持有的 desktop-managed 进程，端口被其他实例占用时拒绝操作，不接管该实例。
连接成功才保存并打开界面；保存或打开失败尝试恢复先前设置和自有进程。
旧 connection.json 缺少 localPort 时沿用构建默认端口，不迁移账号数据库。

## Windows / Linux 来源规则

代理地址在进程首次构造默认客户端时通过 `LazyLock` 冻结，与旧 rquest 的
缓存边界一致。更改代理地址环境变量或系统地址后需要重启进程；bypass 则在
每次构造客户端时重新读取。已构造的客户端不会被后续 bypass 变化改写。

1. 有效 `ALL_PROXY` 优先于 `all_proxy`，默认同时用于 HTTP、HTTPS。
2. 有效 `HTTP_PROXY` / `http_proxy`、`HTTPS_PROXY` / `https_proxy` 分别覆盖
   ALL；大写值无效、空或无法读取时，继续尝试小写。协议变量无效不清除 ALL。
3. `REQUEST_METHOD` 存在时，忽略大小写 HTTP_PROXY，防止 CGI HTTPoxy 注入；
   ALL 与 HTTPS 代理仍可使用。不能将 CGI 误实现为全部禁用代理。
4. 仅当整个有效环境代理 map 为空时，Windows 才回落 HKCU Internet Settings
   的手动代理。`ProxyEnable` 必须等于 `1`；只配置环境 HTTP 时，不补系统 HTTPS。
5. Linux 无 Windows 系统回落。当前只启用 HTTP/HTTPS 代理，不支持 SOCKS。

Windows `ProxyServer` 支持旧分协议规则：`http=host:port;https=host:port`，
缺连接 scheme 默认 HTTP；重复协议以后项为准。列表结构错误清空整个 map，
某项地址错误只忽略该项。无 scheme 的单值同时用于 HTTP/HTTPS；显式
`http://...` 单值按旧合同仅用于 HTTP 请求。协议字段不 trim 或 lowercase。

代理 userinfo 支持 Basic auth；缓存里可能保留认证信息，但不会为代理配置
派生 `Debug` 或输出地址、用户名、密码。解析失败不打印原输入。

## bypass 的存在性不是空值判断

`NO_PROXY` 可读取时优先于 `no_proxy`，**明确空值也算存在**。因此：

- `NO_PROXY=""`：没有 bypass 项，不读取 Windows `ProxyOverride`。
- 未设置两种环境变量：Windows 独立读取 `ProxyOverride`，不依赖代理地址来源
  或 `ProxyEnable`；因此环境代理也可能使用系统 bypass。
- 非空环境 bypass：只使用该值，不混入系统 bypass。

系统 bypass 保持旧 normalize：分号切分、trim、逗号拼接、移除 `*.`。
不新增 IPv4 wildcard 到 CIDR 的转换，也不将 `<local>` 当作额外 WinINET
规则。底层 NoProxy matcher 仍归 wreq 所有，不能声称所有特殊输入完全等价。

原生 wreq 0.16.1 的 Windows 自动代理会把明确空 NO_PROXY 与未设置混淆，
并可能逐协议混合环境/系统地址；Gateway factory 先关闭该自动选择，再添加
确定的分协议代理。**修复的是 Gateway 默认行为，不是原生 wreq 默认行为。**

## 显式代理与平台边界

需要自己的代理时，必须先清空 factory 默认代理，避免前面的默认 matcher
优先拦截：

```rust
let client = crate::http_client::builder()
    .no_proxy()
    .proxy(rquest::Proxy::all("http://127.0.0.1:8080")?)
    .build()?;
```

`.no_proxy()` 仍完全禁用默认代理。测试用 loopback 明确使用该调用时，不构成
默认代理发现的验收。当前生产调用点没有额外显式代理配置。

macOS 无环境代理时委托 wreq 原生平台实现。本轮没有验收 macOS 代理来源、
缓存、明确空 bypass 的旧语义，也没有 Windows PAC/自动配置验收；不要把
Windows 实测和 Linux 可编译设计外推为所有平台通过。

## 回归与发布边界

聚焦合同包括：

- `http_client::proxy_policy`：纯来源解析与合成系统代理真实 socket；
- `http_client_proxy_contract`：隔离子进程的默认环境代理、bypass、ALL/HTTP、
  CGI、认证、显式禁用；内部 ignored 入口由每个父测试实际执行；
- `http_client_connection_contract`：连接复用、307 POST Body、跨 origin
  敏感头删除、默认及请求级 redirect 策略；
- `test_gateway_http_client_cutover_contract.py`：旧/新完整配置窗口和生产接线；
- `test_gateway_http_client_profiles.py`：profile、timeout 与负向 mutation。

根锁图变更必须用官方 Cargo 生成并运行完整根锁图审计。当前
`wreq-rt 0.2.2-rc.4` 仍是 prerelease，不能称为全 stable 依赖图。
源码切换、聚焦测试和全目标编译不等于正式 release、Docker/Linux runtime、
完整 router/provider 或在线提供方验收。实际阶段证据见
[源码切换检查点](status/2026-10-01-http-production-source-cutover.md)。
