# ChatGPT Web 两阶段 requirements 与真实验收

## 已确认的问题与改动

参考项目 basketikun/chatgpt2api 固定 commit
`92dd80e91543157d36a4ddad60e765c5293760e9` 的调用契约是先请求
`/backend-api/sentinel/chat-requirements/prepare`，再请求同一路径下的 `finalize`。
本轮重新联网读取该固定源码，SHA256 为
`3a12ea7aad9eb86d69fe6dca68a33fc37fb92f082621a58ac25170a6013bc28f`。

旧包本地复现确认：仅发送旧单阶段 requirements 请求，无法完成只提供新两阶段接口的
模拟服务。这证明存在协议差异，但不能单独证明历史真实 HTTP 403 的根因。

- 普通 conversation 默认使用 prepare/finalize；旧 f 流程默认继续单阶段 requirements。
  缓存的旧 f 材料仍沿用原来的跳过握手逻辑。
- 可以通过 `extra_body.chatgptWebRequirementsMode` 明确选择 `prepare_finalize` 或
  `legacy`；未知值拒绝，不在拒绝后自动降级、重试或切换账号。
- prepare 发送原有 `p` 材料；finalize 发送准备阶段取得的 `prepare_token` 以及原有
  challenge handler 返回的材料；最终 conversation 只使用 finalize 返回的主 token 和
  可选 so token。没有新增或移植第三方挑战求解算法、浏览器指纹或换出口逻辑。
- 提取现有 challenge handler，保留 Arkose 拒绝和原有 solver 容量限制；缺少所需材料
  或任一阶段失败立即停止，不进入后续生成。
- 同一请求上下文贯穿各阶段，沿用账号局部 Cookie 更新、禁止重定向和有界响应读取。
  两阶段均剥离旧 f 专用 header，不把 prepare token 误当成生成 token。
- 增加固定 `requirements_prepare` / `requirements_finalize` 错误阶段；HTTP 错误保留
  原始状态与分类，但不回显上游 JSON/HTML。必需 token 检查空值、类型、控制字符和
  32 KiB 上限，可选空 so token 继续兼容。

本补丁不改调用者模型名，真实验收目标仍是 `chatgpt-web / vmjcv111` 与显式
`gpt-5.6-sol`。不修改在线账号材料、配置、数据库或网络，不重启现有 4200 实例。

## 验证记录

证据目录：`linshi/gateway-chatgpt-requirements-20261029-0348/`。

- `runtime-red.log`：旧 EXE 使用纯模拟账号、独立 4300 后端和 loopback 服务，
  确认旧请求路径无法满足两阶段协议；临时后端与模拟服务已停止，真实上游请求为零。
- 定向 Rust 回归：`cargo test --locked --offline -j 2 --lib chatgpt -- --test-threads=1`
  最终 221 passed / 0 failed / 3687 filtered out，回执为 `focused-green-r2.json` / 日志。
  编译 15 分 53 秒、测试 138.99 秒，仅为本次测量。第一轮编译发现阶段枚举的 move
  及测试 stream error 转换问题，均已修正，未作为通过。
- 新回归覆盖流式/非流式的默认两阶段顺序、finalize 请求体及生成 token 来源、
  两阶段 302/401/403/503 拒绝即停止、缺失/畸形 token、JSON 错误、显式旧模式和
  未知模式拒绝，以及两阶段之间并发账号 Cookie 隔离。
- scoped rustfmt、有效行 checker tests（31 项）、ratchet、UTF-8 无 BOM 与
  `git diff --check` 已通过；最终源码指纹由 `final-source.json` 记录。

有效行前后：requirements 164→83，common 49→53，surface 60→64，transport
183→185，原 execution tests 435→437，session tests 155→166；新增 challenge handler
81、exchange 123、flow tests 222。均低于 500，不涉及历史大文件例外。

## 打包与实际验收边界

定向回归已通过；接着使用 desktop devtest 流程生成新不可变目录，并检查清单、EXE
哈希、buildKind 和源码指纹；不覆盖旧包，不自动启动桌面壳或接管在线端口。

真实验收必须显式 opt-in，只读核对目标账号名称和 token 邮箱 claim，使用独立 4300
实例且仅配置该一个账号。目录请求与一次短生成分开记结果，生成必须取得有效回答、
终止标记和匹配的审计归因；本地 `/v1/models` 路由声明不算上游资格验证。
不因单次失败盲目重复调用，不切换其他账号或模型凑成功。

本文件不预先声明真实验收通过。最终打包和外网回执保存在上述证据目录；含私有
隔离数据的子目录不得整体打包、提交或外发。
