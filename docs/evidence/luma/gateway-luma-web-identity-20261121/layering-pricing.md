# Luma 分层公开价格核实

- 核实日期：2026-11-21（Asia/Shanghai）。
- 来源：https://docs.agents.lumalabs.ai/guides/images/layering/
- 读取：Exa 对该官方页面的全文读取；本机 urllib 直连返回 403。
- 该页面描述 Agents 分层价格，不能代替 MCP 布局输出 schema，也不能证明本次 JSON-RPC 错误的原因。

以下为价格原文：

+## Pricing

Layering is priced per generation, by `layering.resolution`. The rate is flat — it does not depend on how many layers come back.

| `layering.resolution` | Credits per generation |
| --- | --- |
| `1k` | 75 |
| `2k` | 105 |

A 2-layer result and a 10-layer result cost the same at the same resolution, so you know the charge before you submit. The hold placed at submit equals the final charge — nothing is released on a successful generation. A failed generation releases the hold in full and is never charged.

These are introductory rates and are subject to change. Layering is not yet listed on the Pricing page — confirm current rates with support+luma-agents-api@lumalabs.ai before committing to high-volume workloads.
