# MiniMax M Plan 显式订阅 Key 合同

核实日期：2026-11-10。当前官方 Coding Plan 页面已重定向到 M Plan；本实现接入官方订阅 Key，独立于社区 MiniMax Code Agent OAuth。后者继续保留在完整接入看板中，不能用本切片替代。

## 官方证据

| 合同 | 官方来源 | 本次确认的含义 |
| --- | --- | --- |
| 国际订阅 Key | <https://platform.minimax.io/docs/m-plan/quickstart.md> | 套餐详情取得 Key，与按量 Key 不通用 |
| 国内订阅 Key | <https://platform.minimax.cn/docs/m-plan/quickstart.md> | 当前国内 base 为 `https://api.minimax.cn/anthropic` |
| Messages | <https://platform.minimax.io/docs/api-reference/text-chat-anthropic.md> | POST `/anthropic/v1/messages`，推荐单一 Bearer；也支持 x-api-key |
| SDK 参数 | <https://platform.minimax.io/docs/api-reference/text-anthropic-api.md> | 工具选择只支持 auto；多轮须回传完整 thinking/text/tool blocks |
| 平台模型列表 | <https://platform.minimax.io/docs/api-reference/models/anthropic/list-models.md> | 公开 GET `/anthropic/v1/models`，未承诺按订阅账号权益过滤 |
| M Plan | <https://platform.minimax.io/docs/m-plan/intro.md> | 当前套餐明确包含 MiniMax-M3.1-Flash，不据此赋予每个账号所有平台模型 |
| 旧套餐 | <https://platform.minimax.io/docs/m-plan/token-plan-notice.md> | 老 Token Plan 保留已订阅权益，不自动迁移或改成现行套餐 |
| 额度 | <https://platform.minimax.io/docs/m-plan/faq.md> | 官方给出 remains 请求，未公开成功 JSON 字段合同 |
| 用量 | <https://platform.minimax.io/docs/m-plan/usage-rules.md> | 五小时和七天窗口独立恢复，不能直接折算为固定 token 数 |

匿名抓取快照保留在 `C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-minimax-20261110/contracts/`，带来源和抓取时间。本次匿名请求两个官方 remains 地址均得到 HTTP 200 + `base_resp.status_code=1004`，属于未认证失败；没有进行推理或订阅余额成功查询。

当前官方 Markdown 未发布可依赖的 Key 前缀、长度或正则合同，所以只作安全传输字符与长度校验，不通过 `sk-cp` 前缀假定订阅身份。Key 是否属于 M Plan、具体可用模型及费用由官方服务决定。共享按量目标并不意味着两种 Key 权益互通。

## 配置与执行

| 模板 | 固定 base | 发送地址 |
| --- | --- | --- |
| `minimax-m-plan` | `https://api.minimax.io/anthropic` | `https://api.minimax.io/anthropic/v1/messages` |
| `minimax-m-plan-cn` | `https://api.minimax.cn/anthropic` | `https://api.minimax.cn/anthropic/v1/messages` |

两个模板共享 `minimax_m_plan` profile、`minimax_m_plan_key` auth mode 和 `line-minimax-m-plan` feature。固定 preset、provider 目标与 credential override 必须同区域；`keep` 绑定稳定账号与完整目标，切站必须显式提供对应 Key。旧 `api.minimaxi.com` 的认证兼容性未证实，因此不自动添加别名或重定向。

控制台先创建空账号池，再通过现有 secret grant、replace patch、CAS 保存显式订阅 Key。新建、替换和备注修改保留 credential ID，不把 Key 写入普通文档。自动测试关闭，不提供猜测的默认模型、价格、余额或可用性标记。当前用手工模型选择，平台 Models API 与订阅权益发现分开；未接入自动目录和 remains 查询。

公共 `/v1/chat/completions`、`/v1/messages`、`/v1/responses` 的文本及函数工具请求复用 Anthropic 编码；上游始终是 Messages。Messages 的原生 thinking/signature、工具块和 output_config 通过现有 native body 保留机制传递；Chat/Responses 当前只承诺文本与函数工具桥接，未实现完整 reasoning 往返，不能把原生 Messages 的测试外推给它们。

Messages 请求只接受 auto 工具选择，其他策略明确失败，不静默降级。托管工具、远端 Responses 状态、后台/存储任务及未实现入口不外发。媒体不属于本切片验收范围。

最终 sender 重验 URL、方法、无 query、Key、协议和模型，清除 caller/SDK 身份，只发送当前账号 Bearer；禁用共享 Client 默认头和重定向，排除 Native Claude shortcut。模型不会触发 XML 工具注入。非流请求使用有界 JSON 读取，读取、解析及响应合同失败统一停止重放；流在协议转换前用现有有界 SSE decoder 验证 message_start、message_delta 和 message_stop，结束帧延迟到合法 EOF 才释放。截断、错误帧、非法尾部均交给现有唯一结算 owner 失败退款；客户端取消仍由共享 Drop 路径释放资源和退还预扣。

## 验收边界

运行验证与回执见 [本轮进展](../progress/2026-11-10-minimax-m-plan-runtime.md)。本地测试只使用合成 Key、非转发 loopback 和隔离临时数据库；测试版本的最终 transport seam 没有 loopback scope 时直接拒绝，不向真实推理地址回退。

没有读取真实凭据、验证订阅消费、运行原生 EXE、部署、推送或重启。MiniMax Code 的 `mcode-public` device OAuth、Agent `mavis` 路由、免费 credits 及成功余额合同仍未实现。固定社区参考 `magpie-community/plugins@93dacd738f2c35981cf0364ad1bfbfe7b427a78a/packages/minimax` 声明其未经 MiniMax 批准；不能把其内部接口标为官方支持。
