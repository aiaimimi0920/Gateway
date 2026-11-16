# Command Code Provider API Chat 本地接入

状态：`partial_pass`。Command Code Provider API Chat 的本地管理／目录／UI／推理切片已完成验证；最终 Rust 联合 254 passed / 0 failed / 0 ignored，其中 Command Code 23 项。持续范围仍为 Magpie 原始 42 项＋红框 23 张卡＋聚合子渠道，本切片不代表全目标完成。

## 改了哪些内容

- 新增独立 preset/profile/feature、固定 Provider API Key 身份、空模型池模板和匿名官方目录。仅导入明确声明 Chat 的模型，不猜协议、价格或套餐权益。
- 贯通控制台添加账号、目录读取、秘密替换/Keep、稳定 ID、grant 与 CAS。自动测试默认关闭，目录读取不自动保存。
- 专属 Chat sender 接入三个公共文本入口，支持 JSON/SSE 和函数工具双回合；固定目标、禁跳转、脱敏错误、失败不重放、取消释放与本地预扣恢复。
- 复用既有单消息打包函数，保留全部 system 顺序及片段；不按模型名称猜参数。通用 normalizer 已证丢语义的 reasoning、hosted tools、富文本及历史输入明确拒绝。
- 通用识别、后台 discovery、目录、header builder 与账号探测增加产品边界；保留其专属控制台测试为后续范围。

## 验证与来源

证据目录：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-commandcode-20261110`。`before/` 保留任务开始前的 dirty 文件，`scope-files.json` 及 `scoped.diff` 记录本轮范围。HEAD 起点 `7969f12c85efceacf2a5e384e143b5c39a550981`；大量 inherited edits 保留，`wholeWorkingTreeProvenance=false`。

| 检查 | 实际结果 | 回执 |
| --- | --- | --- |
| 官方合同 | 六份原文/目录/固定 SHA 源码已联网保存，匿名目录 97 条中 85 条声明 Chat | `contracts/sources.json` |
| Rust 首轮 | 编译失败：本轮目录测试服务器闭包缺少 `async move`；已修复，日志保留 | `cargo-final.json/.log` |
| Rust 第二轮 | 252 passed / 2 failed；双候选账号断言写死、纯文本使用必有工具的解析助手，均已修正 | `cargo-final-r2.json/.log` |
| Rust 最终轮 | 254 passed / 0 failed / 0 ignored，其中 Command Code 23 项 | `cargo-final-r3.json/.log` |
| Command Code UI | 12 passed / 3 文件；名称及 ID 唯一性筛选 2 passed / 56 skipped | `ui-commandcode.json/.log`、`ui-names-commandcode.json/.log` |
| 相邻 UI | SenseNova 3 文件 12 passed | `ui-adjacent.json/.log` |
| TypeScript | 通过 | `typecheck.json/.log` |
| Python | inventory 11、reference 14、manifest validator 5，共 30 项通过 | `python-*.json/.log` |
| 清单 | 54 条 manifest；inventory/reference 校验无错误，全部保持 metadata-only | `manifests.json`、`inventory-validate.json`、`reference-validate.json` |
| 有效行与格式 | checker 31 passed；最终 ratchet、定向 rustfmt 检查通过 | `effective-tests.json`、`line-ratchet-final.json`、`rustfmt-check-final.json` |
| 最终增量与来源 | scoped diff、UTF-8 无 BOM、Rust 构建输入 hash 对账见最终汇总 | `scoped-review-final.json`、`source-r3.json`、`source-final.json`、`final-validation.json` |

最终联合轮覆盖公共三入口文本 JSON/SSE、函数工具双回合、审计/本地计费、失败不重放、取消释放、坏 JSON/SSE/错误尾部、固定目标/身份、管理 grant/Keep/CAS、匿名目录和管理旁路。额外检查 system 顺序/片段、多模态与 hosted 控制拒绝、Responses 空 annotations 正常回传，以及原生响应中的 HTML entity 工具参数和 XML 示例文本不被通用兼容修复改写。

相邻覆盖共享 OpenAI 解析/打包/流、call flow、preset、implementation lines、全部选中的目录合同和普通 API header。真实请求测试均使用非转发 loopback、合成 Key 和独立 LocalRuntime；退款断言只表示恢复本地预扣。

前批名称合同的 5 项已知失败（Kiro ARN、Cloudflare ID、DIM 两站、Nous label）本轮没有修改其来源，也未重跑全量名称合同；只验证新增 Command Code 与 ID 唯一性。不能把这次筛选通过当作全量 UI 全绿。

## 未验边界

真实账号/套餐权限/消费、真实 SSE、原生 Responses/Messages、System One/Jev、浏览器签发 Key、Go 私有接口、完整 reasoning/media、ZDR、专属控制台测试、浏览器视觉、EXE、Server/Redis、独立 feature 裁剪和部署均未验或不在本切片范围。此前独立裁剪的 S06 Gemini 门控问题保留原 owner。

未提交、推送、出包、重启或读取真实 `.ng`/credentials。合同细节和公开来源见 [Command Code 合同](../providers/commandcode-chat-contract-2026-11-10.md)。
