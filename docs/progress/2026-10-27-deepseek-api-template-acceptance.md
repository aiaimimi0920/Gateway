# DeepSeek API 模板验收（2026-10-27）

## 结论与边界

按用户批准的余额受限标准完成 `deepseek-api` 模板复核；实际验收后登记完成标记。
真实生成尚未成功：三次单模型生成尝试均收到 HTTP 402。流式请求在 SSE 建立前被拒绝，
不能称为真实流式文本输出通过。未充值、未遍历模型、未修改正在使用的 4200 实例。

## 修正

- 模板 ID、默认 Provider ID、显示名称、preset 统一为 `deepseek-api`，不再注册旧 `deepseek-openai` preset。
- 同步品牌、协议 profile、健康适配器与实现线映射；内部 `deepseek` 协议及既有凭据目录格式不变。
- 默认地址 `https://api.deepseek.com`，明确使用 `/chat/completions` 与 Bearer 鉴权。
- 删除模板预填的旧 `deepseek-chat`：创建空账号、空模型、空路由池；主动测试仅 `deepseek-flash`，自动测试关闭。
- 沿正常手动导入流程声明单个模型；没有新增官方目录专用读取器，不调用通用批量 discovery。
- 新增前端空池/账号导入测试与 Rust 有界 loopback JSON/SSE/402 合同；假密钥不进入真实网络。

## 官方依据（本轮联网核对）

- https://api-docs.deepseek.com/ ：OpenAI-compatible 地址与 Bearer 示例，当前模型为 `deepseek-flash`、`deepseek-v4-pro`。
- https://api-docs.deepseek.com/api/create-chat-completion/ ：`POST /chat/completions`、`stream`、SSE `[DONE]`，支持 `thinking.type=disabled`。
- https://api-docs.deepseek.com/quick_start/error_codes/ ：401 为鉴权失败，402 为 Insufficient Balance。
- 用户 Key 只读 `GET https://api.deepseek.com/models` 返回 200，目录包含上述两个模型；不以目录成功代替推理成功。

## 隔离 UI 与真实请求

地址 `http://127.0.0.1:4300/ui/`；数据位于本轮证据目录 `private/runtime`，不共享用户 4200 数据。
通过正常 UI 依次创建空服务商、添加账号 `deepseek-acceptance-20261027`，手动模型仅 `deepseek-flash`。
保存 revision `r2-fdfc0bc2c4b5`，1 账号、1 路由，导入后生成审计数为 0。

| 项目 | 结果 | 精确请求证据 |
| --- | --- | --- |
| UI 固定账号连通测试 | HTTP 402，failed、callable=false、score=null | `probe-c227772c-f80b-4937-a9e6-e964b463ca65` |
| `/v1/chat/completions` 非流式 | HTTP 402，Insufficient Balance，532 ms | `c4dbbbd5-8bc9-4232-8dfb-b70e0d8a5646` |
| `/v1/chat/completions` 流式 | HTTP 402 JSON，SSE 尚未建立，485 ms | `63b20faf-b8b1-4aa1-b1fb-193244f9edd8` |

普通入口两次均为同一精确账号、单 candidate、单 route attempt；显式关闭 thinking，max_tokens=16。
上游 `code=invalid_request_error`，错误正文为 `Insufficient Balance`。Gateway errorKind 仍为 `Unknown`，
但两条审计 failureClass 均正确归为 `quota_exhausted`；不把 UI 调度恢复后的“可用”当作余额证明。
UI 测试后通过正常调度开关关闭/开启一次，以进入普通入口验收；未写 SQL、未删除审计。
共 3 条真实生成尝试审计；没有调用 `deepseek-v4-pro`，没有重复或自动重试。

## 验证与证据

- 前端定向测试初次按预期失败（新模板不存在），修改后通过。
- Rust `cargo test --locked --lib deepseek -j 4`：4/4，通过 URL、body 参数、JSON/SSE、402 与相邻 DeepSeek 测试；preset registry 2/2。
- TypeScript typecheck、effective-lines checker 31/31、ratchet、定向 rustfmt、`git diff --check` 通过。
- web devtest 后端构建通过，Cargo 耗时 6m47s，构建前后来源指纹一致；不是正式发布或新桌面整包。
- 冻结 EXE SHA256：`5864ef318d3e4a7a910dc2ac66edc5993f2f6d38bae06e09206a8566a3f35855`。
- SQLite integrity 为 ok。Key 仅通过正常 UI 保存在隔离运行数据中，不写源码、报告或测试夹具。
- 完成标记在验收后追加，冻结 EXE 不含该新增 UI 标记；用户当前桌面 EXE 尚未更新。

证据根目录：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-deepseek-template-acceptance-20261027/`。
主要文件：`models-readonly.json`、`ui-imported-account.json`、`ui-import-no-generation.json`、
`ui-test-results.json`、`buffered.json`、`stream.json`、`audits-sanitized.json`、`integrity.json`、`build-provenance.json`。
未验：充值后的成功生成与真实 SSE 完整内容、其他模型、Responses/Anthropic 兼容接口、工具调用及长期稳定性。

## 最终收尾

完成标记追加后，同步更新精确标记白名单及 UI 文本断言，最终相关前端 4 文件 23/23 通过，
typecheck、ratchet 与 diff 检查再次通过。曾出现的标记白名单/文本断言失败已修正，不隐藏失败过程。
有效行数：catalog 423、workflow 118（均不增长），preset 202→205，client 模块接线 436→439，
新增前端测试 37、Rust 合同测试 128；本轮没有新增或扩张超限文件。
隔离后端通过正常 drain 接口退出，4300 已无监听，自建浏览器页已关闭；保留隔离数据与证据，
未操作现有 4200 进程。界面结果截图为 `ui-result.png`，停止回执为 `cleanup.json`。
