# Flow 图片／视频 HTTP-only 路线研究

日期：2026-11-01。范围：继续调查 `flow-web` 的无浏览器图片／视频生成，不以其他服务商或外部浏览器替代。

后续已追加[真实 HTTP 实验](2026-11-01-flow-web-http-live-canary.md)：修复 Cookie 导入缺陷后，
只读请求及项目创建成功，一次图片提交被上游反滥用拒绝，视频未提交。下文保留为当时仅研究阶段的记录。

## 本轮结论

当前不能交付满足约束的 `flow-web` 图片／视频生成。研究没有证明纯 HTTP 在技术上绝对不可能；
也没有执行真实 Flow 生成并收到上游拒绝，不能把本轮描述为真实生成失败验收。

公开参考的主要已用路线仍包含浏览器，或者把验证码处理交给第三方。第二个参考的 HTTP 兜底
涉及重建 reCAPTCHA 验证材料获取流程，而不是使用 Flow 官方公开的无验证生成入口；本轮没有移植、
运行或改写该防自动化绕过逻辑。没有为了移除浏览器而加入这类能力。

Google 官方图片／视频 API 是有文档的无浏览器替代路线，但需要相应 API 凭据和计费。
它不等于 Flow 网页会话，也不能默认声称消耗的是 Flow 订阅额度；没有静默替换服务商或调用现有 API key。

## 复核到的证据

| 来源 | 本轮核对内容 | 能证明什么／不能证明什么 |
| --- | --- | --- |
| `HL320217/flow2api` | 当前默认分支 commit `95d9cabea3dc539478bbae6c101691a4344adab4`；README 主要使用 Chrome broker，也保留 HTTP 兜底；`backend/app/services/flow/recaptcha.py` 的本地 token 检查仅检查存在和长度 | 确认存在实验代码，不等于 Google 生成端接受；README 的出图／视频实测声明没有把 HTTP 兜底单独证明为成功 |
| `TheSmallHanCat/flow2api` | 当前 README 仍要求浏览器或第三方验证码服务 | 该项目没有提供满足本轮约束的已证明调用路线；不能据此推导所有路线不可能 |
| `ffroliva/gflow-cli` | 当前 README 明确以真实浏览器驱动 Flow；检索到的项目说明仍将 HTTP transport 列为待解限制 | 是另一份浏览器依赖的项目经验，不是 Gateway 自己账号的失败实测 |
| `eddie-fqh/flow-py` | README 将只读操作与生成分开，生成依赖浏览器；报告其程序化 token 被 403 拒绝 | 属于该项目旧 Labs 路线的观察，不能当作当前 `flow.google.com` 上所有纯 HTTP 方案的通用定理 |
| Google 官方 Gemini API 文档 | 图片与视频有官方 API 路线 | 可以在不启动浏览器的条件下通过正式 API 使用相应能力，不证明 Flow Cookie 或 Flow 额度可用于该 API |

本轮检索未找到足以交付的官方公开 Flow Cookie 生成 API。网页请求的传输是 HTTP，
不意味着验证材料获取也不依赖浏览器；拿到 token 字符串也不等于通过服务端评估。

## 实施与验证边界

- 已完成联网研究、参考源码核验及本地现有实现核验。
- 没有新增未完成的生成 encoder、后台 proof worker 或隐藏浏览器 fallback。
- 没有运行参考项目的验证码代码，没有生成或伪造验证材料，也没有接入打码服务。
- 没有执行 Flow 真实登录、图片／视频提交、额度消耗、成品轮询或回收。
- 本轮没有生产源码改动；仅补充本记录和入口文档，执行文档编码与 Git whitespace 检查，不重复编译。
- 保留现有模板未标蓝、模型目录为空、生成 fail-closed 的状态；该本地错误不是上游拒绝证据。
- 未改运行实例、已有凭据、数据库；未出包、提交或推送。

## 后续可以推进的边界

若 Google 提供受支持的 Flow 自动化授权接口，或有不需要规避防自动化验证的正式无浏览器路线，
可以继续在既有 Rust HTTP 架构实现并进行真实验收。若改用官方 Gemini API，必须作为其自己的服务商、
凭据和计费处理，不能用它来把 `flow-web` 标蓝。

## 来源

- [HL320217 README（固定 commit）](https://github.com/HL320217/flow2api/blob/95d9cabea3dc539478bbae6c101691a4344adab4/README.md)
- [HL320217 本地 token 检查（固定 commit）](https://github.com/HL320217/flow2api/blob/95d9cabea3dc539478bbae6c101691a4344adab4/backend/app/services/flow/recaptcha.py#L69-L70)
- [TheSmallHanCat/flow2api](https://github.com/TheSmallHanCat/flow2api)
- [ffroliva/gflow-cli](https://github.com/ffroliva/gflow-cli)
- [eddie-fqh/flow-py](https://github.com/eddie-fqh/flow-py)
- [Google 官方图片生成文档](https://ai.google.dev/gemini-api/docs/image-generation)
- [Google 官方视频生成文档](https://ai.google.dev/gemini-api/docs/video)

上述链接均于本轮联网核对；项目 README 的自述与 Gateway 自己的真实验收分开记账。
