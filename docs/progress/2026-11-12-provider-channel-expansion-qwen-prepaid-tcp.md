# R07：普通余额 Key 的真实 TCP 联合计费验收

日期：2026-11-12（Asia/Shanghai）。本批仅扩展测试TCP宿主、测试凭据可见性与新增测试，无生产路由、协议或计费修改。

## 同一次真实连接流程

正常管理创建普通Key并设置message_prepaid或token_prepaid → 生产build_app_state/隔离SQLite → 随机回环TCP监听上的正常router → rquest客户端携带实际创建的Key发请求 → 合成Qwen create/completion → 响应或客户端断开 → 公共终态结算与唯一审计。

新增3项测试，共20组合：

- **成功8组合**：Chat/Responses × buffered/SSE × message/token。客户端实际收到答案及正确终态，buffered JSON usage=5；审计2/3/5。message余额2→1，token余额10000→9995。
- **截断8组合**：同样的入口/响应/余额模式。上游没有完成事件，buffered不返回200，流不伪造完成标记；唯一failed审计、usage未知，message恢复2、token恢复10000。
- **断连4组合**：Chat/Responses × message/token，均流式。上游保持打开，真实客户端读到增量且没有终态；断开前确认已有预扣（message=1或token下降），drop客户端bytes_stream后观察上游连接关闭；唯一cancelled审计、usage未知，message恢复2、token恢复10000。

所有组合均验证审计归属创建的Key/exact/native model并脱敏；指定Bearer/Cookie/UA实际出站，上游严格create+completion两次，无重放/换号。TCP宿主graceful shutdown并join后，再等待审计引用释放和检查余额，最后关闭SQLite/freebuff/peer。读取16KiB上限，HTTP超时10秒，断连增量等待3秒，测试沿45秒总预算。

这里的下游不再是oneshot/body drop模拟，而是真实HTTP客户端与监听连接；上游仍是合成peer，并非Qwen平台。不代表真实供应商退款。

## 实现与检查

- 原tcp.rs保留旧静态Key包装，新增接受AppState和token的测试入口，复用同一监听、task-local隔离及关闭逻辑；有效行215。
- 父prepaid122→124；fixture208不变，仅将合成token开放给同级测试；新增public_tcp144有效行。没有扩大生产可见性或添加运行时特例。
- 工具会话中途重载，初始构建会话丢失，初始日志为空且无本批Cargo进程/完成凭据；不计为通过。记录build-interruption.json后重新启动构建。
- 第二次locked/offline、runtime-defaults、4 jobs lib构建完成，build-exit.json记录0。
- 完整qwen_web_pipeline_tests实际24 passed、0 failed、1 ignored；含新增3项、既有15项prepaid及原6项公共pipeline/TCP。忽略的是专用浏览器宿主，本批未重复网页验收。
- checker31、ratchet、rustfmt、限定whitespace、UTF-8无BOM通过，无baseline/exception修改；构建前后源码输入无漂移。
- Cargo compiler-artifact定位真实测试EXE，源/副本SHA256一致：`f89da2624f99622ff1556495566ea2f4d5c38df584645948c4affb7f6d192b91`。

证据根：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-qwen-prepaid-tcp-20261112`。最终构建/运行来源为build-2.jsonl、build-exit.json、tests.exe、pipeline.log、test-exit.json；另有inputs.json、input-drift.json、checker.log、ratchet.log及resource-final.json。TEMP/TMP和合成数据均在此，测试进程退出，正常监听任务和peer均结束。

## 范围结论

关闭Qwen普通余额Key与真实下游TCP成功/失败/断连之间的限定联合证据空白。Grok同等级余额Key TCP联合、Server/PG、崩溃恢复、现金账本/倍率、真实供应商及成品包仍未由本批证明。

新增模板仍17，无新增整卡offline_ready，P50/P60/P70未完成。未使用真实凭据/付费上游，未修改在线实例、出应用包、提交或推送。
