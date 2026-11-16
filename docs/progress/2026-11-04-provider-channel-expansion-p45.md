# P45：Accio 正常空池模板与桌面 UI

日期：2026-11-04（Asia/Shanghai）。本批模板/UI 子链完成定向离线验证；总目标仍 active，R01/W01 不标整卡 `offline_ready`。新增模板记录由 11 增至 12，不把模板存在当真实可用验收。

## 1. 实现范围

- 正常目录增加 `accio-oauth`，默认 ID 同名但可改名；复用 `preset: accio`、固定 `https://phoenix-gw.alibaba.com` 和专用 call flow。最终配置投影拒绝改向，新模板默认空 credentials、空 supported_models、无新增路由。未改变旧 preset 的运行默认模型，未迁移历史手工配置，亦不禁止操作者之后显式配置模型/路由。
- Accio 独立 API schema/输入/状态；显式 client、China/Global、非零 u16 port 与已有 utdid。无预填 client/site/port/device；device 不 trim/生成，新账号必填，重授权留空只表示保留自身设备。callback 密码输入、16384 字节上限、提交后清空；可执行授权链接只允许两个固定 HTTPS login origin/path，没有自动 listener。
- 真实 ConsoleGlobalDialogs/CredentialDialog 接入新增和稳定 credential ID 重授权。仅服务端保存确认 succeeded 才关闭/refresh，不将 OAuth 材料送普通表单二次保存。旧手工录入有明确切换，字段为 Access Token，不冒称原生 API Key 或自动补齐 device/refresh/SG；metadata、模型、enabled、keep 编辑保留。
- 只提取与 Grok 相同的有界请求和管理会话生命周期。产品 schema、状态集合、错误码、面板和 Map 仍独立；Accio 没有 claimed/identityVerified/principal。receipt 按 client/管理身份隔离，无 localStorage 或全局单例；30 秒请求预算、1.5 秒串行轮询、连续五次失败暂停、最多 16 个未确认 receipt。revision 变化禁止新 mutation，已接纳保存继续观察；迟到创建用原 API/Grant 取消，未知保存和迟到 import 不重复提交或复活已消费记录。
- China/Global/旧导入的额度关系如实显示 unknown。新授权缺 SG 清旧 SG、旧账号上下文替换和“不证明同账号”文案沿用真实 P44 合同，不引入第二套后端存储、刷新 scheduler 或凭据字段。

## 2. 红绿证据与实际修补

