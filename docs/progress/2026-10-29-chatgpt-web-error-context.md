# ChatGPT Web 前置拒绝：错误归属与目录分类修复

## 当前结论

目标账号此前对 `gpt-5.6-sol` 的一次真实短调用，失败于
`POST /backend-api/sentinel/chat-requirements`：HTTP 403、HTML 响应。
请求尚未进入模型生成阶段，因此不能据此判定该模型不存在、账号无模型权限或登录过期。
这份修复解决错误来源丢失和错误分类问题，不代表已经解除上游拒绝或完成真实生成。

本轮只读检查确认：在线 revision 为 `r36-21e0488cbd29`，目标账号有 Bearer
session-auth、Cookie、User-Agent；未发现非 Bearer 的认证配置覆盖。
没有改在线凭据、配置、网络出口，也没有重启在线 4200 Gateway。

## 改了哪些内容

1. 在 bootstrap、requirements、prepare、conversation 四个 HTTP 响应分类调用点
   添加固定阶段标签及原始 HTTP 状态。非流式和流式发送共享相同的分类规则。
2. 禁止请求时浏览器回退时，先显示原始失败原因，再显示回退策略；保留原错误 code
   和 `upstream_status`。对外仍使用现有的 503 与
   `chatgpt_web_request_time_browser_forbidden`，不把上游拒绝混成 Gateway 调用者鉴权失败。
3. 网页模型目录不再将任意 403 等同于浏览器验证。
   `cf-mitigated: challenge` 才报告明确的 challenge；普通 403 使用
   `chatgpt_web_catalog_forbidden`，不武断宣称登录失效或要求重新登录。
   challenge 文案报告实际状态，不再将 503 等错误写成 HTTP 403。

预期安全错误来源示例：

```text
ChatGPT Web reverse upstream failure: stage=requirements upstream_status=403.
...
original_code=chatgpt_web_browser_challenge_required upstream_status=403
```

## 验证

- 修复前：ChatGPT 相关 Rust 测试 199 passed / 4 failed，四项均命中本次新增的回归断言。
- 修复后：`cargo test --locked -j 2 --lib chatgpt -- --test-threads=2`：203 passed。
- 新增 loopback 测试覆盖四阶段 × 流式/非流式，共八种拒绝路径；验证后续请求不再发送，
  且模拟响应正文中的 private marker 不进入诊断。
- wrapper 回归覆盖上游 403 和 401，保留外层 503 与原始状态的区别。
- 目录回归覆盖普通 403、明确 challenge、challenge 携带 HTTP 503，以及原有异常响应边界。
- effective-line checker：31 passed；ratchet passed；相关 Rust 文件 scoped rustfmt passed；
  UTF-8 无 BOM 与 `git diff --check` passed。
- 旧 EXE 的独立 loopback 模拟已重现：只发送一次 requirements 请求，错误缺少阶段标签。
  首次夹具因未设置模拟 session 的到期时间停在 keepalive，补齐夹具后才得到上述复现；
  该问题属于测试准备，不计为产品回归。

日志位于 `linshi/gateway-chatgpt-web-errors-20261029/`。测试编译耗时约 10 分 54 秒，
测试执行 126.47 秒；这是本次测量，不承诺其他构建的速度。

## 增量规范与安全边界

有效行数前后：common 30→49，execution 193→210，requirements 159→164，
transport 172→179，bootstrap 262→255，policy 107→108，policy tests 181→213，
execution tests 428→429，新增 failure-context tests 128，catalog 163→171，catalog tests 113→133。
所有受影响源码均少于 500 有效行，不涉及例外或历史大文件扩张。

阶段仅来自固定枚举，状态来自 HTTP 数值；不增加响应正文、完整 headers、Cookie 或 token
的采集和输出。不改变请求认证、挑战求解器、浏览器指纹、重试/回退权限或网络出口。
成功路径不增加格式化分配、后台任务或队列；仅失败路径添加有界的来源文案。
沿用原配置、凭据、数据库格式，不需要迁移；保留本轮开始前的其他 dirty edits。

## 手测包与未完成项

准备 desktop devtest 包 `gateway-devtest-20261029-013800-web-error-context`；
构建后的哈希、完整性及独立 loopback 运行回执保存在同一 linshi 证据目录，
以实际构建回执为准，不由本文预先声明通过。

未新增真实外网生成调用，未自动处理上游验证，未验证 `gpt-5.6-sol` 对目标账号的目录资格
或生成能力。该账号的真实可用调用仍未完成，不应将本次诊断与分类修复称为业务调用成功。
