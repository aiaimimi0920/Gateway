# Luma 视频参数界面与分层收尾

日期：2026-11-19（Asia/Shanghai）。按用户选择先续做最近 Luma，再推进 R01–R16；本批不切换到 Magpie 67 项或其他三服务商优先线。

## 改动

修复 [图片分层](2026-11-18-provider-channel-expansion-luma-layering.md) 两项真实失败：字体 weight 保留 JSON 整数／小数表示，既有范围校验保留。

正常 Luma Agents 账号媒体测试增加后端已支持的参数：视频扩画归一化位置；视频重绘 HDR、五类 conditioning、单张 guide 和最多 64 个成对关键帧／源帧序号。默认／启用／停用及可选 tuning 分开处理；隐藏参数不提交，自定义 controls 不同时发送 auto_controls。切换操作清空临时参数，不保存到账号。无效组合与超过 256 KiB 的请求在收费确认前拒绝。

费用确认绑定完整请求、provider 配置及精确 credential；输入、账号或未保存草稿状态变化后取消旧确认，确认执行时再次比较请求。旧实现更改 prompt 后仍留确认的红测已复现，修复后转绿。

官方 [视频重绘](https://docs.agents.lumalabs.ai/guides/videos/editing/)、[视频扩画](https://docs.agents.lumalabs.ai/guides/videos/reframing/)、[分层](https://docs.agents.lumalabs.ai/guides/images/layering/)、[Files](https://docs.agents.lumalabs.ai/guides/files/) 和 [LLM inference](https://docs.agents.lumalabs.ai/guides/llm-inference/) 均匿名 GET 200。正文、UTC 时间、状态及 SHA-256 见 `../evidence/luma/gateway-provider-resume-20261119/sources.json`。LLM 为独立域名、Key 体系与 token 后付费产品，另行接入，不使用媒体 Key 或 credits 池。Files 本批仅核实，尚未实现上传／管理。

## 已执行验证

- Rust layering 定向 16 passed；完整 `luma_agents` 95 passed / 0 failed / 1 ignored，包含 16 项。覆盖公共媒体／管理 probe、成功扣次及失败退款、失败 credits、取消／deadline、不重提、视频扩画／重绘／分层。ignored 浏览器宿主专项未执行。
- 前端 `lumaVideoProbeDraft.test.ts`、`LumaMediaProbeDialog.video-controls.test.tsx`、`LumaAgentsMediaProbe.test.tsx` 共 23 passed，TypeScript 通过。
- 生产 lib `cargo check --locked --offline --lib --no-default-features --features runtime-defaults -j 4` 通过，保留 115 项既有 warnings；Rustfmt 通过。
- checker 31 passed，effective-line ratchet 通过；该阶段 5811 measured files、14 immutable runtime artifacts，governed source 超过 700 行为 0。
- 实际 main 入口隔离生产 Web 构建通过，输出仅在临时目录；移除发布插件、关闭清理以避免清空既有 `dist`。
- 独立 Edge／合成回调检查 1280×900 和 420×850：窄屏 document 宽 420、dialog 客户／滚动宽均 386，表单内部高 590／滚动高 1639，运行按钮可达。确认一次后合成回调收到 720p HDR、pose precise、配对关键帧；无真实上游请求，控制台 0 errors / 0 warnings。截图 `luma-wide.png`、`luma-narrow.png` 已保存于上述仓库归档目录；原 `.playwright-cli` 调试输出仍是临时记录，不作为接手依赖。浏览器和本批预览服务已关闭。

## 保留边界

这不是正常 main→真实管理 TCP→供应商的完整联合验收；合成预览只渲染实际组件、主题与 CSS。没有真实生成、资产／画质／视频播放、供应商收费或账号权益验收，没有 EXE 或正式发布包。本轮未修改运行实例、账号、数据库、Git 提交或远端。

Files、MCP 创作业务、取消中尚未上交 receipt、截止后／跨进程追账、现金结算及适用 Server／原生包仍按总计划保留；R14、P50/P60/P70 与 67 项总数不因此关闭。
