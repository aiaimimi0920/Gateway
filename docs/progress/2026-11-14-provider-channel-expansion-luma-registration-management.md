# Luma MCP 注册确认、管理回执与授权入口

本批开始于 2026-11-13，跨日完成验证。承接注册协议底座，将显式动态注册接到正常管理和桌面界面；无真实注册 POST、登录或业务调用。

## 实现范围

- `POST /console/luma-client-registrations` 仅准备本地操作：正常管理身份与敏感访问准入、固定 MCP provider／revision 校验、预留数值 loopback 临时端口、返回不含秘密的 receipt。此时不访问 Luma。
- `POST /console/luma-client-registrations/:id/start` 要求 `confirmRegistration=true`、同 owner、原 revision 和当前有效目标。原子状态转换后，由现有 Registry 的 JoinSet 持有单次注册，不随 HTTP 响应断开而释放任务；重复 start 不重发远端请求。
- `GET /console/luma-client-registrations/:id` 只返回同 owner 的状态与公开 client/redirect；所有响应 no-store。远端错误、取消或超时统一保守记录 `unconfirmed`，不据此再次注册。
- 最多 16 个注册回执、复用现有 32 个任务上限与 shutdown/join。已接受的回执在当前 runtime 生命周期内保留；准备状态最多 20 分钟，清理本地监听。成功或未知结果先释放端口，再暴露终态；后续登录仍须成功绑定同一端口，不偷偷改 redirect。
- 尚未提交的准备若过期或遇配置更新，可显式重新准备并作废旧本地回执；已经 start 的尝试绝不走这条重建路径。此恢复缺口经交叉审查发现，前端 `reprepare-red.log` 先复现失败后修复，后端补过期重新准备与旧 ID 不可启动测试。
- 新账号界面按“准备 → 确认注册 → 检查结果 → 使用此客户端授权”操作。注册成功不自动登录或保存，也不请求工具目录、模型、余额或生成。既有账号重授权继续复用原登记信息，不展示新注册操作。
- 前端在 start 前保留提交标记，挂在既有按连接／管理身份隔离的 OAuth operations Map 生命周期下；关窗重开或响应不明只留 GET 查询。确认权限到达不自动提交，任意服务器错误正文不显示。

## 验证与证据

目录：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-luma-registration-management-20261113`。

- 前端 `ui-tests-final-v2.log`：5 个文件 **35 项通过**，其中本批注册界面 7 项；覆盖独立确认、回执丢失后只查询、关闭重开、版本变更、敏感访问、绑定失配、过期恢复，以及既有 Luma 授权／工具目录／空池入口。
- `typecheck-final-v2.log` 前端类型检查通过；`build-final.log` 桌面、BrowserConsole 真实入口及模拟页面构建通过，输出隔离在上述目录，不覆盖现有 dist，不作为 EXE 包交付。
- 第一轮 Rust `rust-tests.log`：124 passed、0 failed、2 ignored，未包含随后补入的过期恢复测试及最后的监听终态次序修正；不冒充最终源码验证。最终 `rust-tests-final.log` 为 **125 passed、0 failed、2 ignored**，包含本批 4 项真实管理／loopback 注册测试及现有 Luma 授权／协议／媒体等相关测试；两项 ignored 为既有独立浏览器专项。第二轮构建报告 9m58s（含共享目录等待），测试运行 20.37s。
- checker 31 项、有效行 ratchet、官方 scoped rustfmt 通过。最大新增测试文件 261 有效行，单一注册管理测试责任；其余本批文件均不超过 208 有效行。`scope-hashes.json` 记录 15 项源码，不调整 baseline、policy 或例外。
- `production-check.log`：非测试 `cargo check --locked --offline --lib --no-default-features --features runtime-defaults -j 4` 最终 exit 0；存在继承 warning，不称零警告。测试及编译结束后 15 项源码哈希一致，范围 UTF-8 无 BOM、空白与文档链接检查通过。
- 本轮独立只读审查未发现重复远端 start 或锁循环；指出的从未提交准备的恢复阻断已修。新增状态机、HTTP、UI与测试同批加入，除明确记录的恢复红测外，不声称全部遵循了测试先失败顺序。

## 浏览器观察

Edge 使用生产 `LumaOAuthPanel`、真实主题及标明“本地模拟”的 API，观察准备前无提交、准备后的风险确认、确认后只剩查询、Escape 关闭再打开仍不出现重复注册按钮、查询成功后显式填写 client／redirect、登录按钮可用。未点击真实登录链接。

本次没有宣称完整 Gateway 管理 HTTP 与浏览器端联合、真实供应商或窄屏验收；后端真实管理 Router／loopback peer 与前端模拟链分别记录。临时 HTTP 服务和测试标签已关闭。

## 剩余边界

动态注册的本地管理／UI入口已经接线，但真实供应商动态注册及正常账号联合仍未验收。回执尚未跨 runtime 重启持久化；丢失已提交回执时不可自动新建注册，不把当前进程内重开恢复等同崩溃恢复。注册成功的公开 client/redirect 最终随正常账号授权保存；只注册、尚未保存账号阶段的跨进程恢复仍需补齐。

另外保留：refresh、实际模型发现、生成／取消／结果／费用业务、远端 MCP session DELETE、跨机器授权、窄屏和原生包。未生成、下载、消费积分、操作现有凭据／数据库／运行实例，未提交或推送；不据本批减少整产品未完成数量。
