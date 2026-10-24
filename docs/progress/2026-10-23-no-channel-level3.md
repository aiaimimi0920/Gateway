# 无可用渠道直接进入第三档临时禁用

## 源码修改

- 明确的 distributor “无可用渠道”拒绝首次发生就进入第三档：32 分钟。
- 同步更新内部档位和截止时间；已有第四档不降级，到期探测再失败进入第四档 60 分钟。
- 沿用现有 provider/group/model 键、并发 generation 隔离和单探测租约。
- 成功恢复仍要求原有的两次间隔探测；普通错误仍按原有累计失败规则处理。
- HTTP 与输出前 SSE 的结构化拒绝不在同一候选重试，允许既有预算边界内切换候选。
- 识别英文 `No available channel ... (distributor)`、中文“无可用渠道（distributor）”，
  以及本次实测的 `get_channel_failed` / “的可用渠道不存在（retry）”。
- 泛化的 503、仅 `model_not_found`、鉴权拒绝、代理故障不因此直接跳第三档。

新增识别与状态转换回归用例，但依用户要求没有执行 Rust 测试或构建；没有重启运行实例。
源码已格式化、复读并进行范围内差异检查。现有其他 AI 的修改保留。

## 真实上游调用（与源码测试分开）

2026-10-23 22:05–22:10（北京时间），从本地已配置的第三方兼容 API 及相关模型聚合服务中，
检查 20 个服务商配置、40 个凭证条目；实际调用 36 个未禁用条目，保留 4 个禁用状态不调用。
不涉及搜索、音视频生成、浏览器会话型或仅提供自身模型的其他无关服务。
仅发送 `Reply with OK only.`，最大输出 64 tokens，不发送项目内容。

- OpenRouter：一个现有凭证普通请求及 SSE 均 HTTP 200，正文 `OK`；SSE 正常 `stop` 和 `[DONE]`。
  请求 `deepseek-v4.1-flash`，响应模型为 `deepseek/deepseek-v4.1-flash`。另一个凭证 401。
- 君的公益、guji、小鸡毛、魔方（6 个凭证）、简直（2 个凭证）：无可用渠道。
- 薄荷：503 `model_not_found`，未返回明确的渠道不可用结构。
- 月城：429 月额度耗尽；muu：小写模型无渠道，目录中的大小写变体额度不足。
- 42：3 个凭证 401；另两个分别代理不可达、500 `get_channel_failed`。
- seekai：key 被禁用；烁、百倍、ag：403/401 访问拒绝。
- NVIDIA：404；讯飞可用条目：400 `no category route found`；Poe：500。
- 旧 Muyuan 条目：401；lanln：配置已禁用，未调用。

脱敏明细：`C:/Users/Public/nas_home/AI/GameEditor/linshi/deepseek-providers-1792764345045/summary.json`。
OpenRouter SSE：同目录 `openrouter-stream.json`。
扫描脚本：`C:/Users/Public/nas_home/AI/GameEditor/linshi/deepseek-provider-probe-20261023.cjs`，密钥只从
本地配置读入内存，不写入结果，禁止自动重定向，单次 45 秒/2 MiB，上游并发最多 3。

没有把 OpenRouter 自动写入运行中的 DeepSeek 路由，没有变更配置、凭证或数据库。
上述结果只是该时段的小请求可用性，不代表长上下文、工具调用或持续稳定性验收。
