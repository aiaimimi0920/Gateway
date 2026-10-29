# 服务商模板名称与默认 ID 规范化

本轮检查当前添加服务商目录的全部 24 项模板（含 3 个 Web 模板和自定义模板），统一中英文名称与默认 providerId，均以 `-oauth`、`-api` 或 `-web` 结尾。

新增默认 ID：cohere-api、groq-api、nvidia-api、openrouter-api、poe-api、linkup-api、tavily-api、you-api、exa-api、jina-api、websearchapi-api、custom-api。
原已规范的 codex-oauth、openai-api、anthropic-api、gemini-api、azure-api、deepseek-api、mistral-api、xAI-api、perplexity-api，以及 chatgpt-web、gemini-web、aistudio-web 保持 ID。
Gemini、Azure、Mistral 和三个 Web 模板的 labelZh/labelEn 同步为规范名称。WebSearchAPI 是品牌名称，因此保留 websearchapi 主体再加 -api。

## ID 边界

- 新建服务商的 `providers[].id` 和生成的 `model_routes[].provider_ids` 实际使用规范后的 ID，不只是展示标签变化。
- 不改目录内部 templateId 和协议 preset：这些是流程分支、验收记录、后端协议选择的稳定键，不是服务商实例 ID。
- 不对用户已保存的实例 ID 做启动时自动替换。当前没有覆盖账号、权益、路由及历史记录引用的完整重命名流程；直接改旧 ID 不能视为安全迁移。
- 现有账号、密钥、服务商及路由保持不变；新增服务商也不会覆盖同品牌旧实例。已有实例仍可由上一轮的展示规则呈现规范后缀。

## 验证

- 全目录命名与创建合同测试先红（19 项失败），修复后通过。
- 29 个相关测试文件 148/148 通过，包含所有模板、空池创建、账号导入、路由引用、站点健康、旧数据保留和完成标记。
- TypeScript typecheck 通过；effective-lines checker 31/31 与 ratchet 通过；git diff --check 通过。
- providerCatalog 有效行数保持 423，providerWebCatalog 保持 12，新增命名合同测试 31 行。文本使用 UTF-8 无 BOM。
- 本轮无付费 API 请求、数据库迁移、运行实例更新或 EXE 构建；不宣称已有实例 ID 已迁移。
