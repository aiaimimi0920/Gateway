# Nous Portal：凭据独占上下文与持久化材料绑定

此记录续接 [received-grant 前置切片](2026-11-09-nous-received-grant.md)，属于 OAuth／订阅优先批次。
它完成 typed storage 合同，不代表 Nous 自动刷新、管理 UI 或完整产品接入已经完成。
DIM 按用户决定继续本地实现，不等待 OAuth Client 注册信息；真实注册与账号验收单列。

后续 [自动 Received worker 子批](2026-11-09-nous-worker-lifecycle.md) 已完成定向验证。
本页“worker 尚未接线”和临时 guard 的描述保留为本切片结束时的历史状态，不是后续实现的当前状态。

## 本子批已实现

- 凭据新增可选 `nous_portal`，消费材料新增可选 `nousPortal`；None 不写出，不改旧 material ID、
  family ID、locator 或 storage version。旧 generic Nous endpoint 的无 receipt Result 仍可读取。
- 上下文只属于单个 credential；DB payload 合并先删除父级该字段，不继承、不递归拼装。
- RefreshMaterial、RefreshAttempt、授权身份比较、generation apply、Result/replay、有限配置投影、
  cache 写回与恢复贯穿完整上下文。新 typed Result 必须有先前保存的 receipt，不能只凭调用方声明。
- receipt 后验校验使用旧 context；successor 省略 inference base 时保留原 Welcome 目标。
  typed Result 的 access、RT、context、绝对 expiry 必须与 receipt 一致。
- Access digest、显式 client、JWT subject hint 与绝对 expiry 绑定；不把未验签 JWT 当作身份认证。
  小数时间不会被 Unix 秒截断后误当相等，不能通过 `.5Z` 给授权续寿。
- Secret Keep 的完整目的地投影纳入 Nous context。保留旧秘密时不能改 client、target、subject hint、
  access digest 或 expiry；正常模型编辑不被阻止。
- 直接 cache 写回和重启修复都核对旧材料。交叉审查指出的直接写回旧 expiry 缺口已修正：
  顶层 expiry 必须等于旧 context；payload expiry 存在时也必须一致，完整 raw CAS 仍保留。

自动 received worker 尚未接线。`Prepared::new` 对 typed Nous 暂时明确拒绝，**不会落入 generic form**，
不会在这一中间阶段把 single-use RT 发进旧通用刷新实现。该 guard 是未完成入口的安全边界，不是可用性验收。

## 实际验证

证据根：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-magpie-expansion-20261108`。
源码候选：`nous-typed-candidate-r5.json`。构建使用 locked/offline、`runtime-defaults`、最多 4 个 Cargo jobs，
仅命令行覆盖 `profile.test.package.gateway.debug=0`；没有增大 `RUST_MIN_STACK` 或修改仓库 profile。

| 日志 | 实际结果与边界 |
| --- | --- |
| `nous-typed-red-r1.log` | 预期 **0 passed / 2 failed**：原凭据读写丢失上下文，父级合并会继承上下文 |
| `nous-typed-green-r3.log` 库筛选 | **28 passed / 0 failed**，包含前置协议合同与本批新增 8 项 |
| 同日志 material 集成 | **2 passed**：公开 credential roundtrip 与 credential-only payload 合并 |
| 同日志 Local receipt 集成筛选 | **7 passed**，是完整 Local suite 的子集，不重复相加 |
| `nous-typed-adjacent-refresh-r5.log` | **152 passed / 4 ignored**；包含本批 7 个 refresh/material/cache 合同 |
| `nous-typed-adjacent-secrets-r5.log` | **15 passed**，包含本批 Keep 合同 |
| `nous-typed-adjacent-payload-r5.log` | **19 passed**；DB payload reader/merge 合同，不是真实 DB 部署验收 |
| `nous-typed-full-local-r5.log` | **14 passed / 6 ignored**；包含上述 7 个 Local receipt 集成合同 |
| `nous-typed-checker-r1.log` | checker tests **31 passed**，checker 本身未变化 |
| `nous-typed-ratchet-r5.log` | 5153 文件、14 个 immutable runtime assets；governed source >700=0 |

筛选之间有重叠，不能把表中 passed 数量当作互不重复的新用例总数。
10 个 ignored 都要求显式 disposable Redis URL，**未执行，不算通过**。
Cache 正反向 CAS 使用现有 loopback Redis wire witness，不等于真实 Redis Lua/Server 验收。

本批真实 Local owner 合同完成 reserve → receipt → Result → 新 runtime 重开，验证 receipt 保留、
消费证据为 Existing、Result 仅在原 request 下 replay。它没有覆盖 Nous 自动 worker 或完整 Published/cache 生命周期。

中间失败均保留：`nous-typed-compile-r1.log` 的 E0502 是新测试借用顺序错误；
`nous-typed-green-r2.log` 为 **26 passed / 1 failed**，新 cache 测试误用了 witness 固定 key 之外的 ID。
后者仅把新测试 ID 对齐 `exact-build`，没有放宽 fixture、生产校验或改成假成功。
后续 r3 绿测同时包含“漂移 0 commits”和“正确输入 1 commit”的断言。

范围内 39 份 Rust 输入最大 498 effective lines，无 >500 文件；rustfmt、UTF-8 无 BOM 与 scoped diff check 通过。
范围哈希与实际三个测试 EXE 的来源核对见 `nous-typed-final-r5.json`。
工作区仍有其他 owner 的改动，明确 `wholeTreeProvenance=false`，不把范围核对称为整树证明。
没有停止其他编译、清空 target、提交、推送、打包、切在线实例、读取真实 profile 或调用付费推理。

## 继续工作与不能越过的边界

1. 用独立 Received outcome 接自动 scheduler，先 durable receipt，后 admission/Result/apply/publication；
   内存 pending、存储失败仅磁盘重试、正常 shutdown flush、重启精确恢复必须走同一 owner。
2. 不能用未 admission 的 raw pair 构造 TokenResponse，不能进入通用 21600 秒默认 TTL。
   历史 receipt 恢复不得重新计时；原消费 grant 结果不明时不得重发旧 RT。
3. 完成 typed declaration、固定 sender、preset/feature/manifest、管理 session、显式保存、目录应用与 UI。
   自动旋转如果改变固定 inference target，须明确 runtime payload 与配置/base publication 的一致性，
   不能仅换 access/context 而把新 token 发往旧目标。
4. 三公共 API 走实际 Chat 转换，继续验证 SSE、函数工具、取消、计费及 Native Codex/Claude/XML 路径隔离。
5. 真实 Redis、Gateway 获准 client、真实授权/轮转、权益/费用、桌面/Server/跨应用仍未验收。
   不借用 `hermes-cli` 或其他客户端身份，资格边界见 [固定来源合同](../providers/nous-contract-2026-11-08.md)。

完整 42 项、红框 23 卡片及聚合子产品仍以 [全范围看板](../providers/magpie-workboard-2026-11-08.md) 为准。
