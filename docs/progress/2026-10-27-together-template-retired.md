# Together 模板移除（2026-10-27）

## 结论与范围

用户在复核中确认不再需要 Together，并明确要求删除服务商模板。本轮停止原定的
目录接入、账号导入和隔离调用验收，改为从标准服务商模板目录移除 `together-openai`。
“添加服务商”不再显示 Together；不登记蓝色完成标记。

仅移除新建入口，不删除后端 preset、协议实现、line manifest、品牌兼容映射或站点健康
适配器，避免破坏已有配置。没有修改当前 4200 实例、账号、凭据、路由或数据库。
用户改需求前只发送过两次官方目录 GET 预检，分别收到 Cloudflare 403 和 HTTP 401；
没有生成请求，没有创建 Together 服务商或导入账号。失败记录保留，不改写为验收成功。

## 改动与验证

- `providerCatalog.ts` 删除 Together 模板定义，其他模板不变。
- `TogetherCatalogTemplate.test.tsx` 改为移除回归：验证模板 ID、Provider ID、preset
  均不再提供，界面无 Together 按钮，相邻 NVIDIA / DeepSeek 入口仍存在。
- 先复现两项预期失败，再移除模板；最终相关 6 个文件 71 项测试通过。
  中间一次失败为新测试未兼容 NVIDIA 完成标记的可访问名称，修正选择器后通过。
- TypeScript、31 项 checker 测试、effective-line ratchet、`git diff --check` 通过。
- 模板文件有效行 450→435；新回归文件低于 100 行，无超限增长。

证据目录：
`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-together-template-acceptance-20261027/`。
原复核红测、预检及移除回归的红绿记录分别保留；Key 和原始响应仅位于私有子目录。

未执行：后续上游请求、4300 运行验收、新 EXE 编译、发布、提交、推送或 live 数据修改。
本轮为源码改动，已运行的旧 EXE 不会自动更新，待下一次用户要求出包时纳入。
