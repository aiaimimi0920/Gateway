# R03/R07：token 非流式结算与余额不足准入

日期：2026-11-12（Asia/Shanghai）。本批仅扩展 Grok/Qwen 公共 Key 测试和 fixture，未修改生产计费、协议或路由。

## 新增验证

沿既有生产 build_app_state、隔离 SQLite、正常管理创建 Key/调整余额、公共 router 到合成上游路径，新增 4 项测试、22 个组合：

1. **非流式成功共 6 组合**：Grok 的 Chat/Responses 两入口；Qwen 两入口 × 已报告/未报告 usage。返回正常 JSON 和合成答案，唯一 completed audit 归属 Key/exact/native model。有 usage 的 Qwen 保持 2/3/5，余额 10000→9995；未知 usage 的 Grok/Qwen 保持 null/None，余额扣除既有估算预扣。测试按单文本“合成问题”的 12 UTF-8 bytes 和实际配置比例独立计算既有预扣量，不将估算写成审计 usage。
2. **不足准入共 16 组合**：两渠道 × Chat/Responses × buffered/SSE × 剩余 0/1 token。公共入口返回 HTTP 429、`error.code=token_balance_exhausted`，合成 peer 调用严格为 0；余额仍为原值，购买总量保持 10000。异步状态引用释放后唯一 failed audit 归属该 Key，total_tokens=None。不因 stream=true 绕过准入，不先 create/身份查询再拒绝。

新增 fixture 的 token_remaining 仍走正常管理余额接口，token_mode 复用它。没有直接调用 reserve/settle/refund，也未给生产路径添加测试特例。

## 检查与证据

- locked/offline、runtime-defaults、4 jobs 的 lib 测试构建通过。
- `web_pipeline_tests::prepaid` 共 16 passed、0 failed、0 ignored，包含新增4项与原12项。已有成功流、失败、取消和 message_prepaid 行为均保持。
- checker31、ratchet、rustfmt、限定 whitespace 与 UTF-8 无 BOM 通过；baseline/exception 未改。
- Grok fixture 有效行175→178、tokens101→182；Qwen fixture191→194、tokens104→188。所有改动文件在250行以内。
- 构建前 Cargo.toml/Cargo.lock/全src输入与验证后比较无漂移。
- Cargo compiler-artifact 返回真实测试EXE，复制后源/副本哈希一致。SHA256：`c17d9434dd4a9c546f26e7a69a3597899238e76d7fd8477181f790b75ce20bbe`。

证据根：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-web-token-admission-20261112`。包含 inputs.json、input-drift.json、build.jsonl、tests.exe、prepaid.log、checker.log、ratchet.log、resource-final.json。TEMP/TMP和合成配置/数据库留在该目录，fixture/peer正常关闭，测试进程退出。

## 范围结论

上一批明确保留的 buffered 成功 token 结算证据空白已关闭；同时证明0余额及正但不足预扣的余额会在上游调用前拒绝。这里不是恰好等于预扣阈值、并发最后一份余额竞争或实际用量超过余额的证明。

下游仍为 router oneshot/body drop、上游为合成peer，未新增真实TCP计费断连或平台验收。现金账本/倍率、供应商扣额/退款、PG/Server、崩溃预扣恢复、完整服务和成品包仍未由本批证明。新增模板仍17，无新增整卡offline_ready，P50/P60/P70未完成。未使用真实凭据/付费上游，未修改在线实例、提交或推送。
