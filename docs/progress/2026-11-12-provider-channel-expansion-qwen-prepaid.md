# R07：Qwen 普通 Gateway Key 按次数预付费合同

日期：2026-11-12（Asia/Shanghai）。工作区 `C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway`。本批仅新增测试、模块声明和验收文档，未修改生产计费、协议或路由逻辑。

## 实际验证

通过生产 `build_app_state` 创建隔离 Local/SQLite 状态，关闭静态公共 Key；正常管理接口创建 normal Key 并设置 `message_prepaid`，总量和余额均为 2。创建接口返回的 token 用于公共 Chat/Responses 请求，经正常鉴权、路由、预扣与终态结算访问合成 Qwen create/completion peer，不直接调用私有结算函数。

新增 3 项测试，覆盖 14 个组合：

- 成功：Chat/Responses × buffered/SSE × 已报告/未报告 usage，共 8 组合。成功返回合成答案；已有终态正确输出。已报告 usage 保持 prompt=2/completion=3/total=5，未报告则保持 None，不补零或估算。两种情况余额均 2→1，购买总量不变。
- 截断：Chat/Responses × buffered/SSE，共 4 组合。去掉上游完成事件后关闭 body；buffered 不返回成功，流不伪造完成标记；唯一 failed 审计，余额恢复为 2。
- 取消：Chat/Responses 流式各 1 组合。上游保持打开，实际读到“合成答案”且尚无完成标记；取消前余额为 1，drop 下游 body 后上游连接关闭，唯一 cancelled 审计，余额恢复为 2。

所有组合均确认唯一审计归属新建 Key、exact 账号及 qwen3.7-plus；恰好一次 create 和一次 completion，出站是指定 Bearer/Cookie/UA，未重放、换号或跨渠道。审计不包含公共 Key token、上游 token、clearance 或调用方秘密。等待异步审计释放状态引用后再查最终余额；每个 fixture 请求 shutdown、关闭 SQLite/freebuff，peer 正常结束。

此处下游使用 router oneshot/body drop，不冒称新增真实 TCP 计费断连验收；上游是本地合成 HTTP/SSE。生产 build_app_state 不等于完整服务入口、所有后台 worker 或成品 EXE 验收。取消释放的是本地次数预扣，不承诺 Qwen 远端停止生成或退款。

## 检查结果

- locked/offline、runtime-defaults、4 jobs 的 lib 测试构建通过。
- 新 prepaid 3 passed；原公共 pipeline/TCP 6 passed、浏览器 opt-in 宿主 1 ignored；管理 qwen_web_probe_tests 2 passed。合计 11 passed、1 ignored。未重复未改动的 Grok、余额规则或前端测试。
- checker 31 passed，ratchet、rustfmt、限定 whitespace、UTF-8 无 BOM 检查通过；未修改 baseline/exception。
- 父模块有效行 266→268；新增 prepaid.rs 110 行、prepaid/fixture.rs 169 行。测试只使用合成材料，读取受超时和字节上限约束，沿既有 owner 释放资源，无生产特例。
- Cargo compiler-artifact 返回的真实测试 EXE 复制并比较哈希后执行；SHA256：`2684d05f309806d5dd96478e64a8931de80b65e81ba33a26c28257868fdae7fa`。
- 构建前 Cargo.toml/Cargo.lock/全 src 哈希清单与执行后比较无漂移；本批结论绑定冻结测试 EXE。测试进程已退出。

证据目录：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-qwen-prepaid-20261112`。包含 inputs.json、input-drift.json、build.jsonl、tests.exe、prepaid.log、pipeline.log、probe.log、checker.log、ratchet.log、resource-final.json；TEMP/TMP、合成配置及数据库均留在该目录。

## 剩余范围

Grok 与 Qwen 均已有普通 Key 的 message_prepaid 成功/失败/取消合同。token_prepaid、现金定价与账本、供应商真实扣额、PG/Server、进程崩溃中的预扣恢复、成品包和真实平台验收不由这些测试证明。

新增模板仍为 17，无新增整卡 offline_ready，P50/P60/P70 未完成。未使用真实凭据或付费上游，未修改在线实例，未构建应用包、提交或推送。
