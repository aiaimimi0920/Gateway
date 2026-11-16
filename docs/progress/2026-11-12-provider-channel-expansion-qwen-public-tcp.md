# R07：Qwen Web 公共入口真实 TCP 验收

日期：2026-11-12（Asia/Shanghai）。工作区 `C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway`；HEAD `7969f12c85efceacf2a5e384e143b5c39a550981`，保留现存并行修改。此批补齐既有单轮文本的入站 socket 证据，不增加生产功能，不代表整卡完成。

## 改了哪些内容

- 新增 `src/upstream/client/qwen_web_pipeline_tests/tcp.rs`，父测试模块只增加注册。复用现有精确账号路由、非转发合成上游、Local SQLite 与生产 `build_router`，由随机回环 listener 承接真实 HTTP 客户端请求，不用 `oneshot` 替代 TCP。
- 测试 middleware 仅把既有 task-local 合成目标绑定到 axum 的请求任务；没有生产配置开关或真实目标回退。合成 peer 只支持既有两个 v2 path，客户端禁止环境代理，凭据全部为固定合成材料。
- 新增 3 个测试，共 14 种组合：Chat/Responses × buffered/SSE × 已报告/未知 usage 的 8 种成功；两协议 × buffered/SSE 的 4 种截断；两协议读到增量后关闭客户端的 2 种取消。

成功必须有答案和对应成功终态；buffered usage 与审计中的已报告/未知 total 保持一致。截断必须 `failed`，断开必须 `cancelled`，每次只有一条审计。失败流逐块保留错误发生前已收到的字节，避免读取整体 body 报错后跳过“不能出现成功终态”的断言。

取消场景的上游没有终态且保持打开，客户端在两秒内收到答案后主动丢弃响应流，再等上游 body 释放。这证明真实增量与本地连接取消，不证明平台停止生成、退款或删除会话。所有场景均检查仅一次 create 加一次 completion、精确账号 Cookie/Bearer/UA、别名解析和审计秘密隔离，没有换号或备用渠道重放。

下游使用 graceful shutdown 并限时 join，随后释放 AppState、关闭 SQLite、停止合成上游；异常展开路径保留 task abort。没有新增真实凭据、付费调用或无限等待。

## 实际验证

证据目录：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-qwen-public-tcp-20261112`。

| 检查 | 实际结果 |
| --- | --- |
| 锁定离线测试库编译 | `cargo test --locked --offline --lib --no-default-features --features runtime-defaults -j 4 --no-run --message-format=json` 退出 0；关闭默认 management-ui，未触发 UI 发布/清理 |
| 新增 TCP 定向 | 3 passed / 0 failed；14 种内部组合 |
| Qwen 相邻测试 | 同一冻结测试程序按 `qwen` 过滤：120 passed / 0 failed / 0 ignored，已包含新增 3 项，不重复相加 |
| 行数 checker 自测 | 31 passed；正式 ratchet 初轮及最终无报告模式通过 |
| 格式与范围 diff | 两个 Rust 文件 rustfmt 通过；父文件与直接备份、独立新文件的 diff whitespace 检查均无输出 |

Cargo 返回的实际测试 EXE 已复制后再执行，原件与副本 SHA256 均为 `335169c0f27e4d19e69c1eeb9bb08659a38f1fb56324381c0aa9c319efb7a4e4`。原路径见 `build.jsonl`，执行副本为证据目录内 `qwen-tests.exe`。它是测试 harness，不是应用 EXE 或可交付手测包。

编译曾等待其他 Cargo 工作，不中断其他进程。构建输入快照在等待期间采集；交叉审查后收紧断言并更新快照。构建/验证期间存在范围外漂移：`src/protocol/minimax.rs`、`src/protocol/minimax/wire_tests.rs`、`src/provider_runtime.rs`、`src/provider_runtime/anthropic_model_probe.rs`、`src/provider_runtime/console_model_probe.rs`。本批两个测试文件及独立 Qwen 文件与最终快照一致；结果绑定固定 harness，不能称最新整个工作树已统一验证通过。

本批仅加测试，没有生产行为修复，因此没有制造所谓旧生产红测。中间一次 ratchet 命令将 `--json` 指向仓库外，因 `Path must be repository-relative` 退出 2；保留原日志，改用官方无报告模式后通过，没有放松 checker。初轮 npm gate 的继承报告已按原 bytes 恢复。没有运行前端、关线矩阵、真实平台、容器、独立应用进程或打包。

## 代码规模与剩余边界

父测试模块有效行 262 → 264，新测试模块 202；无 baseline 或 exception 修改。代码审查限定合成网络、固定凭据、读取上限、deadline、取消与 handle 收尾；子代理只读交叉审查发现的终态断言过宽已收紧。

这批没有建立带余额普通 Key，因此不声称扣额、退款、permit 或健康计数完整验收；只证明现有公共认证 fixture、准确终态审计与 socket 生命周期。正常浏览器到管理 TCP 的材料保存/精确 probe 联合、独立 Server/真实 PG、应用进程/包、Qwen 登录刷新、多轮/工具/模态、真实权益/额度和平台验收仍未完成。

新增模板仍为 17，无新增整卡 `offline_ready`，P50/P60/P70 仍未完成。未读取真实凭据、请求真实供应商、修改运行实例、提交或推送。下一步可承接 Qwen 正常管理 UI 联合，而不是重复本批公共 TCP；其他渠道按已有合同继续，不猜 Perplexity patch 或接管并行 Sensenova/Gemini 工作。
