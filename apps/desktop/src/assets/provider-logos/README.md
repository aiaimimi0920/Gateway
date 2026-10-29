# 服务商品牌图标

此目录保存供应商卡片使用的真实商标资源，不在运行时访问供应商官网或第三方 CDN。
品牌名称与商标归各自权利人所有；展示只用于标识对应服务，不表示合作或背书。

## 来源与分发

- `sources.json` 逐项记录下载日期、来源 URL、原始与最终 SHA-256，以及处理说明。
- Jina、Linkup、You.com、WebSearchAPI、Tavily、OpenRouter 来自对应官网。
- 其余图标来自 MIT 授权的 Lobe Icons；授权原文见 `LICENSE.lobe-icons.txt`。
- `../../../public/provider-logos-NOTICE.txt` 包含完整 MIT 授权及商标说明，通过 Rsbuild
  的 public 目录复制机制随 Web 与桌面前端构建交付，不能只保留源码目录中的许可证。
- ChatGPT、Codex 与 OpenAI 复用 OpenAI 商标；NVIDIA 继续使用现有 `NvidiaBrandIcon`。
- You.com 的标记从官网内联 SVG 提取，保持路径几何不变，将依赖站点 CSS 的填色固化为
  `currentColor` 单色版本。其余资源未变换；不要依据旧印象覆盖官网当前的品牌标记。

## 维护约束

新增商标必须先核实来源，再同步更新 `providerBrandAssets.ts`、`sources.json` 与测试。
不接收用户上传的 SVG，不使用远程图片 URL，不通过 `dangerouslySetInnerHTML` 注入内容。
保留 SVG 的 `viewBox` 和原始比例；图片由 `object-fit: contain` 显示。未知供应商保留字母
兜底，不自动冒用已知商标。

`providerBrandAssets.test.ts` 校验资源完整性、大小、有效 UTF-8、SVG 惰性内容及分发授权；
`ProviderBrandIcon.test.tsx` 覆盖卡片正反面、品牌匹配和未知服务商兜底。
