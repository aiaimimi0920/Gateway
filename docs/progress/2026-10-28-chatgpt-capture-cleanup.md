# ChatGPT 网页登录读取与清理超时修复

## 现场与归因边界

当前运行包 `gateway-devtest-20261028-210345-cookie-persistence`，用户正常登录后关窗，
自动重开读取窗口，随后显示“网页登录等待超时”。实际管理请求 503，总耗时 71749 ms；
同时间有 scratch 清理待重试日志。旧日志没有阶段信息，不能精确反推这一次发生在
CDP endpoint、具体读取命令，还是进程清理，也不能把 72 秒解释成用户未完成登录。

源码与修复前失败测试证明了两个确切缺陷：

1. 单条 CDP 命令超过 20 秒返回通用 `browser_timeout`，不在读取重试白名单中，
   会提前关闭已登录浏览器，而不是继续使用 60 秒捕获预算。
2. `finally` 中的清理异常可覆盖已经捕获成功的材料，或覆盖原有读取错误。
   Windows 原来的 taskkill 等待仅 3 秒，且只采用强制关闭。

此前真实 Chrome 测试仅验证持久 Cookie 与 debugger ready，未覆盖实际
`Runtime.evaluate(pageSession)`、`Network.getCookies`、重试和完整清理。

## 修改

- CDP 超时使用 `session_read_timeout`，在当前登录目录与预算内重试。
- 初始登录等待、捕获窗口就绪、会话读取、整体预算使用不同的固定错误码与提示。
  Rust 仅记录固定阶段，不记录页面内容、账号材料、Cookie 或 token。
- 新增凝聚的 cleanup owner：已验证的专用 browser endpoint 先执行 `Browser.close`；
  失败时才终止本次拥有的进程树。taskkill helper 自身有界且被回收，清理总预算 14 秒。
- 清理失败保留读取结果/原错误，输出布尔 `cleanupPending`；管理 worker 的父级
  Job Object 与 canonical workspace owner 仍负责最终回收。已知进程仍活着时不删目录。
- 第一阶段人工/Google 登录仍不启用 CDP，不读取日常浏览器，不改变 Cookie 路径修复。

## 验证

修复前新增三项保护测试全部失败：读取超时不重试、成功结果被清理异常覆盖、原读取错误被覆盖。
修复后 Node 定向 38 项通过。新增清理测试覆盖优雅关闭、强制关闭回退、保留活进程目录、
清理错误不回显 OS 细节；真实 WebSocket 测试覆盖读取专用超时码。

真实 Chrome opt-in 测试现已扩展为：合成持久 Cookie 正常关窗重开、通过 CDP 拦截提供
完全合成的同源页面与会话响应、执行产品完整会话读取代码、故意制造首次 CDP 超时并重试成功、
读取 synthetic HttpOnly Cookie、真正 `Browser.close` 正常退出且强制终止次数为零。
它不使用真实账号、不访问真实 ChatGPT 会话接口、不调用模型。

Windows Job Object 环境首轮曾出现清理待父级接管，读取结果已保留；测试 teardown
原先遇到清理异常会跳过 fixture/server 关闭，导致测试宿主残留，现改为 finally 回收。
补充逐阶段诊断连续五轮正常；随后未插桩产品代码的同环境完整测试连续三轮通过。
Rust `chatgpt_web_browser` 定向 3 项、Python 包合同 2 项、checker 31 项及 ratchet 通过。
`cargo fmt --all -- --check` 与 `git diff --check` 通过。真实账号人工验收仍未执行。

构建/包内测试回执放在
`linshi/gateway-chatgpt-native-package-20261028/capture-cleanup/`，
源码定向检查放在同父目录 `capture-cleanup-tests/`。

这些是已证缺陷修复与合成完整流程验证，不等同于真实账号人工验收。
当前 Gateway、4200、账号数据库不改动，不自动启动新包接管服务。
