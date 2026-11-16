# R03：Grok Web 公共入口真实 TCP / WebSocket 验收

日期：2026-11-12（Asia/Shanghai）。工作区 `C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway`；HEAD `7969f12c85efceacf2a5e384e143b5c39a550981`。保留继承修改，只增加测试，不改生产实现。此批承接原 `oneshot` 单轮验证，补真实入站 socket，不是 Grok 整卡或真实平台验收。

## 改了哪些内容

新增 `src/upstream/client/grok_web_pipeline_tests/tcp.rs`，父模块只加两行注册。复用现有精确账号路由、Local SQLite 和非转发合成 peer，生产 router 经随机回环 listener 接受真实 HTTP 请求，上游执行真实身份 GET 和 WebSocket Upgrade、收发事件。

测试 middleware 只在实际 axum 请求任务中绑定已有 test-only loopback scope；没有生产网络例外。合成目标只允许既有 identity/WS 两条路径，缺 scope 直接失败，不回退真实 Grok。

新增三个测试，共 18 种组合：

- 4 种成功：Chat/Responses × buffered/SSE，必须收到完整答案及对应成功终态；buffered usage 与审计 usage 均保持未知，不补零。
- 12 种失败：上述两协议/两输出形式 × 提前关闭、坏 JSON、未知终态。必须记 `failed`，不能输出成功终态；逐块保留传输错误前收到的字节，防止整体 body 读取失败掩盖先前错误发出的成功标记。
- 2 种取消：两协议分别在 WS 无终态且保持打开时先接收答案，再丢弃真实下游 HTTP 响应流。两秒内收到答案证明非 EOF 聚合；等待上游 WS task 关闭，唯一审计必须为 `cancelled`。

每种场景还检查：只有一次 identity GET 和一次 WS；实际 Cookie/UA 归属 exact 账号，caller Authorization 不进入 WS；只有 session.create、用户 item、response.create 三条客户端事件，alias 对应 native `fast` 模式，输入文本未变；唯一审计的模型、账号、状态准确且无秘密泄漏。`fast` 是调用模式，不是固定模型版本或账号权益证据。

正常收尾先 graceful shutdown/join 下游，再释放 AppState、关闭 SQLite，最后等待 WS 关闭并停止合成 server；失败展开另有 task abort。只证明本地资源收尾，不承诺远端停止生成、停止计费或退款。

## 实际检查

证据根：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-grok-public-tcp-20261112`。

| 检查 | 结果 |
| --- | --- |
| 锁定离线测试库编译 | `cargo test --locked --offline --lib --no-default-features --features runtime-defaults -j 4 --no-run --message-format=json`，退出 0；未启用默认 management-ui 发布 hook |
| 新增 TCP 定向 | 3 passed / 0 failed，内部 18 种组合 |
| Grok 相邻检查 | 同一冻结 harness 按 `grok` 过滤：147 passed / 0 failed / 0 ignored，已包含新增三项，不重复相加 |
| checker 自测 | 31 passed |
| 官方 ratchet / rustfmt / 范围 whitespace | 均通过；ratchet 使用无报告模式，不覆盖继承报告 |
| 来源与收尾 | 选定 Rust/Cargo/build 输入无漂移，执行后无 `grok-tests` 进程残留 |

Cargo 实际输出 EXE 已复制冻结，副本 `grok-tests.exe` SHA256：`25ed426cbbe876b3af38c03ff2f47b67d5c39dc4849ffceba6ebdd04b934c65b`。这是测试 harness，不是应用 EXE 或手测包。源码未改变生产行为，未制造旧生产红测；本轮定向及相邻测试没有失败阶段。

主要证据为 `build.jsonl`、`build-inputs.json`、`build-drift-final.json`、`tcp-tests.log`、`grok-adjacent.log`、`checker.log`、`ratchet.log`、`format.log` 与最终回执。父模块有效行 247→249，新模块 211；无 baseline/exception 修改。只读交叉审查检查了真实 HTTP/WS 路径、精确终态、隔离和清理，没有以审查替代实际执行。

## 尚未证明的边界

本批只覆盖单轮 `grok-chat-fast` 经两个公共入口，不是四模式或全部模型新一轮验收。成功用例检查成功终态存在，不据此单独宣称其只发一次。fixture 未建立有余额普通 Key，未核验 permit/健康计数，不外推扣额、退款或全部资源结算。

正常管理网页材料保存/精确 probe 联合、独立应用进程/Server/真实 PostgreSQL、devtest 整包、当前真实 TLS/WAF、SSO/challenge、额度/权益和供应商验收仍未完成。多轮、工具、搜索引用及其他模态继续受原合同限制，不猜新 wire。

新增模板仍 17，无新增整卡 `offline_ready`，P50/P60/P70 与总目标继续。未读取真实凭据、调用真实供应商、操作在线实例/容器、出包、提交或推送。下一步可补 Grok 正常管理网页的合成联合链，不重复本批已通过的公共 TCP。
