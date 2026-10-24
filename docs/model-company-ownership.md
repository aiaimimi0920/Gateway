# 模型开发者分组规则

## 2026-10-23 范围与命名

盘点当前配置的 `supported_models`、发现目录、已验证目录及 `default_model`，
按完整 ID 去重共 441 项。没有把远端公开目录直接加入当前账号能力。
本次为展示归类，不证明转发站点实际使用的权重、版本真实性或可调用性。
旧静态目录排序审阅日期不随本次家族规则补充而更新。

- 单家族通常使用模型品牌：DeepSeek、Qwen、Claude、Grok、Kimi、GLM、Granite、
  Step、LongCat、SenseNova、Laguna、Jev、Hunyuan、Yi、BGE、StarCoder、Solar、Palmyra 等。
- 多家族使用开发者名称：Google（Gemini/Gemma/Imagen/Veo）、OpenAI（GPT/DALL-E/Sora）、
  Meta（Llama/Muse）、Mistral AI（Mistral/Codestral/Voxtral/Leanstral 等）、NVIDIA、
  Microsoft（Phi/Kosmos）、ByteDance（Doubao/Seed/Seedream）、InclusionAI（Ling/Ring）、Luma（Ray/Uni）等。
- 内部公司 ID 不变；`anthropic` 显示 Claude。用户指定的品牌显示名优先于通用规则：
  小米的公司品牌辨识度更高，因此 `xiaomi` 显示 Xiaomi，MiMo 家族仍归入该组。
- Agnes 沿用用户明确指定的展示归属，不冒充独立核实过的公司身份。
- `nv-mistralai/mistral-nemo-12b-instruct` 属联合开发，显示 Mistral AI / NVIDIA。
  NVIDIA 的 Minitron/Nemotron 衍生版本不能因基础模型名称而归给 Meta 或 Mistral。

## 实现边界

`modelDisplayCatalog.ts` 保留精确目录顺序和组 ID；`modelCompanyFamilies.ts` 管理家族规则。
精确目录优先；`:free`、`:batch`、`[free]` 包装继承精确型号归属与排序。
家族匹配不区分大小写，接受版本与常见后缀；冲突/未知命名空间不强行认领。
不按命名空间包揽所有模型，不扫描字符串中间的任意品牌片段。
当前讯飞 `model_map` 明确对应的四个原生 ID 单独登记：
`xop35qwen2b`、`xopqwen35v35b` → Qwen；`xophunyuan7bmt`、`xophunyuanocr` → Hunyuan。
这些别名依据当前明确配置，不泛化到任意 `xop*`，不改路由 ID。

## 盘点结果与证据边界

- 409 项进入开发者/模型品牌组。
- 25 项为搜索、随机路由或媒体适配器虚拟 ID，归入“工具与服务”，不宣称模型开发者。
- 7 项保留 `other`：`42-pro-0721-free`、`Meow`、`Muu-Translate-Auto`、`big-pickle`、
  `exo`、`space-bunny`、`space-bunny-alpha:free`。缺少可信开发者证据，不能把代理站点或社区猜测当作归属。
- 完整非秘密分组结果保存在临时目录 `gateway-model-classification-inventory-20261023.json`。
- 执行了只读盘点与分类函数；未执行测试套件、构建、推理调用、配置保存或重启。
  已补家族覆盖、精确别名、后缀、衍生模型、冲突命名空间和工具隔离测试源码。

## 本轮联网核实的主要来源

- [Meta Muse](https://ai.meta.com/tools/muse/)：与 Llama 共属 Meta。
- [Mistral 目录](https://docs.mistral.ai/models/)及 [Labs](https://docs.mistral.ai/inference/labs)：Codestral、Voxtral、Leanstral 等。
- [Mistral NeMo](https://huggingface.co/mistralai/Mistral-Nemo-Base-2407)：联合开发；
  [NVIDIA Minitron](https://huggingface.co/nvidia/Mistral-NeMo-Minitron-8B-Instruct)：衍生版本归属。
- [Luma 目录](https://docs.agents.lumalabs.ai/guides/model/)：Uni/Ray；[Poolside](https://poolside.ai/models)：Laguna；[TypeSafe](https://typesafe.ai/)：Jev。
- [InclusionAI Ling](https://github.com/inclusionAI/Ling)及 [Ring](https://github.com/inclusionAI/Ring)：同一开发者。
- [LongCat](https://longcat.ai/)：Meituan；[Yi](https://github.com/01-ai/Yi)：01-ai。
- [Dracarys](https://huggingface.co/abacusai/Dracarys-Llama-3.1-70B-Instruct)：Abacus，不能归 Meta。
- 发布者模型卡：[Fuyu](https://huggingface.co/adept/fuyu-8b)、[Jamba](https://huggingface.co/ai21labs/AI21-Jamba-Large-1.5)、
  [BGE](https://huggingface.co/BAAI/bge-m3)、[StarCoder](https://huggingface.co/bigcode/starcoder2-15b)、[SEA-LION](https://huggingface.co/aisingapore/SEA-LION-v1-7B-IT)。
- [Databricks DBRX](https://www.databricks.com/blog/introducing-dbrx-new-state-art-open-llm)；[Tencent HunyuanOCR](https://huggingface.co/tencent/HunyuanOCR)。
- [OpenCode 目录](https://dev.opencode.ai/v2/docs/console/models)：Big Pickle 服务标识，不据此认定权重开发者。
  Space Bunny 的第三方说明存在冲突，本次不据此认领。
