# Command Code 模型协议声明保存

首批三个服务商按 Command Code → SenseNova Token Plan → SAP AI Core 收口。
本块解决 Command Code 目录读取后丢失 supported_endpoints 的问题；不是原生协议接通回执。

## 改动

- ModelCatalog 增加可选 supportedEndpoints 模型到路由映射，读取目录时保存已导入模型的
  已知协议声明。未知未来路由不推断为现有协议；重复模型保持首个可导入条目胜出的既有语义。
- 非空声明仅属于 Command Code 固定目录，模型键必须和 models 完全对应，路由非空、
  去重且受白名单约束。前后端保持相同校验，普通表单应用和 JSON 保存保留该字段。
- 旧目录无需迁移，没有声明时保持缺失，不补造协议。其他服务商目录仍省略该字段；
  构造器补空值不改变其目录语义。
- 保持只导入 Chat-capable 模型。Messages-only／Responses-only 尚不纳入当前执行池，
  避免目录扩张先于 runtime 接通。metadata 不证明账号权益，不改变真实发送协议。

## 验证

证据根：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-first-three-20261113`。

- `endpoint-rust.log`：Command Code 定向回归 **35 passed / 0 failed / 0 ignored**，
  包含新增两个 metadata 测试和现有目录、管理、推理、调度测试；编译 11m29s，测试 70.76s。
  复用 runtime-defaults,copilot-sdk-runtime 的已有构建配置，不是独立 feature 通过证据。
- `endpoint-ui-regression.log`：产品目录模板、账号目录读取、编辑及新增协议保存测试，
  **4 files / 15 tests passed**。单独先跑的 3 项已包含于这 15 项，不重复计数。
- `endpoint-typecheck.log`：tsc --noEmit 通过。
- `endpoint-catalog-regression.log`：复用同一测试 EXE 的共享 model_catalog 回归
  **47 passed / 0 failed / 0 ignored**，测试 30.24s；与上述产品筛选存在重叠，不相加计数。
- checker 31 项、ratchet、定向 rustfmt 与 scoped diff 检查通过。
- `endpoint-lines.json`：目录主模块 393 有效行（此前 384）、新协议声明模块 92、
  目录测试 115、前端 schema 129、新前端测试 34；未改 baseline。

## 未完成

原生 Messages／Responses 的选路、请求、回复、流转换及专属 probe 尚未接入此字段。
持久化目前验证 schema roundtrip 与前端正常应用／序列化，不冒称已执行完整管理 CAS
到磁盘重启的新增字段验收。独立 feature 仍有先前复现的 S06/Gemini disabled surface
阻碍，不能以本次 runtime-defaults 回归抵扣。其余产品功能、包、Server 与真实账号门槛
仍按 [首批收口清单](../providers/first-three-closure-2026-11-13.md) 跟踪。
未调用真实服务商、修改真实凭据、出包、部署、提交或推送；Command Code 仍为开发中。
