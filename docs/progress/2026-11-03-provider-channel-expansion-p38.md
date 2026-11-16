# 服务商分渠道扩展：P38 Perplexity Web 材料与目的地保护前置

> 2026-11-03；本批材料/目标保护前置及180项不同Rust测试、33项前端测试已通过。R12/W08、P50/P60/P70与总目标继续执行，不把材料前置算作完整渠道。
> 工作区 `C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway`；`main`，HEAD `7969f12c85efceacf2a5e384e143b5c39a550981`。保护前序/并行脏改，未提交或推送。

## 1. 范围与实际取舍

P37 request/parser/HTTP owner继续保留。本批原拟接完整正常入口；调查确认通用session导入器没有浏览器Cookie属性校验，而canonical response尚无独立来源列表。因此先完成材料及后端目的地安全前置，不静默丢引用，也不提前开放不能正常执行的模板。

- 新增前端专用纯parser：显式token/header与浏览器storageState两种材料边界分开；仅规范化本站session和可选clearance，校验属性、到期时间、连续分片和预算。不自动登录、读取浏览器、扫描历史材料或联网。
- 新增后端产品/规范化材料owner：原provider声明、合并后parent及credential、无flow早返回前的prepare与发送Session构造均保护固定消费端，拒绝API/OAuth/共享材料/任意headers/body/目标覆盖。`api_key`仍只是原有秘密槽名，不改变材料实际为Web session的含义。
- Keep在完整resolved文档上复用原有目的地/继承消费者保护，并增加UA与绝对expiry绑定。改目标、删除标记、改变UA/expiry时必须明确replace/clear，不能无Secret Grant保留旧会话秘密。
- 新增typed special枚举和稳定身份常量；**没有注册preset/profile/implementation line、前端selector/catalog、正常材料表单/提交或公共dispatcher**。空账号可存储不代表可发送，过期材料不自动刷新。
- 源码检查发现未知adapter会进入`custom_http`的raw-body planner；本批增加明确的未接线拒绝，防止材料前置意外启动错误发送。该拒绝是安全边界，不是已实现的服务商功能。

精确支持/材料格式与预算见[实现合同](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/provider-perplexity-web.md)。规范化token的形状、Cookie存在、属性正确均不证明登录、订阅或无限额度；与Sonar/Search关系继续unknown并隔离。

## 2. 安全及验证边界

裸token/header模式是操作者显式声明，不提供domain/path/secure来源证明；storageState模式才检查这些属性。只支持明确的Playwright Cookie形状，不自动猜测其他浏览器导出格式。原始localStorage/其他Cookie/远程目标不投影，错误固定文案不回显材料。Cookie值不URL decode，浏览器分片保留数字顺序并限制总长；已知expiry只收窄，未知不伪造。

Session不实现Debug，CookieHeaderValue保持sensitive；`from_payload`绑定完整产品和原绝对expiry，未来dispatcher必须使用它。`from_tokens`仍只是低层未分片值构造器，不具备payload的目标/expiry校验。空材料、已过期材料可保存，但不能构造可发送Session。后端配置禁止provider共享材料和混池，不新建平行secret owner，不改存储version或重写历史数据。

跨语言合成回环为：真实前端parser导出3种规范化材料→测试中显式构造draft/patch投影→真实Rust SecretPatch resolver→YAML重载→redaction/Keep→正常配置编译/Session构造，并验证disabled账号未被启用。**中间draft/patch由测试构造，不是尚未接通的正常UI apply/save链，也不证明管理HTTP/CAS或真实业务已验收。**

两位只读审查分别复核浏览器材料边界及后端原声明/继承/prepare/Keep。未发现该限定范围的明确绕过；补齐expiry、多标记同时删除及混入API账号的断言。审查不代替运行证据。主线程额外查出generic planner fallback，另补红绿测试，不能因为审查未发现就省略。

## 3. 本批验证

