# R14：Luma Inference 正常网页联合验收

日期：2026-11-20（Asia/Shanghai）。承接独立 Inference 渠道与 Files 网页联合子批。范围为正常网页中的精确账号文本测试、可见结果和管理／上游／本地审计联合；没有运行真实供应商业务。

## 实现与验收范围

正常账号测试弹窗原先执行请求后只保留 `runs.responses`，没有渲染测试结果。定向回归实际复现“找不到测试结果区”，随后复用已有 `CredentialTestResults`，在独立弹窗的滚动表单内显示本轮账号、模型状态和可展开的实际答案。嵌入式计划编辑器继续由计划列表呈现结果，没有增加自动测试或额外读取请求。

持续入口为 `tools/luma/inference-browser-check.mjs` 与 `src/protocol/luma_inference/wire_tests/browser_host.rs`；复用 Files 的正常网页静态服务和透明管理代理。脚本、fixture 和[运行说明](../../tools/luma/README.md)均在仓库，不依赖临时目录中的旧 EXE、浏览器 profile 或数据库。

浏览器通过正常登录／敏感授权进入 exact 账号，配置唯一提示词及别名模型，先关闭再重开，确认配置操作没有调用；只在显式运行后发出一次管理请求。页面必须显示可调用模型及“合成答案”，保存宽窄屏截图。后端独立核对一次 `/v1/chat/completions`、精确 Key、映射后的 `glm-5.2`、提示词、非流式／256 token 上限、凭据头隔离、2/3/5 token 审计、普通 Key 余额 2 和 revision 不变。

管理文本测试的既有审计语义是记录已映射的模型，assessment 仍保留用户选择的别名。此处没有改变共享审计或公共 API 的模型身份语义。

## 验证记录

最终浏览器脚本与 Rust 宿主双通过，后端 **1 passed / 0 failed / 0 ignored**，修复后的前端 **12 passed / 0 failed**。正常 main 构建、TypeScript、formatter、Node 语法、规模门禁与 diff 检查通过；checker 31 项复用同轮 Files 未变检查器的回执。没有重跑前批全部 19 项 Inference 后端或 108 项前端，也没有把它们计入本轮。

长期回执：[verification.json](../evidence/luma/gateway-luma-inference-browser-20261120/verification.json)。最终只产生一次管理请求和一次合成 Chat 请求；2/3/5 token、精确账号、余额 2 和 revision 均匹配。两次宿主的进程与端口均已退出，没有本轮浏览器进程残留。40 份关键日志、截图、AX、审计和哈希已逐份校验归档；没有复制数据库、profile 或完整构建目录。六项直接相关源码哈希在编译期间采集并于验收时复核未变，不代表全仓编译前冻结。

首轮新增的后端宿主在浏览器成功后断言失败：误用了公共 API 的 `assert_identity`，把管理审计的 `requested_model` 期望为公共别名。核对 `console_model_probe` 与 `model_probe_recording` 后修正为现有管理合同，并继续断言实际请求模型、精确账号、用量与别名 assessment。失败回执保留，不计作联合验收通过。后端在该断言前已关闭管理 TCP、隔离 runtime 和合成上游。

第一版结果区放在滚动表单外，实际截图显示它挤压配置区；最终位置移入已有滚动容器，1440px 与 390px 截图已实际检查，答案可见，底部关闭和运行按钮可操作。未新增样式系统或结果组件。

## 边界

合成测试不证明销售账号开通、真实模型资格、供应商限速／账单、Server/PG/Redis、跨进程恢复或新原生包。MCP 创作、Web 当前协议、取消后／截止后费用追账等继续列于[剩余清单](../providers/luma-development-status.md)。R14、P50/P60/P70 和 R01–R16 总计划仍未整体完成。
