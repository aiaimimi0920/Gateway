# Codex CLI 直发与 Gateway 实际对比

## 方法与边界

使用真实 Windows Codex CLI 0.155.1、合成密钥、隔离 HOME/工作区和回环接收端。
代理只允许注册的回环端口；不访问真实服务商，不使用真实账户，不更改系统信任。
先实际运行 CLI 直发、续聊、经 Gateway 发送、经 Gateway 续聊，再把捕获到的同一原始请求
重放给 Gateway，逐字节比较接收端得到的完整 HTTP 请求。保留所有差异，不排序或剥离头。
两次独立 CLI 会话有不同的消息/会话/安装 ID、时间戳和隔离 HOME 的 skill 路径，
其请求不能直接宣称字节相同。相同输入回放用于分离 Gateway 引入的变更。

另通过真实 CLI 的 provider `http_headers` 显式设置五个已知可选协议字段。
该用例证明 CLI 实际发送后的保真，不声称实际运行了子代理或记忆生成任务。
字段依据包括固定版本的
[`core/src/client.rs`](https://github.com/openai/codex/blob/rust-v0.155.1/codex-rs/core/src/client.rs)。

## 发现与修复

修复前 EXE SHA256：`aeeacd5810efba487ba2fbcf0c5f35cdc8ed8674b979534f81bb5360e1304317`。
主请求和续聊回放已相同，但扩展请求 body 相同、headers 不同，缺失：

- `x-codex-parent-thread-id`
- `x-codex-routing-hint`
- `x-openai-memgen-request`

实际 CLI 经 Gateway 的独立运行也观测到同样三项缺失。上一轮修复的 subagent/residency 正常保留。
本轮将这三个明确的原生协议头加入固定白名单，并扩展顺序、缺失不补、长度/CRLF 及凭证隔离回归。
不放开任意头，不绕过认证或服务商显式覆盖，不将路由提示当作 Gateway 控制指令。

## 修复后实际结果

重新构建的 EXE SHA256：`5d4e8106308cfb874f060eebec30fba18c65fce3fb27e24cc6991be9a9916ca9`。
CLI SHA256：`eba0f32c976667cb9298efafd98513e823eeda7b576a03ec658bb8be8d336316`。

直发/经 Gateway 的 exec、resume、显式可选头请求共六次 CLI 调用均 exit 0。
完整 HTTP 请求的六组受控比对全部一致：主请求、续聊、重复 query、可选协议头、
TUI 主请求和 TUI 空 tools 的 title 请求。TUI 两条链路各捕获两个请求后由 harness 关闭，
不把人为关闭解释成自然退出验收。

TLS 另测三对首次 ClientHello：每条 165 bytes，只清零 32 bytes 的 random 后整条 record 一致。
接收端仍在 ClientHello 后主动断开，未完成完整 HTTPS 握手及数据交换。
不能把上述结果外推为所有版本/OS、WebSocket、完整连接生命周期、异常重试或任意上游均不可区分。

## 证据与检查

证据根目录：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-codex-live-20261020/`。
`before/verification.json` 保留三个头丢失的反例；`after/verification.json` 包含修复后的精确比对
及独立会话差异；`http-verification.json` 包含常规/TUI 的五组精确比对；
`tls-after/verification.json` 包含 TLS 结果及 binary hashes。原始 HTTP、body、响应和 CLI 日志均保留。

源码回归：`cargo test --locked --offline --lib native_ -- --test-threads=1` 48 passed；
`cargo test --locked --offline --lib protocol::codex_wire::tests -- --test-threads=1` 2 passed。
`cargo build --locked --offline --bin gateway` 通过，存在未改动区域的 warnings；
checker tests 31 passed，ratchet、formatter 和 `git diff --check` 通过。
本轮未重跑全量测试。测试后核对 EXE hash 未变化，本轮 Gateway/CLI 进程残留为零。
Rust 构建输入清单为 `source-manifest.json`，1840 个文件，聚合 SHA256：
`2dfd578653c4f29aa6c67c6e2e7a17e440442454c01547a96eaffd7bc7570177`。
有效行数：协议模块 90 → 99，入口测试 92 → 98，sender 测试 227 → 236。
未修改行数 baseline/policy/exception；未提交、推送、部署或重启真实实例。
