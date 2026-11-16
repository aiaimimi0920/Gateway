# Grok 可选自动 loopback 回调

## 范围与实现

本批补齐 Grok Build 代码授权的本机回调接收入口，不新增模板、不借用官方客户端身份，不改变手动 callback 或设备授权默认行为。

- 管理 create 增加显式 `authorizationFlow: "loopback"`，沿用管理身份、Secret Grant、原 revision、provider/credential 校验。
- 数值 loopback 地址、非零显式端口、精确 `/callback`；使用 Authorization 的规范化 redirect，不进行 DNS 解析、不换端口，绑定失败不注册会话。
- 独立有界 listener JoinSet，最多 16；创建与 shutdown 共用注册锁，join 等待任务退出。单 listener 串行有界读头，不建立每连接后台任务。
- 复用共享 HTTP parser 和既有 `Session::complete` 单次交换；错误 state 继续等待，用户拒绝结束，浏览器断开不取消已接纳交换。
- 自动回调只到 Ready；正常 UI 隐藏手动 callback 输入，仍要求明确保存。新增可选回包字段兼容旧手动会话。
- 同步登录合同中过时的 device/listener 缺口描述，未改其他渠道或 S06。

## 验证

证据目录：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-grok-loopback-20261112`。

- 前端新行为先红：缺少自动回调选项，1 failed。随后首次绿测发现新夹具遗漏取消 action 的 Promise，已仅修正测试夹具。
- 前端相关 6 文件 45 项通过；补充 API 自动模式兼容测试后，该 API 文件 12 项通过。TypeScript typecheck 通过。
- Rust 首轮 88 passed / 1 failed：新成功路径夹具误用了设备流可接受的 opaque access token，代码流按既有身份规则拒绝。已按同仓代码流夹具补入 Team principal hint，不放宽生产校验；最终 **89 passed / 0 failed / 0 ignored**，四项新增 socket 合同全部通过。
- 非测试生产 `cargo check --locked --offline --lib --no-default-features --features runtime-defaults -j 4` 通过，111 条既有 warnings，未顺带清理无关代码。
- 首轮测试命令含共享编译锁等待，总计 23m23s；修正夹具后增量测试编译 10m06s、测试 29.08s；生产 check 4m50s。仅记录本次实测，不承诺构建倍数。
- checker 测试 31/31；ratchet 通过，5488 文件，governed source >700=0；rustfmt 与范围 diff whitespace 检查通过。
- 只读独立审查未发现已确认的生产阻断或权限旁路；按建议增加自动交换期间手动 complete 拒绝、上游仅一次请求的离线验证。
- `source-hashes.json` 的本批十个源码/测试文件 SHA256 在验证完成后全部匹配。未冻结其他任务的共享 dirty 工作树。补做未跟踪文件的独立 whitespace 检查通过；最初检查包装器误把 `git diff --no-index` 的正常差异 exit 1 判成失败，纠正判据后无 whitespace 报错。

## 增量行数与审查

`registry.rs` 214→276、`session.rs` 265→269、`mod.rs` 70→73、管理 route 193→207、UI 79→80、API 24→25。新增 listener 101、Rust socket 测试 248、前端 UI 测试 33 有效行。没有扩大超限文件或改变 baseline/policy。

审查覆盖：管理与 OAuth 授权分离、精确回调地址及有界请求、秘密不回显、容量/TTL/取消/关停任务所有权、一次性交换、浏览器断开、无自动持久化。数值 IPv6 分支及 16 listener 并发极限目前为代码审查边界，未单独做真实 socket 验证。

## 未宣称的完成项

未调用真实 xAI/Grok、未检查第三方 client 使用许可、未读历史凭据、未修改数据库或运行实例、未提交推送、未重新打包。旧 `gateway-devtest-provider-expansion-20261112-132816` 不含本批修改。本批不是 Grok 全渠道、P50/P60/P70 或整个扩展计划完成。