证据：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-provider-expansion-20261103-p38`；只有合成材料，无真实账号或上游费用。

| 检查 | 结果 |
| --- | --- |
| Keep行为红测 | 实际3项：2 passed / 1 failed；旧逻辑允许保留会话秘密并修改API目标，`red-backend-v1.log` |
| 新前端parser | 32 passed；`frontend-v1.log` |
| 前端parser及合成导出 | 最终33 passed（32 parser+1实际导出），`frontend-v3.log`；不累计v1/v2重复执行 |
| 后端编译检查 | `--test perplexity_web_material_contract` exit0，`check-backend-v1.log` |
| generic planner行为红测 | 1 failed；实际生成指向消费端origin的raw Responses RequestPlan，`red-dispatch-v1.log` |
| 新后端最终定向/跨语言回环 | 11 passed（Keep4+产品/配置5+跨语言1+planner拒绝1），`green-backend-v2.log`；v1的10项不重复累计 |
| 共享秘密/文档回归 | 101 passed / 1既存opt-in ignored；Console70、Bedrock10+3、Qwen13+5，`shared-secrets-v1.log`；其中4项与上述Keep重复 |
| 既有Perplexity/call_flow | 32 passed / 40 passed，`perplexity-regression-v1.log`、`call-flow-v1.log` |
| 代表性feature编译 | `--features line-gemini-web-reverse --lib` exit0，`feature-representative-v1.log`；不是全feature矩阵 |
| 类型检查 | 新增parser及roundtrip均已纳入；最终exit0，`typecheck-final.log` |
| checker/ratchet/Rustfmt | checker31/31；最终源码ratchet/Rustfmt/diff均exit0，`ratchet-final.log`、`rustfmt-final.log`、`diff-check-v1.log` |
| 源码与文本审计 | 20源码+4文档的严格UTF-8无BOM/尾空白/hash审计，最终结果见`audit-final.json`；不等于真实平台验收 |

`test-summary.json`按test target+完整名称去重：**180个不同Rust测试通过**，另1项旧session材料导出opt-in ignored；不是把11+101+32+40直接相加。前端33项、checker31项分别记录。最后只给前端输入预算增加分配前长度拒绝，前端33项/typecheck已重跑；`frontend-materials-v3.json`与已通过Rust回环的`frontend-materials.json`逐字节相同，Rust源码未变，不无故重跑同一回环。

20项源码以官方lexer测量、最终冻结于`source-frozen-v3.json`；最大203有效行。既有文件前后值：protocol入口38→39、typed flow136→138、prepare137→138、Session97→110、config202→203、provider编译186→189、Keep183→188、文档合同入口45→47、planner154→160；新文件最大195，无501–700例外，不改baseline/exception。20项最终hash与P37其余11项hash由`audit-final.json`再次核对；`scoped-source-final.diff`及20项独立whitespace检查只包含本批必要增量。

首次尝试把官方checker JSON报告定向到linshi，被其repository-relative限制拒绝（exit2）；没有写原有报告。随后使用同一checker的无报告入口，ratchet exit0，保留两份日志；不以此修改治理脚本或基线。

后端命令固定`--locked --jobs 4 --no-default-features --features runtime-defaults`；TEMP/TMP/TMPDIR放本批linshi，测试进程`RUST_MIN_STACK=16777216`。不启用management-ui发布/删除hook，不升级依赖、不重装/清缓存。完全关闭所有feature的既存Gemini缺符号失败仍见P37，不无故重试或接管该lane。

## 4. 后续最小任务与总体剩余

1. 注册独立preset/profile/line/feature和前端selector；在真正dispatcher可执行后再开放正常空池模板，不借Sonar/Search。
2. 接专用材料表单、暂存后再核目标、SecretPatch/grant/原revision/CAS的真实正常提交；本批parser和合成投影不能代替此项。
3. 明确canonical/客户端的来源列表投影，不静默丢引用；正确区分原请求alias与选路后的`turbo`，不偷偷丢弃不支持参数。
4. 接精确账号probe及auth/filter/route/send/finalize公共链，覆盖buffered/适用聚合SSE、错误不重提/不跨渠道fallback、取消与实际归属。不冒称实时token streaming。
5. 后续集中录制再核实真实session/challenge、终态/来源profile、模型权限、额度与厂商使用边界；现在不逐项要求登录。

其他原计划Accio/Kiro/Vertex OAuth、Qwen OAuth受限决策、Grok剩余登录/恢复/刷新/业务边界、其余API/媒体及P50/P60/P70仍未完成。新增前端模板仍为P36的10个；本批没有新增完整offline_ready平台卡。

本批未读取或试用历史凭据、未调用真实平台、未创建Redis/Docker资源、未出包/替换在线实例、未提交/推送。P34单容器许可已在前序消费并清理，未复用。
