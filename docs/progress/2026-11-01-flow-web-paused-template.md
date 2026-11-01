# Flow Web：保留模板与专用导入，暂停生成开发

日期：2026-11-01，Asia/Shanghai。

## 用户确认与范围

用户确认将 `flow-web` 暂记为未完成，暂停其生成路线开发，保留服务商模板、服务商创建及专用账号导入。
本轮不再次登录、不请求生成、不研究或获取 fresh proof，不改当前运行实例、账号或数据库。

## 实际状态与改动

- 已核对现有独立 `flow-web` 标准模板、Rust preset 和 `flow_web` 专用适配器注册，无需重复添加。
- 模板仍可创建独立服务商，账号池初始为空，不制造生成模型或路由，也不混同 `flowmusic-web`。
- 已核对保存后的 preset 选择 Google Flow 专属账号导入：可选账号名称、普通网页登录或网页会话 JSON。
- 保留敏感访问授权、目标绑定、revision/CAS 保存和已有账号；导入不触发图片／视频生成。
- 新增独立的暂停状态，不写入完成验收登记。目录仍为 `pending`、非蓝色，title 与可访问状态为“未完成（已暂停）”。
- 暂停不禁用添加服务商或导入控件，也不静默改变其他服务商的验收状态。
- 图片仍未闭环、视频未验收；保持原有生成 fail-closed。恢复生成开发须用户另行明确要求。

## 本轮验证

暂停状态测试先复现失败：Flow 模板没有 `data-development="paused"`；最小接线后转绿。

- 六个相关前端测试文件：46/46 通过，包含模板添加、专属导入、真实组件组合的网页登录启动和 JSON 普通保存链。
- 上述为本地合成界面／合同回归，不是真实 Google 登录或生成验收。
- `npm run typecheck`：通过。
- effective-line checker 自测：31/31；ratchet：通过，governed source >700 为 0。
- `git diff --check`：通过。
- 本轮变更文本：UTF-8 无 BOM。

有效代码行（仓库 lexer 实测，本轮前 → 后）：

| 文件 | 前 | 后 |
| --- | ---: | ---: |
| `providerTemplateAdaptation.ts` | 141 | 145 |
| `ProviderCatalogDirectory.tsx` | 31 | 35 |
| `ProviderCatalogDialog.test.tsx` | 122 | 140 |
| `BrowserConsoleApp.google-web-import.test.tsx` | 66 | 68 |
| `BrowserConsoleApp.google-web-json.test.tsx` | 53 | 61 |

本轮没有打包 EXE，没有向当前运行实例添加服务商／账号，没有提交或推送；运行中的旧版本不会因源码修改自动更新。
