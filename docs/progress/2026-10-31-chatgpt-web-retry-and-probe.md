# ChatGPT Web 重试：当前拒绝阶段与手动测试入口

## 本轮真实运行证据

2026-10-31（Asia/Shanghai）读取在线 4200：`healthz`、`readyz` 均为 200。
在线目录为 `gateway-devtest-20261031-gemini-dual-mode-r19`，配置 revision 为
`r38-7f9e66c08496`。没有关闭或重启该实例，也没有修改其配置和登录材料。

`chatgpt-web / vmjcv111` 已启用；管理接口的脱敏描述确认 Bearer 与 Cookie 均已配置，
保存的到期时间为 `2026-11-07T14:52:34.000Z`。这说明账号材料已经导入，
不是账号没有显示、没有保存或要求用户再次证明完成登录；也不证明上游当前接受该材料。

本轮只执行一次该账号的模型目录读取，结果为：

```text
HTTP 403
code=chatgpt_web_browser_challenge_required
ChatGPT Web site initialization failed: stage=bootstrap upstream_status=403.
```

请求未到达模型目录或生成阶段，不能据此判定登录过期、模型不存在或账号没有模型权限。
收到验证要求后没有继续外网重试、切换账号、替换模型、运行挑战求解或改变网络出口。

另一次精确指定 `vmjcv111 / gpt-5.6-sol` 的手动提示词测试在本地返回 `unsupported`：

```text
Prompt tests require an OpenAI-compatible chat endpoint or official Codex account; this adapter is not supported.
```

该路径没有发出生成调用。源码的 adapter 白名单遗漏了 `chatgpt_web_reverse_compatible`；
不能把 HTTP 200 的管理响应或该 `unsupported` 结果说成模型调用成功。

安全回执：`linshi/gateway-chatgpt-retry-20261031-063148/` 下的 `started.json`、
`result.json`、`runtime-snapshot.json`。只保留阶段、状态、账号 ID、配置状态及模型标识，
不保存管理令牌、Grant、Cookie、Access Token 或上游正文。

## 模型名称与参考项目

当前 Web 池已保存 `gpt-5-6`、`gpt-6`、`auto` 等模型标识，未保存 `gpt-5.6-sol`，
也没有 `model_map`。这是已保存目录的事实，不是本轮重新拉取成功或模型不可调用的证明。
没有擅自把用户指定模型替换为其他标识；Web 标识与官方 OAuth 模型名不能未经验证就当成别名。

本轮联网核对两个参考仓库的最新提交，仍为此前审查的版本：

- `yukkcat/chatgpt2api`: `d58db042411f512449dd90e3b7a73de79ab38c46`
- `basketikun/chatgpt2api`: `92dd80e91543157d36a4ddad60e765c5293760e9`

没有运行第三方代码或复制其反检测、挑战绕过实现。

历史 `10/28` 隔离环境中确有 `gpt-5.4` 的普通与 SSE 成功回执，
但不能替代当前实例、当前账号和用户指定模型的验收。本轮没有重复使用已消费的历史 guard。

## 修复与验收状态

已修复两个可独立复现的源码问题：

- 为 ChatGPT Web 接入显式提示词测试，复用 production HTTP call-flow，固定选中的账号与模型；
  不启动浏览器、不调用 recovery、不隐藏重试、不切换账号或路由。普通 connectivity probe
  不会隐式发出生成请求。测试总预算为 60 秒，输出上限为 32 tokens。
- 修复 buffered SSE accumulator 的假成功：已有文本后出现 failed、moderation blocked
  或不完整终止不能算成功；partial completion 保留为 `finish_reason=length`，提示词测试
  不把它标为 passed。显式 `finished_successfully` 仍可正常结束，无需强制依赖 `[DONE]`。

新增 loopback 合同先证明入口返回 `unsupported`；接入后再次证明 failed 响应曾被标为
passed。修复后四组定向 integration suites 共 9 项通过，覆盖正常完成、bootstrap 403、
requirements_prepare 401、截断、空响应、失败、部分完成、moderation、停用账号、未知模型、
Secret Grant 边界、账号和模型不替换，以及响应与 audit 的合成凭据脱敏。
既有 Gemini fixture 的过时预期按当前“未配置模型”行为修正，没有改变 Gemini production 行为。

`cargo test --locked -j 4 --lib chatgpt -- --test-threads=2` 最终通过 224 项，
另有 3,799 项被过滤；这是定向测试，不是全量回归。四组 integration suites 与该 lib
测试串行执行，最终命令退出码为 0。最终摘要、源码 hash 与检查结果保存在
同一证据目录的 `final-validation.json`，不以中途失败日志 `probe-green.log` 的名称判断通过。

其余定向检查已通过：effective-line checker tests 31 项、ratchet、Rust 2021 scoped
`rustfmt --check --config skip_children=true` 和 `git diff --check`。
一次直接调用 rustfmt 时遗漏 edition 参数导致工具按 Rust 2015 解析；补齐仓库声明的
`--edition 2021` 后检查通过，未因此改动源码。

有效代码行：新增 probe 为 173 行，入口 336 → 341 行，module wiring 74 → 75 行，
buffered response 405 → 445 行；均未越过 500 行，不需要新增例外或更改 baseline。
只读审查未发现此次 buffered/probe 修复新增高／中风险；该审查不是上游验收。

范围边界：此次完成的是 buffered accumulator 与手动 probe，不包含 streaming translator
既有的 EOF 合成终止行为。loopback 只证明内部合同，不能解除真实 HTTP 403，
也不能代表 `vmjcv111 / gpt-5.6-sol` 生成成功。未编译或交付包含本次修复的新 EXE。

未改动另一个 owner 的“请求恢复等待”文件；未提交、推送、替换在线包或迁移数据库。

后续源码检查点已补齐 streaming translator 的终态与四入口失败审计合同，见
[流式终态修复](2026-10-31-chatgpt-web-stream-terminal-r20.md)。上述 streaming 未覆盖
描述保留为本检查点的历史范围，不代表后续未继续修复；真实 bootstrap 403 仍未解决。
