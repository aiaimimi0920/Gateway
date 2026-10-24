# OpenRouter 模型刷新与 DeepSeek / Qwen 分组

## 原因

当前本地 OpenRouter 配置仅保存少量免费模型；卡片展示配置能力，并非远端完整目录。
旧配置采用 `preset: openai`，被仅允许无 preset 兼容适配器的自动识别入口排除。
通用地址候选也没有保留 OpenRouter 的 `/api/v1` 协议前缀。
DeepSeek、Qwen 的展示目录只有少数旧型号，没有新版模型家族兜底。

## 改动

- 前后端允许官方 OpenRouter `/api`、`/api/v1` 地址下的 OpenAI preset 显式刷新。
- 使用固定同源 `/api/v1/models` 目录及 Chat Completions 探测，不扫描其他协议。
- 目录读取上限为 8 MiB，模型数量继续限制为 2048；普通发现响应仍为 1 MiB。
- 公共目录不等于凭证有效：仍需至少一个真实生成结果才能保存发现记录。
  `verified_models` 只记录实际探测成功的模型，不把整个目录标为已验证。
- 沿用 secret grant、revision 和凭证绑定检查，不绕过原有显式保存逻辑。
- DeepSeek、Qwen 新版模型、常见后缀及已知命名空间归入对应公司组。
  OpenRouter `~deepseek` 动态别名只参与展示归类，不改模型 ID。
- 不修改当前运行配置，不硬编码新增可调用模型，不变更原有路由优先级。

## 使用与验证边界

新版本中，对有效 OpenRouter 账号执行显式自动识别/刷新后，目录模型才会进入账号能力列表。
返回的 `deepseek/deepseek-v4.1-flash` 与裸名称是不同配置 ID；本改动不自动建立别名。
刷新会沿用现有流程保存模型能力和对应路由，因此应由操作者明确触发。
已补前端分组、刷新资格和后端地址回归用例；按用户要求，不执行测试、构建或重启。
