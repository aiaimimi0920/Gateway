# Luma MCP 只读账号保存及目录管理增量

## 本批实现

新增独立存储profile `lumalabs_mcp_readonly`、family `lumalabs_mcp`、auth mode `luma_mcp_oauth`，无builtin preset，不增加业务模板数。固定resource为`https://agents.lumalabs.ai/mcp`，要求显式credential池、无provider级秘密、credential级`luma_oauth` context及原`api_key`秘密槽。

本profile只作管理存储，credential必须`enabled=false`，不能开启普通推理、模型测试、scheduled probe、自动补货或refresh。未提供model/refresh/杂交授权、Cookie/header、任意请求路径或call-flow入口。空池、清空秘密及过期context仍可保存管理，过期或缺失材料在目录读取前失败。现有Web/Agents REST不改变身份；仅在配置归属层将已由独立owner验证的MCP profile分开，协议层Agents整host保护未放宽。

新增可选credential-only字段，不修改存储版本或既有数据。正常console route-config保存复用SecretPatch、配置编译、revision/CAS；Keep比较完整context（包括client、redirect、expiry），不能保留原token同时改写这些绑定。DB payload合并不继承account层context，授权代际比较也包含context；本批不开放独立DB账号的完整产品功能。

## 专用管理入口

`POST /v1/internal/gateway/console/luma-mcp-tools`只接受`providerId`、`credentialId`、`expectedRevision`，不接受调用者token、URL或工具调用参数。读取已保存账号，恢复授权材料后执行原有只读inspect_tools。

入口前后均检查管理身份及secret consent、前后核对revision；使用共享有界目录并发槽和独立无Cookie jar/自定义retry的HTTP client。所有响应（含extractor/auth错误）设置no-store。固定initialize/initialized/tools/list，不执行tools/call，不改模型健康、额度、凭据或配置。

通用call-flow、HTTP probe、DB通用modelsPath发现、generic balance发包前都拒绝MCP标识，避免专用token被其它网络入口消费。只读存储profile不会成为普通候选账号。用户显式管理自动化driver的其他元数据操作不属于本批目录合同，未宣称该profile能阻止全Gateway所有人工管理动作。

## 验证记录

证据目录：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-luma-management-20261113`。

- `red.log`：原实现两项行为测试实际失败，证明context会丢失且MCP账号被静态Agents拒绝。
- `green.log`：首次验证因新增集成测试访问私有snapshot.document方法而编译失败；修正为公开GET及compiled provider观察，没有放宽生产可见性。
- 独立只读审查发现模型目录直发入口及Copilot秘密混存边界，已在对应owner补齐。
- `green-retry.log`：定向129 passed、0 failed、2 ignored。lib相关107通过；MCP管理API及正常保存3、MCP配置4、MCP秘密绑定3、原Agents配置5和秘密保护7均通过。两项忽略是明确要求独立浏览器环境的Web/Agents browser-host专项，未执行真实浏览器或上游。
- 正常保存测试实际经过console PUT的SecretPatch/编译/revision CAS及GET脱敏读取，证明账号ID/context/秘密保持、disabled不可路由、重复旧revision拒绝。专用tools/list本批只测管理拒绝路径；成功协议链复用已通过的本地MCP transport测试，不冒称真实账号联合验收。
- 测试编译9m04s，lib执行17.51s，各集成执行0.01–0.34s；不将共享工作区构建时间作为性能基准。
- 格式、checker31项、有效行ratchet、本批源码空白检查通过。新boundary28、config129、管理入口101有效行；修改后最大文件management_config487，无需例外、不改baseline/policy。
- `production-check.log`：非测试 `cargo check --locked --offline --lib --no-default-features --features runtime-defaults -j 4` 通过；含共享构建锁等待11m12s，113项既有warning，未指向本批MCP模块。
- `scope-hashes-final.json`的23项源码SHA256复核一致，文档空白检查通过。没有运行全量回归、真实上游或包装/原生UI验收。

## 尚未完成的产品部分

本批没有实际保存用户账号或重新请求Luma授权，没有真实上游调用/生成/下载/消费额度，也没有修改运行实例、凭据或数据库；不出包、不提交Git。离线测试使用synthetic材料，运行产物和临时存储位于linshi。

专用OAuth授权会话/loopback接收与一键保存、桌面UI、动态client注册、refresh、实际model_list、生成/查询/取消/费用、远端session DELETE和真实账号联合验收仍未完成。当前只有普通管理配置保存链及专用工具目录接口，不将其称为完整Luma业务渠道或可手测桌面功能。
