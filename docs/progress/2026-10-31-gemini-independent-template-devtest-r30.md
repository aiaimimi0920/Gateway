# Gemini 独立模板桌面手测包 r30

## 用户决定与实际改动

用户确认保持现有 `gemini-web`、`gemini-canvas` 两个独立服务商模板，不强行统一。
本轮已精确撤回刚增加的统一选择器、preset 选择接线及其试验测试；没有撤销之前完成的
Gemini 功能、既有工作树修改、凭据或配置。两个试验新文件移入 linshi 留存，不在源码目录。

`providerCatalog.ts` 与 `ProviderCatalogRequestFields.tsx` 已确认对 Git HEAD 无差异，
源码中无统一入口 helper／字段的残留引用。现有独立模板仍为：

| 模板 | preset | 专用执行器／登录流程 |
| --- | --- | --- |
| `gemini-web` | `gemini-web-chat` | `gemini_web`／`gemini-web` |
| `gemini-canvas` | `gemini-canvas-browser-relay` | `gemini_canvas`／`gemini-canvas` |

没有把 Canvas 音乐／视频或 Web 朗读的已知失败改成通过，边界仍见 r29 收尾记录。

## 新手测 EXE

完整包目录：
`C:/Users/Public/nas_home/AI/GameEditor/Neuro/release/Gateway/gateway-devtest-20261031-gemini-web-canvas-r30/`。

桌面入口为目录内 `gateway-ui.exe`，后端为 `gateway.exe`；须保留整个目录及支持文件，
不能只复制桌面 EXE。包类型为 **devtest / desktop / windows-x64**，不是正式 Release。

使用官方 `tools/package-gateway-devtest.ps1 -Edition desktop -Jobs 2` 流程，执行 locked
Cargo 构建、Web／Tauri 静态资源构建、打包及两处完整性 smoke。没有升级依赖、手工换入旧 EXE
或覆盖既有版本。构建前观察到其他项目 Rust 检查，可用内存曾为 5.48–7.30 GiB，故从默认
4 个任务降低为 2 个；没有停止其他项目任务。

- 构建／打包开始：2026-10-31 16:05:19（Asia/Shanghai）。
- 完成交付：2026-10-31 16:16:40；整条命令 680.573 秒，约 11 分 21 秒。
- 后端 Cargo：7 分 59 秒；桌面 Cargo：25.13 秒。使用现有增量缓存，不据此推断冷构建速度。
- 构建冻结源码指纹：`bcdcda6a93a63dd1f4469ad62eae26dd7b8724b4c0dbd112a65faf4737463d21`。
- `gateway.exe`：82,746,880 bytes，SHA256
  `45681129836280e911e54484278a4e3664cd4893f65adefc1a2011f4719cbcc1`。
- `gateway-ui.exe`：12,862,464 bytes，SHA256
  `86bf331d5727fd159599c6faf573990bc29885bc311c9f8c13787b6bec2d945d`。

实际冻结的是当前完整工作树，包括此前未提交的功能修改，而非仅 Git HEAD。
本文为出包后追加的记录，不声称当前整个工作树指纹仍等于上述冻结快照。

## 验证与未执行项

- 保留独立模板后的相关前端测试：6 个文件、54 项通过。
- TypeScript typecheck 通过。
- 有效行数 checker：31/31；ratchet 通过，governed source >700 为 0。
- `git diff --check` 通过；两个撤回的接线文件恢复为无 diff。
- Web／Tauri 构建、后端／桌面 devtest 构建通过；存在既有 Rust unused／dead-code／linker 告警。
- locked Gemini worker、ChatGPT browser runtime 导入通过。
- staging 和最终交付目录的 manifest／checksum 完整性 smoke 均通过；EXE 哈希独立复核一致。

撤回之前的试验日志保留在证据目录中；它们不是本次独立模板交付的验证结果。
当前最终 typecheck 与 54 项定向测试为出包依据。没有生产源码新增，未执行无关全量回归、
付费上游调用、Canvas 重试或正式发布审计。未启动本包原生 UI，留给用户手动验收。

未替换或重启现有 Gateway，未复制在线账号／数据库，包中无便携 `.ng`。
手测默认端口为 4200；数据目录默认使用用户主目录下 `.ng-devtest-4300`，名称沿用历史值。
若 4200 被旧实例占用，新桌面程序会明确报错，不自动接管或换端口。

证据目录：
`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-gemini-template-devtest-20261031-r30/`。
其中 `package.log`、`package-result.json`、`delivery-verification.json` 为打包与交付证据；
`typecheck-independent.log`、`independent-template-tests.log` 和 `effective-*-independent.log`
记录最终相关检查。未提交或推送。
