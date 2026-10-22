# 流式成功后的上游优先复用

原非流式成功路径写入 session/user 或 projected-access affinity，流式终结回调
仅更新审计、用量与健康记录，未写入相同 affinity，导致后续请求再次按一般队列选择。

新增 stage_finalize/affinity.rs 作为共享成功写入 owner。只有流的成功终结回调写入，
失败、取消和中断仍走 failure 路径；已有健康、冷却和授权过滤不变。
两种成功路径使用客户端 requested_model 作为亲和键，与 stage_route 读取一致，
避免模型别名或上游模型映射导致写入键与读取键不一致。

新增真实 HTTP 流测试：先取消一次成功候选的流，确认高权重健康默认候选仍排第一；
随后完整消费成功候选的流，下一次实际 stage_route 选择并调用该成功候选。
默认候选保持健康且高权重，未被故障过滤，因此不会以健康降级造成假阳性。
测试包含 caller-alias → upstream-model 映射，两个完成请求均访问成功候选。

验证：pipeline_send_runtime_contract 32 项通过；checker 31 项与 ratchet 通过。
本轮未运行全量回归，未改动正式数据、运行实例或重新发布 release；r4 不含该修复。
复跑曾遇到 linshi 内固定版本 CPA 原生依赖准备目录缺失，使用仓库 prepare 脚本重建，
没有修改系统 Cargo 缓存或依赖版本。
