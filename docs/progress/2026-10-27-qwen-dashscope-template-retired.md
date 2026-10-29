# Qwen DashScope 产品支持移除（2026-10-27）

## 范围

按用户决定移除 `qwen-dashscope-openai`，并在用户明确回复“同意”后继续清理
专属后端兼容实现及配置。此项为产品取舍，不新增或宣称官方价格、免费额度的核验结论。

第一阶段仅移除新建模板；第二阶段已删除该产品 preset、旧 `qwen` preset 别名、
专属 profile/URL 推断、实现线归属、账号目录映射、品牌及旧验收登记，以及两份默认
routes 中的完整服务商块。不保留能通过内置 preset 恢复该产品的入口。

保留 Qwen Coding Plan、Qwen Web、共用 OpenAI-compatible / Anthropic-compatible
协议和通用 DashScope 原生协议引擎。共享 `qwen/official-api.json` manifest 改为描述
Coding Plan；未删除它依赖的 feature，也未给自定义通用协议添加域名黑名单。

## 当前运行配置

只读盘点当前 4200 实例使用的 `.ng-devtest-4300`：生效 revision 为
`r19-46c326392f84`，共 11 个服务商；不存在 id/preset 为 `qwen` 或
`qwen-dashscope-openai`、profile 为 `qwen_dashscope_openai` 的服务商，指向该产品
的路由为 0，`local/routes.yaml` 也无对应引用。因此没有需要删除的生效配置，未改写
当前数据库、凭据或路由；保留历史审计、旧 revisions 和旧包。没有上游调用或实例重启。

## 第一阶段验证

- 新增目录及 UI 移除回归；先确认两项测试因旧模板仍存在而失败，再删除模板。
- 移除旧目录测试对该模板必须存在的断言；相邻 Mistral / xAI 入口仍存在。
- 相关测试同时发现 Cohere 已登记、面板回归期望却未更新，补齐完成标记的期望值，
  不改写验收登记或真实调用结果。
- 最终相关 8 文件 79 项测试、TypeScript、31 项 checker 测试、有效行 ratchet、
  UTF-8 无 BOM 与 `git diff --check` 通过。
- 有效行：模板 435→420，目录测试 84→83，面板测试 102→103，新回归 20；无超限增长。

## 完整清理验证

- 新增 Python 移除合同：先确认 3 项测试因旧实现失败（含子断言共 16 failures），
  清理后 3/3 通过；生产范围精确搜索只剩负向回归中的退役名称。
- 相关前端 7 文件 67/67、TypeScript、provider-reference 生成及校验、44 项
  line manifests 校验通过。沿用已有包含 `jsonschema` 的隔离解释器，不安装依赖。
- scoped rustfmt、31 项有效行 checker 测试、ratchet、UTF-8 无 BOM 和
  `git diff --check` 通过；本轮受影响源码均不超过 500 有效行。
- 有效行示例：preset 171→121、enabled 协议 156→141、disabled 协议 67→55、
  账号分类 412→411、目录布局测试 307→289；新 Python 回归为 38 有效行。
- Rust 定向执行通过：Qwen 50、协议 registry 22、preset 50、实现线 37、账号目录同步
  141（另 14 项既有测试 ignored）；计 300 次通过，部分过滤器会覆盖同一测试。
  `--no-default-features --features runtime-defaults` 编译也通过，覆盖 disabled 分支。
- devtest 构建/打包合同 6/6 再次通过；两份默认 routes 相对本轮前态仅删除目标完整块，
  其余文本保持一致。未额外安装 YAML 解析依赖或声称已做该解析器验证。
- 并行 OpenAI/目录改动合入工作树后，重新执行移除合同 3/3、相关前端 7 文件 77/77
  和 TypeScript，通过。前端先有两次自定义服务商用例的默认 5 秒超时，当时同时存在多个
  Rust 编译进程、可用内存约 3.18 GiB；随后单用例完成全部断言，用时 4.69 秒。
  最后七文件采用 `--maxWorkers=1 --testTimeout=20000` 通过，未修改产品代码或测试断言。

前两个包 `gateway-devtest-cohere-4200-20261027-045634` 和
`gateway-devtest-cohere-catalog-4200-20261027-051413` 均保留，但都早于完整后端清理，
不能作为最终包含清理的交付。后续使用官方 desktop devtest 流程另建不可覆盖的新目录，
端口 4200；不自动启动新包，不替换当前实例。

首轮 `gateway-devtest-qwen-retired-4200-20261027-114925` 在双 EXE 编译完成后，
因构建期间另一个任务修改 OpenAI/目录源码而退出 1：
`Source changed during build; no devtest receipt published.` 未发布包、未绕过来源校验。
该失败证据保留在 `linshi/gateway-desktop-build-qwen-retired-20261027-114925/`。

重试计划为 `gateway-devtest-qwen-retired-4200-20261027-120250`；构建退出码、源码
指纹、完整性及 EXE 哈希以独立打包证据
`linshi/gateway-desktop-build-qwen-retired-20261027-120250/` 中的回执为准。因为观察到
另一项 API 版构建共用 target，另将打包后端哈希与启用 `management-ui` 的 Cargo
feature-specific 输出对照，防止跨 edition 覆盖。本文不将计划输出冒充已验证产物。
源码文档在重试前冻结，避免打包后改写来源指纹。

第一阶段证据：`linshi/gateway-qwen-dashscope-template-retired-20261027/`。
完整清理证据：`linshi/gateway-qwen-dashscope-removal-20261027/`。
未提交或推送；未删除其他服务商、账号、历史数据或旧包。
