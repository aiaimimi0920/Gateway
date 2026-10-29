# 服务商 ID 在线迁移：等待中的授权来源刷新

承接 `2026-10-28-provider-id-authorization.md`。本轮仅完成等待重试的授权上下文
修复，不等于候选身份在线切换或真实数据库迁移完成。

后续已关闭本文记录的网络准入回归，见 `2026-10-28-provider-id-network-admission.md`。
本文的首次失败和夹具尝试记录保留，最新验收结果以后续记录为准。

## 改动

- 将请求中的显式 fallback 权限按来源分开：原字段保存路由/访问投影授权，新增
  私有字段保存本地 Key 授权；发送时检查两者的并集，仍先经过路由 policy 检查。
- 路由开始时同时清空两类请求局部权限。本地 Key 首次授权写入自己的字段，不再
  混入路由字段；不改变候选生成、过滤、排序和账号组交集。
- 等待后的 `refresh_candidate` 接收可变请求上下文，读取重新鉴权返回的 IDs。
  只有候选刷新、账号组检查和 Key 鉴权全部成功才替换 Key 权限。失败继续返回
  原错误，不执行新一轮发送，不累计旧 Key 权限，也不抹掉独立的路由授权。
- 无 provider 限制的 Key 返回 None，不代表显式授权任意跨服务商 fallback；空集合
  同样不会新增 fallback 权限。别名权限只消费本地授权层返回的已验证集合。
- 本轮不修改凭证、持久化格式、请求预算、配额预留、候选投影或流式交付行为；
  新增一个请求局部 Vec，权限规模仍由本地 Key/别名层的既有上限约束。

## 验证与首次失败

- 新增 3 项请求上下文测试：替换 Key 权限时移除旧授权且保留路由权限；None/空集合
  不授权任意 fallback；精确 ID/大小写匹配且不生成候选。3/3 通过。
- 本地 access key 回归 18/18 通过，包含上一阶段的别名、撤销和密钥保留测试。
- 隔离回环 `access_key_groups_contract recovery_wait` 5/5 通过：JSON/SSE 恢复、
  原始截止时间、手动禁用/账号组撤销、真实 HTTP 断连后不后台重放。
- 首次 `--lib pipeline::` 为 115 passed / 1 failed / 1 ignored。失败位置为
  `stage_route/tests/egress.rs` 的立即拒绝断言：夹具未声明等待策略，当前默认等待
  会保留临时网络故障候选。尝试显式关闭等待后，第二次仍为 115 passed / 1 failed /
  1 ignored：虽立即拒绝，但错误 code 为 None，而非 `gateway_network_unstable`。
  `stage_route.rs` 的空候选路径返回通用无服务商错误，说明不能只靠调整等待夹具
  解决此处契约。已撤回本轮夹具尝试，保留原测试和断言；该项未解决，不宣称全绿。
  唯一 ignored 项需要独立 Redis，本轮未配置或启动。
- checker 31/31、有效行 ratchet、scoped rustfmt 与 `git diff --check` 通过。
  Rust 编译通过，仍有工作区既有 unused 等 warnings，未执行 cargo fix。

## 代码规模与边界

修改前后有效行：pipeline/mod 399→402，stage_route 481→482，stage_send 488→485，
recovery_wait 265→267；新增 fallback_authorization 含测试 70 行。egress 测试恢复
本轮开始时的内容。修改前工作树已有其他线程内容，本记录仅归属以上授权来源改动。

等待中的候选仍按原 provider ID 精确匹配；映射与路由发布、同一账号身份匹配、
RequestWaitPolicy 查找、审计/价格/压力/refill 引用和恢复协调器尚未接齐。
不能据此开放迁移入口，也不能让新旧 EXE 并发操作真实存储。

4200 仅执行健康检查，未停止、drain、重启或替换进程；未修改真实配置和数据库，
没有上游付费调用，没有交付新 EXE。
