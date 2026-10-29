# ChatGPT Web 普通文本与账号会话兼容性修复

## 范围与依据

参考项目已联网核实并固定到 yukkcat/chatgpt2api 的
`d58db042411f512449dd90e3b7a73de79ab38c46` 和 basketikun/chatgpt2api 的
`92dd80e91543157d36a4ddad60e765c5293760e9`。对照及原始来源哈希位于
`linshi/gateway-chatgpt2api-review-20261029/`。本补丁按本项目接口独立实现普通
HTTP 协议与会话兼容，不导入或执行参考项目，不复制其挑战求解、验证 token 构造、
反检测或网络出口切换实现。

此前目标账号对 `gpt-5.6-sol` 的真实调用在 requirements 阶段收到 HTTP 403，
尚未进入生成。本补丁修复普通请求与目录链路中另行证实的问题，不能作为该 403
已解除、模型已获权限或真实调用已成功的证据。

## 改了哪些内容

- 普通文本默认使用 `/backend-api/conversation`，不再自动进入旧 f/prepare 流程。
  显式路径覆盖优先；缓存 f 材料、显式旧 f 模式及原有布尔开关仍可选择旧路径。
- 普通请求生成独立 UUID `parent_message_id`，使用普通 SSE/历史关闭契约；
  旧 f 请求体单独保留其已捕获契约，流式和非流式按实际路径选择构造器。
- 原样传递调用者显式模型，包括 `gpt-5.6-sol`。新建网页预设的后备模型为 `auto`，
  不改存量显式模型，不建立未经验证的模型别名，不伪造上游目录或生成成功记录。
- 新增请求链局部 Cookie owner：同一账号的首页、requirements 和普通 conversation
  接收成功响应的 Cookie 更新；共享 HTTP client 不共享 Cookie，不写持久凭据。
  新调用重新导入保存材料，不继承上一次调用的临时更新。
- Cookie 按 origin、Path、Secure、expiry、删除与 prefix 校验；同名不同路径保留并按
  长路径优先发送。原始 Cookie header 无法恢复重名 Cookie 的作用域，因此明确拒绝
  重名导入，而不是静默覆盖。单项 8 KiB、64 项、总计 32 KiB；每响应最多处理前
  64 个 Set-Cookie，超限材料不截断或取代已有合法状态。
- 模型目录先初始化同一账号首页，再沿用更新后的 Cookie 请求固定官网目录；
  不要求首页存在 proof 脚本，不调用 requirements/solver。目录仍只接受白名单身份
  材料并强制 Bearer，不接受持久 Authorization、Host、路径或其他 header 覆盖。
- 首页、目录及发送请求禁止重定向；首页错误保留阶段、状态和分类，不回显 HTML 正文。
  目录失败不覆盖旧模型列表，目录声明仍不等于真实生成验证。

兼容例外：旧 `/backend-api/f/conversation` 及其 prepare 路径继续执行原有 header
剥离规则，不发送 Cookie/Origin 等字段。bootstrap metadata 缓存命中仍不额外访问首页。
Cookie owner 仅接收成功 HTTP 状态的响应，刻意不实现完整浏览器的失败响应 Cookie 行为。

## 验证与证据

证据目录：`linshi/gateway-chatgpt-web-compat-20261029-023613/`。

- 修复前定向回归：202 passed / 5 failed；失败命中普通默认路径、UUID、目录初始化、
  流式/非流式普通流程及预设后备模型的新断言。
- 修复后首轮完整执行为 214 passed / 1 failed。失败来自新增隐私断言的夹具：
  无效输入 `invalid` 与通用错误里的同一单词重合。已改用唯一私密标记，不改生产逻辑；
  `cookie-focused-red.log` 保留单独复现。
- 最终 `cargo test --locked --offline -j 2 --lib chatgpt -- --test-threads=2`：
  215 passed / 0 failed / 3687 filtered out，见 `green-r3.json` / `green-r3.log`。
  本次编译 15 分 36 秒、测试 145.60 秒；不作为其他构建的耗时承诺。
- 相关 21 个 Rust 文件 scoped rustfmt、有效行 checker tests（31 passed）、最终 ratchet
  均已通过，分别见 `fmt-final.json`、`checker-tests-final.json`、`ratchet-r3.json`。
  最终源码哈希、UTF-8 无 BOM、文档与 working-tree diff 检查由 `final-audit.json` 汇总，
  以成功回执为准。
- 新增 loopback 回归覆盖共享 client 并发账号隔离、同链 Cookie 更新、跨调用重置、
  流式/非流式、首页重定向不跟随、目录首页拒绝停止、身份 header 白名单；
  Cookie 单元测试覆盖路径身份、删除、origin/prefix、重复导入与容量边界。

`before/` 保存本轮开始时已包含其他修改的文件；`scoped-before-after.diff` 是本轮
精准差异，`after-final.json` 记录有效行与 SHA256。21 个受影响 Rust 文件均低于
500 有效行，最大 435，无需例外，不增加历史超大文件债务。

## 安全、性能与交付边界

Cookie Debug 固定脱敏，新增错误不输出 Cookie、token 或响应正文；origin 校验在发送
认证材料前完成。锁只保护请求局部的有界 CookieStore，不跨 await；没有新增后台任务、
重试队列、全局缓存或持久化格式变更。沿用锁定依赖，Cargo.lock 未因本补丁变更。

本轮未修改在线 Gateway 的进程、配置、凭据、数据库、网络或浏览器，未重启、关闭或
替换在线实例，未执行新的真实外网生成调用，未打包新 EXE。已有旧包不包含此补丁，
不得作为本轮产物交付。

未完成项：新多阶段 requirements 协议适配未实施；目标账号 403 尚未取得解除证据，
`gpt-5.6-sol` 的真实有效回答验收仍未完成。本地编译/fixture 通过不能代替该业务验收。
