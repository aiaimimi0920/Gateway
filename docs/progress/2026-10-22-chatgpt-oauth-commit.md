# OAuth 保存事务字段顺序修复

## 已复现根因

用户完成 OAuth 后导入报 `Could not save ChatGPT credential; refresh configuration and retry import`。
实际失败事务在 local backend 的 `store_revision` 阶段回滚，journal 使用通用的 `redis_revision_store_failed` 分类。
只读检查实际归档：JSON/YAML 原始摘要正确，归档读取与 revision 校验通过，但 revision envelope 序列化往返字节不一致。
`serialize_json_canonically` 原先依赖 serde_json Map 默认排序；启用 preserve_order 后，多项 HashMap 在重建时改变字段顺序，导致严格字节比较误判。

## 最小修复

- 内部配置序列化显式 `sort_all_objects()`，递归排序对象键，保持数组顺序与内容不变。
- 不修改上游请求风格，不放宽摘要、immutable payload 或激活 CAS 校验。
- 旧 Redis 无序但语义相同的 payload 沿用既有 CAS normalization 兼容路径。
- OAuth 保存错误保留运行时错误码，日志仅输出分类，不输出凭证或配置内容。
- 修复原 canonical 测试对实现重复计算的弱断言，补充嵌套对象和实际 local OAuth 提交/重启测试。

## 本轮验证

按用户要求仅执行相关测试，不跑全量回归：redis_store 8 项、chatgpt_oauth 10 项、本地配置运行时 7 项全部通过。
新增 OAuth 假凭证保存测试确认事务 Committed，重启后 refresh token 与请求头保持。
同一份实际失败归档修复前往返字节不同，修复后完全相同；未写入生产配置或重放真实 OAuth。
格式化、diff check、有效行数 checker 31 项及 ratchet 通过。
正式 release 编译、1528 文件 SHA-256 校验和隔离本地存储 smoke 通过。

## 交付

- `C:/Users/Public/nas_home/AI/GameEditor/Neuro/release/Gateway/gateway-oauth-save-20261022-r2`
- 源码指纹：`6ee83e54da59030c571b1f872064f510d72fadbd7810347e02fe4033fd5ff2b2`
- gateway.exe：`6e4caecaa2fc27733b5310dce51c100976eed05457bf21bd19e07d98dda839e9`
- gateway-ui.exe：`ae602b161ae5dc0a1fbeaf01295189eb17ef3ddc7dc3679ddd72970aa2e2038f`
- 证据：`linshi/gateway-oauth-commit-20261022`；其中 `isolated` 包含诊断归档副本，不应分享或打包。执行工具拒绝自动清理，需用户手动删除该临时子目录。
- 发布包不含 .ng，旧包与正在运行的实例未覆盖或停止。本记录在打包后写入，不属于上列构建指纹。
