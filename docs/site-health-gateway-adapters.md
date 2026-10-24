# 站点健康：网关适配器与内部读取器

## 对外按系统分类，对内按能力读取

站点健康不按域名建立适配器。同一网关项目部署出的不同站点，复用同一个系统级适配器；
站点地址、独立监控认证、远端分组和本地凭证组绑定属于方案配置。官方独立服务继续使用
`openai-status`、`anthropic-status` 等既有独立适配器，不混入网关能力探测。

| 对外选择 | 配置 ID | 内部能力范围 |
| --- | --- | --- |
| New API | `new-api` | 性能摘要及有限的兼容监控扩展 |
| sub2api | `sub2api` | channel monitor v2 分组矩阵 |
| One API | `one-api` | 有限的可选监控扩展；不承诺原版提供模型健康接口 |
| 兼容网关（类型未确认） | `compatible-gateway` | 全部六种已支持的固定健康读取方式 |
| 自动（仅公开接口） | `auto` | 匿名探测已知能力，不凭兼容接口断言网关项目身份 |

New API、One API 的兼容扩展候选为 `new-api-perf-summary`、`model-status-all`、
`channel-health`、`model-status-embed`、`model-health-slot`。sub2api 使用 `sub2api-v2`。
这些名称是内部读取器，不是每站点独立适配器，也不是对上游项目的接口保证。
接口路径和读取器枚举统一由 `src/site_health/discovery/readers.rs` 所有。

系统选择表示用户声明的兼容目标，探测结果证明的是健康数据格式和范围，不是软件供应链身份。
没有模型健康数据时保持未知；`/api/status`、首页 HTTP 200、模型目录或 `/health` 存活结果
都不能替代模型成功率。One API 的边界依据见
[接口调查](plan/2026-10-23-site-health-adapter-evidence.md)。

## 识别、认证与预算

1. 无认证方案只探测固定候选接口；每批最多并发两个读取器。
2. 按有效模型证据数量选择一个读取器，同分采用固定候选顺序，不混合不同统计口径。
3. 有认证的系统级方案先使用独立空认证对象做公开能力识别，不带 Cookie、业务密钥或监控密钥。
   此阶段 New API 不读分组详情，slot 最多识别第一个远端组；这些结果不用于发布方案健康率。
4. 公开识别后最多选择一个读取器执行带认证的完整检测，重新核对全部所选远端分组。
   公开能力仅记录为 `capability_detected`，不是最终分组健康结果。
5. 未识别出能力时，New API 只允许认证到标准性能摘要读取器，sub2api 只允许认证到标准矩阵读取器。
   One API／未确认类型不猜测私有扩展的认证目标，报告 `authentication_target_unconfirmed`，不发送密钥。
6. 认证失败、重定向、无效响应、组不匹配等都不会触发向另一读取器发送密钥。
   自动模式仍禁止配置认证。

各阶段共享原有 25 秒总超时、DNS 固定、无代理、禁止重定向和响应体上限；不放宽 SSRF 策略。
New API 完整检测最多 16 次分组详情请求，未覆盖模型保持未知。例如带一个远端组的 New API
方案，三次浅层公开请求加认证摘要和 16 次详情，共 20 次请求；channel/embed 因不支持分组
在本地拒绝，不产生额外 HTTP 请求。请求数量受读取器、组数和固定详情预算限制，不抓取脚本
或遍历任意 API。其他固定预算见原站点健康文档。

## 旧配置、密钥与回退

六个旧 ID 继续接受并原样序列化：`new-api-perf-summary`、`model-status-all`、
`channel-health`、`model-status-embed`、`model-health-slot`、`sub2api-v2`。
它们是兼容入口，仍固定执行一个读取器，不能因显示名称更新就扩大已有密钥的请求范围。

React 主选择框只显示系统及官方服务。旧摘要／矩阵方案分别显示 New API／sub2api；
不能证明网关身份的旧扩展显示“兼容网关（类型未确认）”，同时提示已固定的旧读取器。
仅打开、修改名称或保存旧方案不会改写旧 ID。用户可明确点击“改为按网关自动识别”，
或重新选择系统；这才会保存新的系统 ID。

适配器是监控密钥目标身份的一部分。旧读取器改为系统级协商、或切换系统时，已有密钥的
`Keep` 会被后端拒绝，必须通过敏感信息授权显式 `Replace` 或 `Clear`；
不能为了兼容显示而把两个不同请求范围规范化为同一个 secret audience。
profile ID、密钥路径、revision/CAS、在途取消与 binding 栅栏保持原协议。

这是新增配置值，不升级存储版本、不迁移数据库、不重写旧 revision。旧 EXE 不认识新的
系统 ID，因此只应由更新后的后端写入／运行；回退旧 EXE 前应通过管理版本历史恢复兼容
revision，不应手改运行中的 journal 或密钥文件。

## 验证入口

- `tests/site_health_gateway_contract.rs`：系统复用、旧值回读、公开识别隔离、唯一认证读取器、
  无认证回退、分组与请求预算、存活不冒充健康。
- `tests/site_health_secrets_contract.rs`：旧读取器到系统级适配器的显式密钥授权。
- `apps/desktop/src/features/console/gatewayHealthAdapters.test.tsx`：系统级选项、旧方案不改写、
  显式切换、分组校验和身份声明边界。
- 既有官方状态、监控策略、凭证组、运行时栅栏及 React 工作区测试继续保护原行为。

公网可达性、Fake-IP 兼容和新增认证头能力属于另外的任务；本次分层不宣称已解决这些问题。
