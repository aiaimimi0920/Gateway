# 服务商品牌图标（2026-10-26）

## 完成范围与运行边界

服务商卡片正反面改为使用真实商标，不再为已接入品牌只显示字母缩写。新增 23 份本地
SVG/PNG 资源，NVIDIA 保留原有 SVG；ChatGPT、Codex 与 OpenAI 共用 OpenAI 商标。
未知服务商继续保留字母兜底，不自动冒用品牌。

用户点名的 Jina、Exa、Linkup、You.com、WebSearchAPI、Tavily、OpenRouter、ChatGPT
均已覆盖，同时补齐 Anthropic、Azure、Cohere、DeepSeek、Google、Groq、LongCat、
Mistral、Perplexity、Poe、Qwen、Suno、Together、Udio、xAI 的既有品牌入口。

本轮未重启或更新 4200，未修改在线凭据、数据库、搜索路由或自动探测策略，未生成 EXE
或分发包，未提交或推送。保留工作区原有其他改动。此次生产前端构建与组件预览不代表
在线业务、原生桌面或用户人工验收。

## 实现与资源来源

- `ProviderBrandIcon.tsx` 统一卡片正反面的展示；本地固定 Map 选择图片，未知品牌兜底。
- `providerCardVisual.ts` 补齐 ChatGPT、Codex、WebSearchAPI、You.com，并优先匹配明确
  ID/显示名及其后缀，再匹配通用 preset，修复 OpenRouter 被误画成 OpenAI 的问题。
- 使用固定 28px 图片和 `object-fit: contain`，不拉伸商标；中性浅色底保证黑色标记在
  深色界面可辨认。保留原卡片交互、尺寸容器与无障碍语义。
- Jina、Linkup、You.com、WebSearchAPI、Tavily、OpenRouter 来自当日联网核实的官网；
  其余 17 份来自 MIT 授权的 Lobe Icons。来源、下载日期与原始/最终 SHA-256 记录在
  `apps/desktop/src/assets/provider-logos/sources.json`。
- You.com 官网 favicon 实际返回 HTML，没有当图片入库；改用官网内联 SVG 的路径，
  固化 CSS 依赖填色并保留几何，变换记录在来源清单。没有绕过官网验证或认证。
- `public/provider-logos-NOTICE.txt` 保留完整 MIT 授权及商标说明。已确认构建复制出的
  文件与源码 SHA-256 相同：`b5700ac18ac319f5798778c1ffcc0e71349f45c00d8b31ef041ad08ca006807a`。

## 安全与资源审查

运行时只加载随包图片或构建内联的 data URL，不请求供应商官网或外部 CDN，不新增
依赖、用户上传入口、HTML/SVG 注入、凭据传输、后台任务、数据库写入或迁移。
资源测试限制单文件小于 128 KiB，校验 SHA-256、PNG 魔数、SVG 解析与 viewBox，
拒绝脚本、事件属性、外部引用和非白名单 SVG 元素；文本使用 UTF-8 无 BOM。

## 实际验证

| 检查 | 结果 |
| --- | --- |
| 初始商标组件回归红测 | 12 项中 10 项失败，捕获缺图片与错误品牌 |
| 分发许可证红测 | 25 项中 1 项失败，捕获缺失 public notice |
| TypeScript typecheck | 通过 |
| 图标、资源、品牌识别、卡片正反面、NVIDIA 定向回归 | 5 文件，86/86 通过 |
| Rsbuild 隔离生产构建（实际 main.tsx 与图标预览入口） | 通过，输出只在 linshi |
| 构建后 public notice 完整性 | SHA-256 相同 |
| Edge 实际组件预览 | 23/23 图片可解码，全部为 data URL 或本地 URL |
| 浏览器布局 | 实际 CSS 视口宽 1144 与 260，均无横向溢出；临时视口已恢复 |
| 有效行 checker 测试 | 31/31 通过 |
| 有效行 ratchet、git diff --check | 通过 |

预览使用真实组件与产品 CSS，但没有连接 Gateway、展示真实账号或调用上游。浏览器
扩展自身有 warning，预览入口缺省 `/favicon.ico` 返回 404；品牌图片全部成功加载。
没有把预览页无 favicon 的情况误归因为生产服务错误，正式配置使用原有 favicon.svg。

有效行：组件 14、样式 10、资源映射 28、资源测试 49、组件测试 51、声明 8、
`providerCardVisual.ts` 167、`ProviderLedgerCard.tsx` 293，均低于 500；没有新增
超限文件或扩大历史大文件。前端未配置独立 formatter 命令，改动保持邻近文件风格。

临时证据与隔离构建保留在 `linshi/gateway-provider-icons-20261026`。本轮创建的浏览器
标签页与随机回环端口静态预览服务已关闭，没有操作用户原有页面或其他进程。
