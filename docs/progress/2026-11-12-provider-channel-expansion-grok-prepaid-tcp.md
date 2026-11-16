# R03：普通余额 Key 的真实 TCP / WebSocket 联合计费验收

日期：2026-11-12（Asia/Shanghai）。本批仅测试改动，无生产协议、路由或计费修改。

## 同一轮真实连接验证

正常管理接口创建普通Key并设置message_prepaid/token_prepaid → 生产build_app_state/隔离SQLite → 正常router随机回环TCP监听 → HTTP客户端携带创建的Key → 合成Grok身份GET与WebSocket → 正常完成/异常/客户端断开 → 余额和唯一审计。

新增3项，共20组合：

- **成功8组合**：Chat/Responses × buffered/SSE × message/token。客户端实际收到答案及正确终态；buffered usage为null，审计prompt/completion/total均None。message余额2→1；token余额保留既有单文本估算预扣，不将其伪造为实际usage。
- **上游终态前关闭8组合**：相同入口/响应/模式。buffered不返回200，流没有伪造完成标记；唯一failed审计，message余额恢复2、token恢复10000。
- **真实客户端断开4组合**：Chat/Responses × message/token，流式。上游WS无终态并保持打开；HTTP客户端读到增量后，先确认预扣已经发生，再drop bytes_stream；peer观察WS关闭，唯一cancelled审计，message恢复2、token恢复10000。

每个组合均确认审计属于创建的Key、exact账号及grok-chat-fast，usage保持None；严格只有身份GET+WS两次调用和三个客户端WS事件，指定Cookie/UA、无Authorization，不重放或切换账号。先关闭/join TCP宿主，再等审计引用释放及检查余额，最后关闭SQLite/freebuff/peer。读取上限16KiB，HTTP超时10秒，增量等待3秒，总测试45秒预算。

下游是实际HTTP连接，不是oneshot模拟；上游仍为合成Grok，不代表真实平台接受性或远端退款。

## 改动与检查

- 复用既有TCP宿主，新增接受AppState/token的测试入口，旧静态Key包装保持；有效行211→224。
- 父prepaid91→93；fixture178不变，仅开放合成token给同级测试；新增public_tcp146行。无生产可见性扩大或运行时例外。
- locked/offline、runtime-defaults、4 jobs lib构建通过；完整grok_web_pipeline_tests为19 passed、0 failed、1 ignored，含新增3项及原16项。专用浏览器宿主未重复运行。
- checker31、ratchet、rustfmt、限定whitespace、UTF-8无BOM通过；baseline/exception未改，构建前后源码输入无漂移。
- Cargo compiler-artifact定位真实EXE，源/副本SHA256一致：`9ab6c93c0734e3bb89616b7ea206c417323d93ed354ee2e8dc69eb1a68cc61d9`。

证据根：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-grok-prepaid-tcp-20261112`。含inputs.json、input-drift.json、build.jsonl、build-exit.json、tests.exe、pipeline.log、test-exit.json、checker.log、ratchet.log及resource-final.json。TEMP/TMP和合成数据均在此，测试进程退出，监听任务和peer正常结束。

## 范围结论

Grok与前批Qwen现均补齐普通余额Key在真实下游TCP下的成功/失败/断连联合证据；Grok未知usage保留预扣和Qwen已报告usage按实结算保持各自语义。不能将两条渠道的限定证据外推为全部provider或整卡完成。

Server/PG、崩溃预扣恢复、现金账本/倍率、真实供应商和成品包仍未由本批证明。新增模板仍17，无新增整卡offline_ready，P50/P60/P70未完成。未使用真实凭据/付费上游，未修改在线实例、出应用包、提交或推送。
