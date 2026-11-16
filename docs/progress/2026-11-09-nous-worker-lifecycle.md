# Nous Portal：自动刷新 Received 生命周期

此子批续接 [typed storage](2026-11-09-nous-typed-material.md)，仍属于 OAuth／订阅优先路线。
DIM 按用户决定先完成本地实现，不等待 OAuth Client 注册。完整范围仍是原三图 42 项、红框
23 张卡片及聚合卡子产品，不因本页的单个切片收窄。

## 实现与验收边界

- 自动 scheduler 使用专用 Nous Prepared／Received outcome；reserve 前只验证与构造请求，
  reserve 后才发送一次。RT 仅在 `x-nous-refresh-token`，不进入 form，也不带 inference Authorization。
- 收到 successor 后保留完整原始 receipt 和 single-flight guard，先由既有 Local／Server authority
  保存 receipt，再后验 JWT hint、scope、subject continuity、expiry 和固定 inference target。
  未 admission 的材料不能进入通用 TokenResponse 或 21600 秒默认 TTL。
- Receipt 写入失败或 ACK 不确定时，pending 保留 raw successor；下一轮只重试存储，不重复 HTTP。
  正常 shutdown 等待 in-flight 返回，再沿同一 PendingWrite owner 做一次 persistence-only flush。
- 只恢复同 provider／credential／revision／material 的 durable receipt；Sending 无 receipt 不自动重发。
  非法 successor 保留消费证据但不生成 Result，Nous 专属 quarantine 会挡住尚未过期的旧 access；
  不改变 generic OAuth 的既有“旧 access 尚有效”行为。
- 历史 receipt 按原 received_at 解释，保留绝对 expiry；合法但现在已过期的历史 Result 可以落盘，
  不能用于推理，后续只能刷新其 successor RT，不能重新计时或回用已消费的旧 RT。
- Typed declaration 与最终 credential payload 校验显式 client、context／access／RT／expiry 绑定、
  固定目标和身份互斥；已过期 access 仍可编译进入刷新。旧 generic endpoint 记录不被重分类。
- Welcome→Subscription 或反向旋转时，一次 material lock 同时读取 access／expiry／target；
  只设置该 credential 的固定 base override，不改池默认值或同池其他账号。
  有限投影、publication reuse、already_saved、Published fence 和 cache CAS 都核对对应新旧目标。

强制 task abort／进程崩溃发生在收到 response 后但 receipt 尚未 durable 的窗口时，不能恢复从未
落盘的 successor；永久 Sending fence 只保证不自动重用旧 RT。存储持续故障时 shutdown flush
也可能失败并明确记录错误，不能承诺任意故障下无损恢复。

## 本地验证记录

证据根：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-magpie-expansion-20261108`。
构建使用 locked/offline、`runtime-defaults`、4 个 Cargo jobs、命令行
`profile.test.package.gateway.debug=0`；未改仓库 profile 或增大 `RUST_MIN_STACK`。

- `nous-worker-red-r1.log`：实际 **0 passed / 1 failed**；临时 guard 阻止网络，期望一次调用但实际为 0。
  红测结束后才修改生产实现。编译期间补充的后续测试不冒称已经属于该旧二进制。
- `nous-worker-checker-r1.log`：checker tests **31 passed**。
- `nous-worker-ratchet-r2.log`：最终候选 5163 文件、14 immutable runtime assets，governed source >700=0。
- `nous-worker-green-r1.log`：**39 passed / 0 failed**，包含 11 项新增 worker／配置／目标生命周期用例。
- `nous-worker-adjacent-refresh-r2.log`：**163 passed / 4 ignored**。
- `nous-worker-adjacent-state-r2.log`：**9 passed**，覆盖 authorization handoff 与刷新状态。
- `nous-worker-adjacent-secrets-r2.log`：**15 passed**。
- `nous-worker-integration-r2.log`：Local grant 集成 **14 passed / 6 ignored**，material 集成 **2 passed**。

上述筛选有重叠，不相加成独立新增用例总数。10 个 ignored 要求显式 disposable Redis URL，
未运行，不算通过。Server receipt ACK 丢失／shutdown 测试使用严格的合成 authority 和 loopback
Redis wire witness，不等于真实 Redis Lua／部署验收。

候选范围快照为 `nous-worker-candidate-r2.json`；r1 后的小修只将 credential raw base override
检查限定到 Nous，保留 generic 账号原有的等效 base 合同，修订发生在本次 Cargo 等待共享构建锁时。
范围内 82 份 Rust 输入最大 486 effective lines；没有修改 baseline 或 exceptions。
独立只读审查未发现新增确定阻断问题；审查不能代替编译和实际测试。

最终范围核对见 `nous-worker-final-r2.json`：`drift=[]`、UTF-8 无 BOM、rustfmt check 与
scoped diff check 通过，记录实际运行的 3 个测试 EXE SHA-256。库测试 EXE 的最终 SHA-256 为
`5f4b84bf30b8430d36046ab795b98b057165b7b8b9b4fa14f686f9d4ae8cfdd3`。
这是范围内来源核对，明确 `wholeTreeProvenance=false`，不覆盖共享脏树中其他 owner 的输入。
这些 EXE 是测试 harness，不是手测交付包；未执行正式发布、全库测试或付费上游验收。

## 尚未完成

Nous 管理 session／显式保存、目录应用与 UI、preset／feature／manifest、固定 inference sender，
三公共 API 的 Chat 转换、SSE／函数工具／取消／计费及其他产品路径隔离仍待贯通。
本地测试不证明 Gateway 已取得获准 client，不证明真实授权／旋转、权益／费用或真实 Redis Lua、
桌面／Server 部署与跨应用验收；不会借用 `hermes-cli`，见 [资格合同](../providers/nous-contract-2026-11-08.md)。
没有停止其他 owner 的编译、清空 target、提交、推送、打包、切在线实例、读取真实 profile 或付费推理。
