# SenseNova Token Plan：原生 Messages

## 合同与实现

2026-11-22 匿名重新访问 [SenseNova 官方文档](https://platform.sensenova.cn/docs)，HTTP 200。
从页面实际引用的公开静态资源 `210-35a7025cd2eaebe6.js` 读取模块 `82285` 的文档文本，未执行页面脚本或导入账号。
资源 SHA-256 `b4cec554990cb8deef4eeb05949bbdec4b1a85bdf45e7e09e1a982e442eae813`。
Messages 使用固定 `POST https://token.sensenova.cn/v1/messages` 和原 Token Plan Bearer Key；
图像输入章节与请求参数区包含 URL／Base64 示例和模型范围；JSON 思考块示例不含签名。

- 正常 Messages 不再经过 Chat；目标固定，不允许配置覆盖路由、客户端身份或注入请求头。
  Local 与 Hosted／缓存的候选能力投影增加 `anthropic_messages`，显式协议策略仍适用。
- 复用原生请求同步与 Anthropic 有界校验，保留 `thinking`、`output_config`、函数和图像；
  canonical 修改优先，不能通过原始副本复活被改掉的字段。图像只接入文档明确列出的
  `sensenova-6.8-flash-lite`、`sensenova-6.8-flash`、`deepseek-flash`；Gateway 自身不请求图像 URL。
- JSON 与 SSE 保留原生字段，公开 model 使用 caller 别名；用量只从权威字段读取，推理 token 不重复累加。
  SSE 允许 SenseNova 无签名思考，Command Code 仍保留签名校验。终态只在干净 EOF 后发布，
  无效流、错误尾部及中断不伪装成功。上游托管工具、MCP 与没有合同的文档、音视频输入不开放。
- Chat、公共 Responses 转 Chat 及现有定向探测未改为原生 Responses。Responses 独立模型、
  无状态准入、原生 JSON／SSE 及图像／reasoning 是后续切片，不把 Messages 进展记作完成。

## 回执

- 改动前新增一条完整公共链路用例：合成上游实际收到 `/v1/chat/completions`，预期 `/v1/messages`，
  退出码 101（1 failed）。并非真实账号、网络不可达或额度失败。
- 改动后 checker 自测 31/31、有效行 ratchet、定向 `rustfmt --check` 与 `git diff --check` 通过。
- 改动后 SenseNova 产品定向回归 28/28 通过，覆盖公共文本及两轮函数 JSON/SSE、原生图像/思考、取消退款、无效响应和不重放。
- 复用同一测试二进制，Command Code（不含 Server 专测）104/104、共享 Anthropic 62/62、共享 Responses 60/60、协议路由 37/37 通过。没有调用供应商推理或消耗额度。

本轮原始日志：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-commandcode-audit-20261122`。
尚未执行新桌面包／原生交互、自动调度与真实 PostgreSQL／Redis 的 SenseNova 业务验证。
控制台 OAuth、积分池与客户端资格另列，不从普通 Token Plan Key 推断。
