# SenseNova Token Plan：原生 Responses

## 合同与实现

依据 2026-11-22 匿名读取的 [SenseNova 官方 API 文档](https://platform.sensenova.cn/docs)，
Responses 是同一 Bearer Key 下的独立 `POST https://token.sensenova.cn/v1/responses`。
模型范围是 `sensenova-6.8-flash-lite`、`deepseek-flash` 和 `kimi-k3`。本地和
Hosted／缓存路由按解析后的上游模型呈现支持，发送前再次校验；不把不支持的模型偷偷转为 Chat。
获取和校验公开文档静态资源的证据见 [Messages 切片](2026-11-22-sensenova-native-messages.md)。

- 原生请求同步 canonical 修改后保留 `reasoning`、结构化输出、函数定义、完整工具历史
  和 `input_image` URL／Data URI；Gateway 不取图。Flash Lite 不开放 `json_schema`。
- 支持明示的无状态推理。拒绝远端引用、存储、后台任务、托管工具/MCP、Files API 引用、
  文档或视频输入。不会将状态请求改写为无状态成功。
- 保留原生 JSON／SSE、reasoning 摘要及函数身份；输出 envelope 的模型向客户端投影为
  请求别名，观察仍用真实模型。复用有界 SSE 解帧和干净 EOF 门槛；接受文档示例中省略
  `object`、`model`、`output` 的简短终态，但终态 ID 必须与 `response.created` 一致。
- 结算只看通过校验的终态，reasoning tokens 不额外叠加。错误或中断不发布成功终态，
  已收到无效生成不重放。Command Code 还是使用自己的完整终态验证。

## 证据

- 改动前新增的公共入口用例真实观察到 `/v1/chat/completions`，预期 `/v1/responses`，
  合成上游的原生 JSON 被按 Chat 拒绝（1 failed，exit 101）。初次 Cargo 调用误带了
  `--exact-filter`，未执行用例；上述红测来自复用编译后的 EXE 以 `--exact` 正确运行，
  不把 CLI 参数错误当产品缺陷。
- 改动后定向格式、有效行检查器自测 31/31、有效行 ratchet、55 条实施线清单验证和
  `git diff --check` 通过。
- SenseNova 定向 **31 passed / 0 failed**：公共三入口文本及两轮函数 JSON/SSE、原生摘要／图像、
  简短 `response.incomplete` 终态、真实模型准入、文件/远端状态拒绝、错误／截断／取消退款和不重放。
  合成 SSE 故意加了非终态伪用量，审计仍以终态的输入 2、输出 3、总计 5 记账。
- 复用同一测试 EXE 串行：Command Code **104 passed / 7 ignored**，共享 Anthropic **62 passed**、
  Responses **60 passed**、路由 **37 passed**、流观察 **53 passed**。相关通过项均为零失败；
  Command Code 隔离 Server 专测未重跑。
- 独立构建 `cargo check --locked --offline --lib --bins --no-default-features --features line-sensenova-token-plan -j 2`
  通过（exit 0）。有编译警告，不把 `check` 视为桌面包运行验收。

原始回执临时目录：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-commandcode-audit-20261122`。
没有导入账号、发送真实推理或消耗额度。自动调度／内部审计、Local 重启、隔离 Server 多实例、
新手测包及原生业务点击仍分开验证；控制台 OAuth／积分池仍是独立资格与合同事项。
