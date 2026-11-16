# R14 Luma Web 自动发现 action 准入

日期：2026-11-11。只完成既有 browser operation 的一项提交前修复，不增加模板，不标记整卡 offline_ready。

## 行为与范围

旧实现读取非空 action 菜单后，如果当前 action 不在菜单中、也找不到 operation 对应的 exact/prefix action，仍返回旧默认（或任意首项），随后发送生成 POST。
现在该分支返回固定脱敏错误 `lumalabs_action_unavailable`，stage 为 `action_discovery`、status 为 502；不会提交生成，既有 finally 取消事件 reader、释放锁、abort 和清除 deadline。

保留现有优先级：菜单内当前 action → 对应 exact → 对应 prefix。显式关闭自动发现不读取菜单；空菜单、菜单 HTTP/传输失败、非法 JSON 仍保持原 best-effort 默认行为，本批不改变这些兼容契约。
不新增 model→action 映射，不证明同类 prefix 对应用户指定模型，也不声称 current action 的供应商语义已验证。精确账号图片 probe/UI 仍未接通，不恢复模型健康。

## 验证

- 红测：图片、视频、音频三种 operation 对非空无匹配菜单，预期 POST 为 0，旧实现实际均为 1；8 passed / 3 failed。
- 修改后 `node --test scripts/tests/lumalabs-*.test.mjs`：37 passed / 0 failed。新增 7 项包含三种拒绝、显式 action 绕过发现、空菜单、当前 action 优先、同类 prefix 兼容；拒绝场景验证取消 stream 和 abort。
- `npm run test:effective-lines --prefix scripts`：31 passed；`npm run check:effective-lines --prefix scripts`：通过。保留并恢复运行前共享报告 bytes。
- 两个修改的 MJS 均 `node --check` 通过；仓库没有为 scripts 配置独立 JS formatter，沿用现有格式，未安装或引入工具。`git diff --check` 通过。
- 权威 lexer 有效行：browser-operation 361→365，测试 128→158；未引入新依赖、重试、进程或数据存储。

证据：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-luma-action-admission-20261111`，含直接修改前副本、red/green/checker/ratchet 日志和检查报告。

## 未验证边界

未联网核实当前供应商 wire，未读取真实凭据、调用付费上游、构建 Rust/EXE、操作在线实例、提交或推送。测试使用序列化 browser function 的合成执行环境及既有隔离 worker/Fetch 测试，不等于真实 Luma 平台验收。
此前 Udio 极简 feature 构建的范围外 Gemini 编译失败未重跑、未修复；总计划 P50/P60/P70 仍未完成，新增模板仍为 17。
