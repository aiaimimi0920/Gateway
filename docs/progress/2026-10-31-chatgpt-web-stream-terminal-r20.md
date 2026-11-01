# ChatGPT Web 流式终态修复与独立手测包检查点

## 当前结论

本次关闭的是已证实的流式假成功缺陷，不是当前账号真实调用验收。
上一真实请求仍停在 `stage=bootstrap / HTTP 403 /
chatgpt_web_browser_challenge_required`；没有得到模型目录或生成成功证据。
没有把已保存的登录材料、历史成功、本地 HTTP 200 或 loopback 通过当成真实账号成功。
没有替换 `gpt-5.6-sol`、切回 `gpt-5.4` 或给不同模型标识擅自建立别名。

## 已改内容

- 将流式译码提取到 `response/stream.rs`，复用有界 SSE decoder，处理分片 UTF-8、
  BOM、CRLF；不把未闭合的 SSE frame 当作完成信号。
- buffered 与 streaming 共用失败和完成判断：正文后出现 failed、error JSON、
  moderation blocked 或缺少终态的 EOF 都不能产生成功终态；partial 保留为 `length`。
- 流式协议失败使用 `StreamError::Protocol`，网络失败保留 `StreamError::Transport`。
  不把上游 error JSON/HTML 放入错误消息，错误只使用内部固定 code/message。
- 完成、协议失败、decoder 失败或网络失败时释放源流及累计缓冲；收到完整 DONE 后
  不继续读取上游。消费者取消后由流对象所有权释放资源，无新增任务或后台重试。
- HTTP 流与三个现有 browser relay 包装点改接 `NativeEvents`。
  仅改类型接线，没有新增 browser recovery、挑战处理或重试分支。
- 对 ChatGPT Web 的 NativeEvents 保留模型观测 tap，仍标为
  `synthetic_or_unobserved`，不将输出中的请求模型名冒充上游模型声明。
- 同步凭据测试文档，说明上一检查点新增的 ChatGPT Web 显式 HTTP 提示词测试。

## 失败先行与最终验证

证据目录：

```text
C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-chatgpt-stream-20261031-075000
```

四项新增 translator 回归先运行：`0 passed / 4 failed`。
原行为会在截断及空响应后追加 stop/DONE、把 partial 标为 stop，并丢失 BOM 首帧文本。
原始回执保留在 `stream-red.log`。

首轮新集成测试另因合成 session 缺少到期信息而提前进入既有会话刷新 worker，返回
`503 / chatgpt_web_browser_probe_failed`，未进入预期 HTTP 流；该失败保留在
`integration-first-failure.log` 与 `integration-diagnosis.log`，不能作为无浏览器路径通过证据。
补齐合成到期时间、配置禁止请求期 browser fallback 后，在 runner 另显式设置
`GATEWAY_REQUEST_TIME_BROWSER_POLICY=disabled` 重跑。没有为通过测试修改 production
会话刷新策略，也没有借此执行在线浏览器验收。

最终实际通过：

- 五组 integration suites 共 **10 项通过**，含新的完整 ingress 合同和四组既有 probe 合同。
- `cargo test --locked -j 4 --lib chatgpt -- --test-threads=2`：**230 项通过**，
  3,799 项过滤；不是全量测试。集成与库测试串行执行，最终命令退出码 0。
- 六个新增 translator 单测覆盖 failed、EOF、error、moderation、成功与 partial、空响应、
  未闭合终态、分片 UTF-8/BOM/CRLF、Transport variant、lazy polling、完成及首包后取消释放。
- 新 ingress 合同经真实 router、路由与本地 SQLite，覆盖 ChatCompletions、Responses、
  Messages、Legacy Completions 的正文后失败，断言 body error、无成功终态、失败 audit、
  上游错误私有 marker 不泄露、synthetic model observation，以及 HTTP 链恰好四次请求。
  ChatCompletions 另有正常完成对照。所有模型及账号均为合成 loopback fixture。
- effective-line checker **31 项通过**；最终 ratchet、scoped Rust 2021 rustfmt 和
  `git diff --check` 通过。最终 hash、UTF-8/no-BOM 与有效行数见 `source-measurements.json`。

有效代码行：`response.rs` 445 → 327；新 stream 150、新 stream tests 183、新 ingress
合同 218；`stream_preflight.rs` 251 → 262。其余接线文件不增长或只增加 trait import。
所有本次新改 Rust 文件均低于 500 行；未修改 policy、baseline 或 exception。

## 交付与现场保护

指定独立桌面手测包版本：`gateway-devtest-20261031-chatgpt-stream-r20`。
使用 `tools/package-gateway-devtest.ps1 -Edition desktop -Jobs 4`，不覆盖旧版本。
本文件是构建前冻结的源码检查点；是否完成打包、产物 hash、耗时和原生运行结果，以
证据目录的 `package-result.json`、`package.log`、`native-sqlite-smoke/result.json` 为准，
不能仅凭预定版本名判定已有可交付 EXE。

原生运行使用 `smoke-gateway-local-storage.ps1` 的新临时目录和随机 loopback 端口，
只验证后端本地启动、持久化及退出，`modelCalls=0`；不启动 Docker/Redis 或用户桌面窗口。
没有替换、关闭或重启现有 4200，也不更改在线凭据、配置、数据库或代理。
“请求恢复等待”owner 的文件未改动；保留工作区其他 dirty edits，未提交或推送。

## 未验证与仍未完成

- 当前 `vmjcv111 / gpt-5.6-sol` 的真实生成仍未验收，bootstrap 403 未解除。
- 三个 browser relay 分支只有静态接线审查，没有在线浏览器运行证据；CUA 曾报告
  无法确认 URL 的安全停止，本轮没有换 CDP、Playwright 等入口绕过。
- Pending 中取消及失败返回前 source Drop 的专门计数测试未补；首包后取消和完成释放已测。
- 四入口 ingress 对 error JSON 的完整链已测；EOF、moderation、partial 的覆盖在 translator
  单测层，未为所有入口新增组合矩阵；injected-tools 组合未另加动态用例。
- 不执行正式发布安全审计或全量回归，不据手测包/原生 smoke 宣称 Web 业务完成。