证据目录：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-provider-expansion-20261104-p45`。

1. `red-ui.log`：3 项行为实际失败：正常 catalog 找不到 Accio 模板，真实全局 Dialog 找不到登录和重新授权按钮；不是缺模块编译错误冒充红测。初版后 `green-initial.log` 为 6 文件、40 passed，typecheck exit 0。
2. `green-accio.log`：Accio API/面板/生命周期/全局入口/授权链接/模板 6 文件、62 passed；后续新增了模式绕过、真实目录及焦点测试。
3. 自查并独立只读复核确认：unknown/importing 时面板虽禁止重投，外层录入方式仍可切回普通手工创建，返回编辑也可绕出。`red-manual-bypass.log` 实际 1 failed。现由 renderer 传稳定活动回调，面板报告 active/busy/pending/unresolved，仅锁模式切换/返回编辑及 handler；整个对话仍能关闭，receipt 不被清空。最终 UI 测试和合成浏览器均证实关闭重开仍锁原未知保存，普通手工入口不被永久禁用。
4. 真实浏览器发现 Escape 后焦点落 BODY。`red-focus.log` 实际 1 failed；复用现有 ProviderCatalogDialog 的 opener owner 模式，关闭时只回到仍连接的 opener，若已进入别的 dialog 不抢焦点。`green-ui-v3.log` 包含焦点红测转绿，浏览器最终构建亦实际回到“打开 Accio 账号弹窗”按钮。后续 typecheck-v3 发现新增夹具误用了浏览器 locator 的 exact 选项（RTL 不支持）；仅改为锚定正则，最终 typecheck-v4 和 green-focus-v4 再验，未改生产语义。
5. 首次隔离构建 `safe-build.log` 失败是 linshi 夹具无法解析 react/jsx-runtime 和错误的 @neuro CSS 文件路径。仅修隔离 builder 的真实 desktop node_modules 解析与包 exports；未升级/安装依赖或改生产构建配置。v2 起通过。初版视觉夹具漏传其他 dialog 的 false 状态导致“添加分组”弹窗出现，仅修夹具；最终 `safe-build-v4.log` exit 0。IAB 未能附着，后改用专用 Edge 标签页；未把失败尝试算视觉通过。

## 3. 最终验证结果与已知失败

| 检查 | 实际结果 |
| --- | --- |
| 扩大前端组合 | `regression-final.log` / `regression-results.json`：33 文件，324 passed / 3 failed |
| 焦点修正后的相关 UI 回归 | `green-ui-v3.log` / `ui-v3-results.json`：相关 CredentialDialog、全局入口、Accio 面板及目录全部通过；与前组按文件+测试名去重，合计 **325 passed / 3 failed，其中本批新增 65 passed** |
| 失败对照 | `baseline-known-failures.log`：两份实际 before-source 经 Vite pre-load 在原 module ID 下替代，不写共享工作树，仍精确复现同样 3 failed / 77 passed |
| 类型 | `typecheck-v4.log`，exit 0；v3 的夹具类型错误保留 |
| checker / ratchet | checker 31 passed，最终 ratchet exit 0；checker 未变，不无故重复 |
| 文本/来源/diff | `source-frozen-v2.json`、`audit-final.json`、`scope.diff`、`diff-check.log`；23 项源码和 5 文档的 UTF-8 无 BOM、摘要、有效行和 whitespace 检查 |
| 隔离 Web 构建 | 真实 `src/main.tsx` 与真实 GlobalDialogs/模板/面板夹具，`safe-build-v4.log` exit 0；显式 cleanDistPath=false，无原 staged publish 插件，不碰 workspace dist/web |

三项扩大回归失败为：`providerCatalogHealthDefaults` 的全模板夹具未提供 Qwen 必填地域/工作空间；`providerCatalogNames` 的 Qwen 同类夹具；以及其 `gemini-canvas` 不符合通用后缀正则。before-source 对照有明确 override 日志和基线摘要，不只是推断为历史问题。本批未修改 Qwen 语义、Gemini ID 或测试去刷绿；这些仍应由对应契约 owner 继续处理，不能宣称全仓绿色。

前端没有仓库配置的 formatter 脚本或配置，本批沿用相邻 TS/TSX 风格，并执行类型、有效行和 diff/文本检查；没有临时安装 formatter 或把 Rustfmt 当 TS 格式验证。23 项源码中 9 项既有/14 项新增：既有最大 CredentialDialog 484→498；GlobalDialogs 313→328，catalog 452→455；共享 hook 168。所有本批文件均不超过 500 有效行，没有 baseline/exception 改写或 oversized debt 增长。

## 4. 浏览器与安全边界

真实入口夹具明确标识“合成材料 · P45 真实入口隔离验证”，使用真实 ProviderCatalogDialog → addCatalogProvider → ConsoleGlobalDialogs → CredentialDialog → AccioOAuthPanel，只有管理 API 模拟，未打开真实授权页面。验证模板固定配置、显式参数、手动 callback、Ready 不自动保存、save_unconfirmed 不报成功/不重投、Escape 和焦点回归、关闭重开原 receipt/模式锁。CSS viewport 实测 1280×900 和 390×844；窄屏 document scrollWidth=390，dialog scrollWidth/clientWidth 均 346，无横向溢出，内部滚动和键盘焦点可达。浏览器截图为工具内联观察，不宣称独立原生或真实管理服务端 E2E。

临时 HTTP server 仅绑定 127.0.0.1 随机端口 57780；结束时核对脚本完整路径后仅停止本批 PID 78228，并检查监听退出。专用浏览器标签关闭、临时 viewport override 还原。没有读取 `.ng` 或历史凭据、调用真实平台、消费付费额度、创建/删除 Redis 容器、出包、替换运行实例、提交或推送。当前树中并行 AccessKey/ProviderStorage/样式变化未接管、清理或回滚。

## 5. 下一批接力

R01/W01 继续补精确业务/probe、旧导入副本归一化与真实客户端/额度边界，不用正常 OAuth UI 代替整个业务链验收。其余原定 Kiro/Vertex/Qwen 未决 OAuth、独立 API/Web/媒体能力和 P50/P60/P70 继续；原始各渠道需求不因本批子链闭合而删减。真实凭据录制仍集中放到功能完成之后。
