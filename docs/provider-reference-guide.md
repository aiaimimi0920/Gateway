# Gateway Provider Reference Guide

This guide closes the documentation and credential-reference gap for the
Gateway provider-line manifests without copying legacy provider material.
Manifest paths are retained as provenance. Each path is classified in the
deterministic report and can be resolved locally only after an operator
reviews a Gateway-owned, sanitized file.

## Resolution model

| Status | Meaning | Operator action |
| --- | --- | --- |
| `local_present` | The referenced file exists under Gateway. | Review its contents and keep material outside checked-in docs. |
| `external_legacy_reference` | The historical path is absent or not owned by Gateway. | Use the safe placeholder below; do not copy a sibling project tree. |

Current deterministic baseline: **176 declared references**, **166 unique paths**, **176 external legacy references**, and **0 unclassified references**.

`sourceRevision` is the content-addressed identity
`manifest-source:<sha256>` and does not depend on Git commit history.
`sourceFingerprint` is the SHA-256 of the same sorted manifest paths
and bytes after normalizing text line endings to LF, including uncommitted
source changes.

## Safe local template

The following values are intentionally non-credentials. Replace them only
in an ignored local file or an approved secret store; never put real values
in this guide, the report, or a manifest.

```json
{
  "value_slot": "INSERT_PROVIDER_VALUE_HERE",
  "state_file": "PATH_TO_LOCAL_STATE_FILE"
}
```

To promote a legacy reference to `local_present`, create the reviewed file
inside Gateway, rerun the generator, and inspect the report diff. The
generator never reads the file contents; the validator only checks that the
path is Gateway-owned and present.

## Reproducible commands

Run both commands from the Gateway repository root after changing a manifest or
intentionally adding a Gateway-owned reference:

```powershell
python tools/generate-gateway-provider-reference-report.py
python tools/validate-gateway-provider-reference-report.py --as-json
```

## Provider line catalog

<a id="line-accio-web-reverse-api"></a>
### accio-web-reverse-api

- Identity: `accio_platform` / `accio`
- Capabilities: `conversation`
- Material kinds: `bearer_token`, `session_auth`
- Manifest: `manifests/lines/accio/web-reverse-api.json`
- Resolution: `0` local present, `4` external legacy, `0` unclassified
- Legacy references:
  - `credentials.samplePath` -> `docs/20-ai-gateway/examples/credentials/accio/web_reverse_api/minimal.raw.sample.json` (`external_legacy_reference`; `path_not_present_in_gateway`)
  - `credentials.fieldsDocPath` -> `docs/20-ai-gateway/examples/credentials/accio/web_reverse_api/FIELDS.md` (`external_legacy_reference`; `path_not_present_in_gateway`)
  - `docs.overviewDocPath` -> `docs/20-ai-gateway/AI-gateway-accio-platform-baseline.md` (`external_legacy_reference`; `path_not_present_in_gateway`)
  - `docs.buildDocPath` -> `docs/20-ai-gateway/examples/credentials/accio/web_reverse_api/BUILD.md` (`external_legacy_reference`; `path_not_present_in_gateway`)

<a id="line-aistudio-official"></a>
### aistudio-official

- Identity: `aistudio_platform` / `google-gemini-api`
- Capabilities: `conversation`, `images`, `audio_speech`, `embeddings`, `models`
- Material kinds: `api_key`
- Manifest: `manifests/lines/aistudio/official.json`
- Resolution: `0` local present, `4` external legacy, `0` unclassified
- Legacy references:
  - `credentials.samplePath` -> `docs/20-ai-gateway/examples/credentials/gemini/official_api/google-gemini-api.minimal.raw.sample.json` (`external_legacy_reference`; `path_not_present_in_gateway`)
  - `credentials.fieldsDocPath` -> `docs/20-ai-gateway/examples/credentials/gemini/official_api/FIELDS.md` (`external_legacy_reference`; `path_not_present_in_gateway`)
  - `docs.overviewDocPath` -> `docs/20-ai-gateway/服务商实现线与Provider目录.md` (`external_legacy_reference`; `path_not_present_in_gateway`)
  - `docs.buildDocPath` -> `docs/20-ai-gateway/实现线可选编译与物理隔离规范.md` (`external_legacy_reference`; `path_not_present_in_gateway`)

<a id="line-aistudio-web-reverse"></a>
### aistudio-web-reverse

- Identity: `aistudio_platform` / `aistudio-web-reverse`
- Capabilities: `conversation`, `images`, `audio_speech`, `embeddings`, `models`
- Material kinds: `session_auth`, `browser_state`
- Manifest: `manifests/lines/aistudio/web-reverse.json`
- Resolution: `0` local present, `4` external legacy, `0` unclassified
- Legacy references:
  - `credentials.samplePath` -> `docs/20-ai-gateway/examples/credentials/aistudio/web_reverse/minimal.raw.sample.json` (`external_legacy_reference`; `path_not_present_in_gateway`)
  - `credentials.fieldsDocPath` -> `docs/20-ai-gateway/examples/credentials/aistudio/web_reverse/FIELDS.md` (`external_legacy_reference`; `path_not_present_in_gateway`)
  - `docs.overviewDocPath` -> `docs/20-ai-gateway/AIStudio Web Reverse基线.md` (`external_legacy_reference`; `path_not_present_in_gateway`)
  - `docs.buildDocPath` -> `docs/20-ai-gateway/实现线可选编译与物理隔离规范.md` (`external_legacy_reference`; `path_not_present_in_gateway`)

<a id="line-anthropic-messages-official-model-api"></a>
### anthropic-messages-official-model-api

- Identity: `anthropic_platform` / `anthropic-compatible`
- Capabilities: `conversation`, `models`
- Material kinds: `api_key`
- Manifest: `manifests/lines/anthropic/official-model-api.json`
- Resolution: `0` local present, `4` external legacy, `0` unclassified
- Legacy references:
  - `credentials.samplePath` -> `docs/20-ai-gateway/examples/credentials/anthropic/official_model_api/minimal.raw.sample.json` (`external_legacy_reference`; `path_not_present_in_gateway`)
  - `credentials.fieldsDocPath` -> `docs/20-ai-gateway/examples/credentials/anthropic/official_model_api/FIELDS.md` (`external_legacy_reference`; `path_not_present_in_gateway`)
  - `docs.overviewDocPath` -> `docs/20-ai-gateway/Anthropic Messages平台实现线、可选编译与物理隔离基线.md` (`external_legacy_reference`; `path_not_present_in_gateway`)
  - `docs.buildDocPath` -> `docs/20-ai-gateway/examples/credentials/anthropic/official_model_api/BUILD.md` (`external_legacy_reference`; `path_not_present_in_gateway`)

<a id="line-aws-bedrock-converse-official-model-api"></a>
### aws-bedrock-converse-official-model-api

- Identity: `aws_bedrock_platform` / `bedrock-converse`
- Capabilities: `conversation`, `models`
- Material kinds: `bearer_token`
- Manifest: `manifests/lines/aws_bedrock/official-model-api.json`
- Resolution: `0` local present, `4` external legacy, `0` unclassified
- Legacy references:
  - `credentials.samplePath` -> `docs/20-ai-gateway/examples/credentials/aws_bedrock/official_model_api/minimal.raw.sample.json` (`external_legacy_reference`; `path_not_present_in_gateway`)
  - `credentials.fieldsDocPath` -> `docs/20-ai-gateway/examples/credentials/aws_bedrock/official_model_api/FIELDS.md` (`external_legacy_reference`; `path_not_present_in_gateway`)
  - `docs.overviewDocPath` -> `docs/20-ai-gateway/AWS Bedrock Converse平台实现线、可选编译与物理隔离基线.md` (`external_legacy_reference`; `path_not_present_in_gateway`)
  - `docs.buildDocPath` -> `docs/20-ai-gateway/examples/credentials/aws_bedrock/official_model_api/BUILD.md` (`external_legacy_reference`; `path_not_present_in_gateway`)

<a id="line-azure-openai-official-vendor-api"></a>
### azure-openai-official-vendor-api

- Identity: `azure_openai_platform` / `azure-openai`
- Capabilities: `conversation`, `models`
- Material kinds: `api_key`
- Manifest: `manifests/lines/azure_openai/official-vendor-api.json`
- Resolution: `0` local present, `4` external legacy, `0` unclassified
- Legacy references:
  - `credentials.samplePath` -> `docs/20-ai-gateway/examples/credentials/azure_openai/official_vendor_api/minimal.raw.sample.json` (`external_legacy_reference`; `path_not_present_in_gateway`)
  - `credentials.fieldsDocPath` -> `docs/20-ai-gateway/examples/credentials/azure_openai/official_vendor_api/FIELDS.md` (`external_legacy_reference`; `path_not_present_in_gateway`)
  - `docs.overviewDocPath` -> `docs/20-ai-gateway/Azure OpenAI平台实现线、可选编译与物理隔离基线.md` (`external_legacy_reference`; `path_not_present_in_gateway`)
  - `docs.buildDocPath` -> `docs/20-ai-gateway/examples/credentials/azure_openai/official_vendor_api/BUILD.md` (`external_legacy_reference`; `path_not_present_in_gateway`)

<a id="line-chataibot-web-reverse"></a>
### chataibot-web-reverse

- Identity: `chataibot_platform` / `chataibot-images`
- Capabilities: `images`, `models`
- Material kinds: `session_auth`, `browser_state`
- Manifest: `manifests/lines/chataibot/web-reverse.json`
- Resolution: `0` local present, `4` external legacy, `0` unclassified
- Legacy references:
  - `credentials.samplePath` -> `docs/20-ai-gateway/examples/credentials/chataibot/web_reverse/minimal.raw.sample.json` (`external_legacy_reference`; `path_not_present_in_gateway`)
  - `credentials.fieldsDocPath` -> `docs/20-ai-gateway/examples/credentials/chataibot/web_reverse/FIELDS.md` (`external_legacy_reference`; `path_not_present_in_gateway`)
  - `docs.overviewDocPath` -> `docs/20-ai-gateway/ChatAIBot图片实现线基线.md` (`external_legacy_reference`; `path_not_present_in_gateway`)
  - `docs.buildDocPath` -> `docs/20-ai-gateway/examples/credentials/chataibot/web_reverse/BUILD.md` (`external_legacy_reference`; `path_not_present_in_gateway`)

<a id="line-chatgpt-codex-oauth-official"></a>
### chatgpt-codex-oauth-official

- Identity: `chatgpt_platform` / `codex`
- Capabilities: `conversation`, `models`
- Material kinds: `bearer_token`
- Manifest: `manifests/lines/chatgpt/codex-oauth-official.json`
- Resolution: `0` local present, `4` external legacy, `0` unclassified
- Legacy references:
  - `credentials.samplePath` -> `docs/20-ai-gateway/examples/credentials/chatgpt/codex_backend/minimal.raw.sample.json` (`external_legacy_reference`; `path_not_present_in_gateway`)
  - `credentials.fieldsDocPath` -> `docs/20-ai-gateway/examples/credentials/chatgpt/codex_backend/FIELDS.md` (`external_legacy_reference`; `path_not_present_in_gateway`)
  - `docs.overviewDocPath` -> `docs/20-ai-gateway/ChatGPT官方API与WebReverse双线路基线.md` (`external_legacy_reference`; `path_not_present_in_gateway`)
  - `docs.buildDocPath` -> `docs/20-ai-gateway/examples/credentials/chatgpt/codex_backend/BUILD.md` (`external_legacy_reference`; `path_not_present_in_gateway`)

<a id="line-chatgpt-official-api"></a>
### chatgpt-official-api

- Identity: `chatgpt_platform` / `openai-platform`
- Capabilities: `conversation`, `embeddings`, `audio_transcriptions`, `audio_speech`, `images`, `models`
- Material kinds: `api_key`
- Manifest: `manifests/lines/chatgpt/official-api.json`
- Resolution: `0` local present, `4` external legacy, `0` unclassified
- Legacy references:
  - `credentials.samplePath` -> `docs/20-ai-gateway/examples/credentials/chatgpt/official_api/minimal.raw.sample.json` (`external_legacy_reference`; `path_not_present_in_gateway`)
  - `credentials.fieldsDocPath` -> `docs/20-ai-gateway/examples/credentials/chatgpt/official_api/FIELDS.md` (`external_legacy_reference`; `path_not_present_in_gateway`)
  - `docs.overviewDocPath` -> `docs/20-ai-gateway/ChatGPT官方API与WebReverse双线路基线.md` (`external_legacy_reference`; `path_not_present_in_gateway`)
  - `docs.buildDocPath` -> `docs/20-ai-gateway/examples/credentials/chatgpt/official_api/BUILD.md` (`external_legacy_reference`; `path_not_present_in_gateway`)

<a id="line-chatgpt-web-reverse"></a>
### chatgpt-web-reverse

- Identity: `chatgpt_platform` / `chatgpt-web-reverse`
- Capabilities: `conversation`, `models`
- Material kinds: `session_auth`, `browser_state`
- Manifest: `manifests/lines/chatgpt/web-reverse.json`
- Resolution: `0` local present, `4` external legacy, `0` unclassified
- Legacy references:
  - `credentials.samplePath` -> `docs/20-ai-gateway/examples/credentials/chatgpt/web_reverse/minimal.raw.sample.json` (`external_legacy_reference`; `path_not_present_in_gateway`)
  - `credentials.fieldsDocPath` -> `docs/20-ai-gateway/examples/credentials/chatgpt/web_reverse/FIELDS.md` (`external_legacy_reference`; `path_not_present_in_gateway`)
  - `docs.overviewDocPath` -> `docs/20-ai-gateway/ChatGPT官方API与WebReverse双线路基线.md` (`external_legacy_reference`; `path_not_present_in_gateway`)
  - `docs.buildDocPath` -> `docs/20-ai-gateway/examples/credentials/chatgpt/web_reverse/BUILD.md` (`external_legacy_reference`; `path_not_present_in_gateway`)

<a id="line-cohere-chat-official-model-api"></a>
### cohere-chat-official-model-api

- Identity: `cohere_platform` / `cohere-chat`
- Capabilities: `conversation`, `models`
- Material kinds: `api_key`
- Manifest: `manifests/lines/cohere/official-model-api.json`
- Resolution: `0` local present, `4` external legacy, `0` unclassified
- Legacy references:
  - `credentials.samplePath` -> `docs/20-ai-gateway/examples/credentials/cohere/official_model_api/minimal.raw.sample.json` (`external_legacy_reference`; `path_not_present_in_gateway`)
  - `credentials.fieldsDocPath` -> `docs/20-ai-gateway/examples/credentials/cohere/official_model_api/FIELDS.md` (`external_legacy_reference`; `path_not_present_in_gateway`)
  - `docs.overviewDocPath` -> `docs/20-ai-gateway/Cohere Chat平台实现线、可选编译与物理隔离基线.md` (`external_legacy_reference`; `path_not_present_in_gateway`)
  - `docs.buildDocPath` -> `docs/20-ai-gateway/examples/credentials/cohere/official_model_api/BUILD.md` (`external_legacy_reference`; `path_not_present_in_gateway`)

<a id="line-deepseek-openai-official-model-api"></a>
### deepseek-openai-official-model-api

- Identity: `deepseek_platform` / `deepseek-openai`
- Capabilities: `conversation`, `models`
- Material kinds: `api_key`
- Manifest: `manifests/lines/deepseek/official-model-api.json`
- Resolution: `0` local present, `4` external legacy, `0` unclassified
- Legacy references:
  - `credentials.samplePath` -> `docs/20-ai-gateway/examples/credentials/deepseek/official_model_api/minimal.raw.sample.json` (`external_legacy_reference`; `path_not_present_in_gateway`)
  - `credentials.fieldsDocPath` -> `docs/20-ai-gateway/examples/credentials/deepseek/official_model_api/FIELDS.md` (`external_legacy_reference`; `path_not_present_in_gateway`)
  - `docs.overviewDocPath` -> `docs/20-ai-gateway/DeepSeek平台实现线、可选编译与物理隔离基线.md` (`external_legacy_reference`; `path_not_present_in_gateway`)
  - `docs.buildDocPath` -> `docs/20-ai-gateway/examples/credentials/deepseek/official_model_api/BUILD.md` (`external_legacy_reference`; `path_not_present_in_gateway`)

<a id="line-exa-search-official-vendor-api"></a>
### exa-search-official-vendor-api

- Identity: `exa_platform` / `exa-search`
- Capabilities: `search`, `fetch`
- Material kinds: `api_key`
- Manifest: `manifests/lines/exa/official-vendor-api.json`
- Resolution: `0` local present, `4` external legacy, `0` unclassified
- Legacy references:
  - `credentials.samplePath` -> `docs/20-ai-gateway/examples/credentials/exa/official_vendor_api/minimal.raw.sample.json` (`external_legacy_reference`; `path_not_present_in_gateway`)
  - `credentials.fieldsDocPath` -> `docs/20-ai-gateway/examples/credentials/exa/official_vendor_api/FIELDS.md` (`external_legacy_reference`; `path_not_present_in_gateway`)
  - `docs.overviewDocPath` -> `docs/20-ai-gateway/Exa Search平台实现线、可选编译与物理隔离基线.md` (`external_legacy_reference`; `path_not_present_in_gateway`)
  - `docs.buildDocPath` -> `docs/20-ai-gateway/examples/credentials/exa/official_vendor_api/BUILD.md` (`external_legacy_reference`; `path_not_present_in_gateway`)

<a id="line-freebuff-web-reverse-api"></a>
### freebuff-web-reverse-api

- Identity: `freebuff_platform` / `freebuff-compatible`
- Capabilities: `conversation`
- Material kinds: `bearer_token`, `session_auth`
- Manifest: `manifests/lines/freebuff/web-reverse-api.json`
- Resolution: `0` local present, `4` external legacy, `0` unclassified
- Legacy references:
  - `credentials.samplePath` -> `docs/20-ai-gateway/examples/credentials/freebuff/web_reverse_api/minimal.raw.sample.json` (`external_legacy_reference`; `path_not_present_in_gateway`)
  - `credentials.fieldsDocPath` -> `docs/20-ai-gateway/examples/credentials/freebuff/web_reverse_api/FIELDS.md` (`external_legacy_reference`; `path_not_present_in_gateway`)
  - `docs.overviewDocPath` -> `docs/20-ai-gateway/AI网关FreeBuff兼容Provider接入基线.md` (`external_legacy_reference`; `path_not_present_in_gateway`)
  - `docs.buildDocPath` -> `docs/20-ai-gateway/examples/credentials/freebuff/web_reverse_api/BUILD.md` (`external_legacy_reference`; `path_not_present_in_gateway`)

<a id="line-gemini-canvas-program"></a>
### gemini-canvas-program

- Identity: `gemini_platform` / `gemini-canvas-program-relay`
- Capabilities: `conversation`, `images`, `audio_speech`, `music`, `video`, `models`
- Material kinds: `session_auth`, `browser_state`
- Manifest: `manifests/lines/gemini/canvas-program.json`
- Resolution: `0` local present, `4` external legacy, `0` unclassified
- Legacy references:
  - `credentials.samplePath` -> `docs/20-ai-gateway/examples/credentials/gemini/canvas_web_reverse/minimal.raw.sample.json` (`external_legacy_reference`; `path_not_present_in_gateway`)
  - `credentials.fieldsDocPath` -> `docs/20-ai-gateway/examples/credentials/gemini/canvas_web_reverse/FIELDS.md` (`external_legacy_reference`; `path_not_present_in_gateway`)
  - `docs.overviewDocPath` -> `docs/20-ai-gateway/Gemini三线路与Canvas派生运行时架构规范.md` (`external_legacy_reference`; `path_not_present_in_gateway`)
  - `docs.buildDocPath` -> `docs/20-ai-gateway/服务商实现线与Provider目录.md` (`external_legacy_reference`; `path_not_present_in_gateway`)

<a id="line-gemini-web-reverse"></a>
### gemini-web-reverse

- Identity: `gemini_platform` / `gemini-web-chat`
- Capabilities: `conversation`, `images`, `audio_speech`, `models`
- Material kinds: `session_auth`, `browser_state`
- Manifest: `manifests/lines/gemini/web-reverse.json`
- Resolution: `0` local present, `4` external legacy, `0` unclassified
- Legacy references:
  - `credentials.samplePath` -> `docs/20-ai-gateway/examples/credentials/gemini/web_reverse/minimal.raw.sample.json` (`external_legacy_reference`; `path_not_present_in_gateway`)
  - `credentials.fieldsDocPath` -> `docs/20-ai-gateway/examples/credentials/gemini/web_reverse/FIELDS.md` (`external_legacy_reference`; `path_not_present_in_gateway`)
  - `docs.overviewDocPath` -> `docs/20-ai-gateway/Gemini三线路与Canvas派生运行时架构规范.md` (`external_legacy_reference`; `path_not_present_in_gateway`)
  - `docs.buildDocPath` -> `docs/20-ai-gateway/服务商实现线与Provider目录.md` (`external_legacy_reference`; `path_not_present_in_gateway`)

<a id="line-google-agent-platform-official"></a>
### google-agent-platform-official

- Identity: `gemini_platform` / `google-vertex-gemini`
- Capabilities: `conversation`, `models`
- Material kinds: `bearer_token`
- Manifest: `manifests/lines/gemini/google-agent-platform-official.json`
- Resolution: `0` local present, `4` external legacy, `0` unclassified
- Legacy references:
  - `credentials.samplePath` -> `docs/20-ai-gateway/examples/credentials/gemini/official_api/google-vertex-gemini.minimal.raw.sample.json` (`external_legacy_reference`; `path_not_present_in_gateway`)
  - `credentials.fieldsDocPath` -> `docs/20-ai-gateway/examples/credentials/gemini/official_api/FIELDS.md` (`external_legacy_reference`; `path_not_present_in_gateway`)
  - `docs.overviewDocPath` -> `docs/20-ai-gateway/服务商实现线与Provider目录.md` (`external_legacy_reference`; `path_not_present_in_gateway`)
  - `docs.buildDocPath` -> `docs/20-ai-gateway/实现线可选编译与物理隔离规范.md` (`external_legacy_reference`; `path_not_present_in_gateway`)

<a id="line-grok-web-reverse-api"></a>
### grok-web-reverse-api

- Identity: `grok_platform` / `grok`
- Capabilities: `conversation`
- Material kinds: `session_auth`
- Manifest: `manifests/lines/grok/web-reverse-api.json`
- Resolution: `0` local present, `4` external legacy, `0` unclassified
- Legacy references:
  - `credentials.samplePath` -> `docs/20-ai-gateway/examples/credentials/grok/web_reverse_api/minimal.raw.sample.json` (`external_legacy_reference`; `path_not_present_in_gateway`)
  - `credentials.fieldsDocPath` -> `docs/20-ai-gateway/examples/credentials/grok/web_reverse_api/FIELDS.md` (`external_legacy_reference`; `path_not_present_in_gateway`)
  - `docs.overviewDocPath` -> `docs/20-ai-gateway/Grok平台实现线、可选编译与物理隔离基线.md` (`external_legacy_reference`; `path_not_present_in_gateway`)
  - `docs.buildDocPath` -> `docs/20-ai-gateway/examples/credentials/grok/web_reverse_api/BUILD.md` (`external_legacy_reference`; `path_not_present_in_gateway`)

<a id="line-groq-openai-official-vendor-api"></a>
### groq-openai-official-vendor-api

- Identity: `groq_platform` / `groq-openai`
- Capabilities: `conversation`, `models`
- Material kinds: `api_key`
- Manifest: `manifests/lines/groq/official-vendor-api.json`
- Resolution: `0` local present, `4` external legacy, `0` unclassified
- Legacy references:
  - `credentials.samplePath` -> `docs/20-ai-gateway/examples/credentials/groq/official_vendor_api/minimal.raw.sample.json` (`external_legacy_reference`; `path_not_present_in_gateway`)
  - `credentials.fieldsDocPath` -> `docs/20-ai-gateway/examples/credentials/groq/official_vendor_api/FIELDS.md` (`external_legacy_reference`; `path_not_present_in_gateway`)
  - `docs.overviewDocPath` -> `docs/20-ai-gateway/Groq平台实现线、可选编译与物理隔离基线.md` (`external_legacy_reference`; `path_not_present_in_gateway`)
  - `docs.buildDocPath` -> `docs/20-ai-gateway/examples/credentials/groq/official_vendor_api/BUILD.md` (`external_legacy_reference`; `path_not_present_in_gateway`)

<a id="line-jina-reader-official-vendor-api"></a>
### jina-reader-official-vendor-api

- Identity: `jina_platform` / `jina-reader`
- Capabilities: `fetch`
- Material kinds: `api_key`
- Manifest: `manifests/lines/jina/reader-official-vendor-api.json`
- Resolution: `0` local present, `4` external legacy, `0` unclassified
- Legacy references:
  - `credentials.samplePath` -> `docs/20-ai-gateway/examples/credentials/jina/reader_official_vendor_api/minimal.raw.sample.json` (`external_legacy_reference`; `path_not_present_in_gateway`)
  - `credentials.fieldsDocPath` -> `docs/20-ai-gateway/examples/credentials/jina/reader_official_vendor_api/FIELDS.md` (`external_legacy_reference`; `path_not_present_in_gateway`)
  - `docs.overviewDocPath` -> `docs/20-ai-gateway/Jina Reader平台实现线、可选编译与物理隔离基线.md` (`external_legacy_reference`; `path_not_present_in_gateway`)
  - `docs.buildDocPath` -> `docs/20-ai-gateway/examples/credentials/jina/reader_official_vendor_api/BUILD.md` (`external_legacy_reference`; `path_not_present_in_gateway`)

<a id="line-jina-search-official-vendor-api"></a>
### jina-search-official-vendor-api

- Identity: `jina_platform` / `jina-search`
- Capabilities: `search`
- Material kinds: `api_key`
- Manifest: `manifests/lines/jina/search-official-vendor-api.json`
- Resolution: `0` local present, `4` external legacy, `0` unclassified
- Legacy references:
  - `credentials.samplePath` -> `docs/20-ai-gateway/examples/credentials/jina/search_official_vendor_api/minimal.raw.sample.json` (`external_legacy_reference`; `path_not_present_in_gateway`)
  - `credentials.fieldsDocPath` -> `docs/20-ai-gateway/examples/credentials/jina/search_official_vendor_api/FIELDS.md` (`external_legacy_reference`; `path_not_present_in_gateway`)
  - `docs.overviewDocPath` -> `docs/20-ai-gateway/Jina Search平台实现线、可选编译与物理隔离基线.md` (`external_legacy_reference`; `path_not_present_in_gateway`)
  - `docs.buildDocPath` -> `docs/20-ai-gateway/examples/credentials/jina/search_official_vendor_api/BUILD.md` (`external_legacy_reference`; `path_not_present_in_gateway`)

<a id="line-kiro-official-vendor-api"></a>
### kiro-official-vendor-api

- Identity: `kiro_platform` / `kiro-compatible`
- Capabilities: `conversation`
- Material kinds: `bearer_token`
- Manifest: `manifests/lines/kiro/official-vendor-api.json`
- Resolution: `0` local present, `4` external legacy, `0` unclassified
- Legacy references:
  - `credentials.samplePath` -> `docs/20-ai-gateway/examples/credentials/kiro/official_vendor_api/minimal.raw.sample.json` (`external_legacy_reference`; `path_not_present_in_gateway`)
  - `credentials.fieldsDocPath` -> `docs/20-ai-gateway/examples/credentials/kiro/official_vendor_api/FIELDS.md` (`external_legacy_reference`; `path_not_present_in_gateway`)
  - `docs.overviewDocPath` -> `docs/20-ai-gateway/Kiro-compatible平台实现线、可选编译与物理隔离基线.md` (`external_legacy_reference`; `path_not_present_in_gateway`)
  - `docs.buildDocPath` -> `docs/20-ai-gateway/examples/credentials/kiro/official_vendor_api/BUILD.md` (`external_legacy_reference`; `path_not_present_in_gateway`)

<a id="line-linkup-search-official-vendor-api"></a>
### linkup-search-official-vendor-api

- Identity: `linkup_platform` / `linkup-search`
- Capabilities: `search`, `fetch`, `research`, `credits_balance`
- Material kinds: `api_key`
- Manifest: `manifests/lines/linkup/official-vendor-api.json`
- Resolution: `0` local present, `4` external legacy, `0` unclassified
- Legacy references:
  - `credentials.samplePath` -> `docs/20-ai-gateway/examples/credentials/linkup/official_vendor_api/minimal.raw.sample.json` (`external_legacy_reference`; `path_not_present_in_gateway`)
  - `credentials.fieldsDocPath` -> `docs/20-ai-gateway/examples/credentials/linkup/official_vendor_api/FIELDS.md` (`external_legacy_reference`; `path_not_present_in_gateway`)
  - `docs.overviewDocPath` -> `docs/20-ai-gateway/Linkup Search平台实现线、可选编译与物理隔离基线.md` (`external_legacy_reference`; `path_not_present_in_gateway`)
  - `docs.buildDocPath` -> `docs/20-ai-gateway/examples/credentials/linkup/official_vendor_api/BUILD.md` (`external_legacy_reference`; `path_not_present_in_gateway`)

<a id="line-longcat-openai-official-model-api"></a>
### longcat-openai-official-model-api

- Identity: `longcat_platform` / `longcat-openai`
- Capabilities: `conversation`, `models`
- Material kinds: `api_key`
- Manifest: `manifests/lines/longcat/official-model-api.json`
- Resolution: `0` local present, `4` external legacy, `0` unclassified
- Legacy references:
  - `credentials.samplePath` -> `docs/20-ai-gateway/examples/credentials/longcat/official_model_api/minimal.raw.sample.json` (`external_legacy_reference`; `path_not_present_in_gateway`)
  - `credentials.fieldsDocPath` -> `docs/20-ai-gateway/examples/credentials/longcat/official_model_api/FIELDS.md` (`external_legacy_reference`; `path_not_present_in_gateway`)
  - `docs.overviewDocPath` -> `docs/20-ai-gateway/LongCat官方模型API实现线基线.md` (`external_legacy_reference`; `path_not_present_in_gateway`)
  - `docs.buildDocPath` -> `docs/20-ai-gateway/examples/credentials/longcat/official_model_api/BUILD.md` (`external_legacy_reference`; `path_not_present_in_gateway`)

<a id="line-lumalabs-web-reverse-api"></a>
### lumalabs-web-reverse-api

- Identity: `lumalabs_platform` / `lumalabs`
- Capabilities: `models`, `images`, `music`, `videos`
- Material kinds: `session_auth`, `browser_state`
- Manifest: `manifests/lines/lumalabs/web-reverse-api.json`
- Resolution: `0` local present, `4` external legacy, `0` unclassified
- Legacy references:
  - `credentials.samplePath` -> `docs/20-ai-gateway/examples/credentials/lumalabs/web_reverse_api/minimal.raw.sample.json` (`external_legacy_reference`; `path_not_present_in_gateway`)
  - `credentials.fieldsDocPath` -> `docs/20-ai-gateway/examples/credentials/lumalabs/web_reverse_api/FIELDS.md` (`external_legacy_reference`; `path_not_present_in_gateway`)
  - `docs.overviewDocPath` -> `docs/20-ai-gateway/LumaLabs平台实现线、可选编译与物理隔离基线.md` (`external_legacy_reference`; `path_not_present_in_gateway`)
  - `docs.buildDocPath` -> `docs/20-ai-gateway/examples/credentials/lumalabs/web_reverse_api/BUILD.md` (`external_legacy_reference`; `path_not_present_in_gateway`)

<a id="line-mistral-openai-official-model-api"></a>
### mistral-openai-official-model-api

- Identity: `mistral_platform` / `mistral-openai`
- Capabilities: `conversation`, `models`
- Material kinds: `api_key`
- Manifest: `manifests/lines/mistral/official-model-api.json`
- Resolution: `0` local present, `4` external legacy, `0` unclassified
- Legacy references:
  - `credentials.samplePath` -> `docs/20-ai-gateway/examples/credentials/mistral/official_model_api/minimal.raw.sample.json` (`external_legacy_reference`; `path_not_present_in_gateway`)
  - `credentials.fieldsDocPath` -> `docs/20-ai-gateway/examples/credentials/mistral/official_model_api/FIELDS.md` (`external_legacy_reference`; `path_not_present_in_gateway`)
  - `docs.overviewDocPath` -> `docs/20-ai-gateway/Mistral平台实现线、可选编译与物理隔离基线.md` (`external_legacy_reference`; `path_not_present_in_gateway`)
  - `docs.buildDocPath` -> `docs/20-ai-gateway/examples/credentials/mistral/official_model_api/BUILD.md` (`external_legacy_reference`; `path_not_present_in_gateway`)

<a id="line-muyuan-openai-aggregator-api"></a>
### muyuan-openai-aggregator-api

- Identity: `muyuan_platform` / `muyuan-openai`
- Capabilities: `conversation`, `models`
- Material kinds: `api_key`
- Manifest: `manifests/lines/muyuan/aggregator-api.json`
- Resolution: `0` local present, `4` external legacy, `0` unclassified
- Legacy references:
  - `credentials.samplePath` -> `docs/20-ai-gateway/examples/credentials/muyuan/aggregator_api/minimal.raw.sample.json` (`external_legacy_reference`; `path_not_present_in_gateway`)
  - `credentials.fieldsDocPath` -> `docs/20-ai-gateway/examples/credentials/muyuan/aggregator_api/FIELDS.md` (`external_legacy_reference`; `path_not_present_in_gateway`)
  - `docs.overviewDocPath` -> `docs/20-ai-gateway/Muyuan聚合API实现线基线.md` (`external_legacy_reference`; `path_not_present_in_gateway`)
  - `docs.buildDocPath` -> `docs/20-ai-gateway/examples/credentials/muyuan/aggregator_api/BUILD.md` (`external_legacy_reference`; `path_not_present_in_gateway`)

<a id="line-nvidia-openai-official-vendor-api"></a>
### nvidia-openai-official-vendor-api

- Identity: `nvidia_platform` / `nvidia-openai`
- Capabilities: `conversation`, `models`
- Material kinds: `api_key`
- Manifest: `manifests/lines/nvidia/official-vendor-api.json`
- Resolution: `0` local present, `4` external legacy, `0` unclassified
- Legacy references:
  - `credentials.samplePath` -> `docs/20-ai-gateway/examples/credentials/nvidia/official_vendor_api/minimal.raw.sample.json` (`external_legacy_reference`; `path_not_present_in_gateway`)
  - `credentials.fieldsDocPath` -> `docs/20-ai-gateway/examples/credentials/nvidia/official_vendor_api/FIELDS.md` (`external_legacy_reference`; `path_not_present_in_gateway`)
  - `docs.overviewDocPath` -> `docs/20-ai-gateway/NVIDIA平台实现线、可选编译与物理隔离基线.md` (`external_legacy_reference`; `path_not_present_in_gateway`)
  - `docs.buildDocPath` -> `docs/20-ai-gateway/examples/credentials/nvidia/official_vendor_api/BUILD.md` (`external_legacy_reference`; `path_not_present_in_gateway`)

<a id="line-openrouter-openai-aggregator-api"></a>
### openrouter-openai-aggregator-api

- Identity: `openrouter_platform` / `openrouter-openai`
- Capabilities: `conversation`, `models`
- Material kinds: `api_key`
- Manifest: `manifests/lines/openrouter/aggregator-api.json`
- Resolution: `0` local present, `4` external legacy, `0` unclassified
- Legacy references:
  - `credentials.samplePath` -> `docs/20-ai-gateway/examples/credentials/openrouter/aggregator_api/minimal.raw.sample.json` (`external_legacy_reference`; `path_not_present_in_gateway`)
  - `credentials.fieldsDocPath` -> `docs/20-ai-gateway/examples/credentials/openrouter/aggregator_api/FIELDS.md` (`external_legacy_reference`; `path_not_present_in_gateway`)
  - `docs.overviewDocPath` -> `docs/20-ai-gateway/OpenRouter平台实现线、可选编译与物理隔离基线.md` (`external_legacy_reference`; `path_not_present_in_gateway`)
  - `docs.buildDocPath` -> `docs/20-ai-gateway/examples/credentials/openrouter/aggregator_api/BUILD.md` (`external_legacy_reference`; `path_not_present_in_gateway`)

<a id="line-perplexity-chat-official-vendor-api"></a>
### perplexity-chat-official-vendor-api

- Identity: `perplexity_platform` / `perplexity-chat`
- Capabilities: `conversation`, `models`
- Material kinds: `api_key`
- Manifest: `manifests/lines/perplexity/chat-official-vendor-api.json`
- Resolution: `0` local present, `4` external legacy, `0` unclassified
- Legacy references:
  - `credentials.samplePath` -> `docs/20-ai-gateway/examples/credentials/perplexity/chat_official_vendor_api/minimal.raw.sample.json` (`external_legacy_reference`; `path_not_present_in_gateway`)
  - `credentials.fieldsDocPath` -> `docs/20-ai-gateway/examples/credentials/perplexity/chat_official_vendor_api/FIELDS.md` (`external_legacy_reference`; `path_not_present_in_gateway`)
  - `docs.overviewDocPath` -> `docs/20-ai-gateway/Perplexity Chat平台实现线、可选编译与物理隔离基线.md` (`external_legacy_reference`; `path_not_present_in_gateway`)
  - `docs.buildDocPath` -> `docs/20-ai-gateway/examples/credentials/perplexity/chat_official_vendor_api/BUILD.md` (`external_legacy_reference`; `path_not_present_in_gateway`)

<a id="line-perplexity-search-official-vendor-api"></a>
### perplexity-search-official-vendor-api

- Identity: `perplexity_platform` / `perplexity-search`
- Capabilities: `search`
- Material kinds: `api_key`
- Manifest: `manifests/lines/perplexity/official-vendor-api.json`
- Resolution: `0` local present, `4` external legacy, `0` unclassified
- Legacy references:
  - `credentials.samplePath` -> `docs/20-ai-gateway/examples/credentials/perplexity/official_vendor_api/minimal.raw.sample.json` (`external_legacy_reference`; `path_not_present_in_gateway`)
  - `credentials.fieldsDocPath` -> `docs/20-ai-gateway/examples/credentials/perplexity/official_vendor_api/FIELDS.md` (`external_legacy_reference`; `path_not_present_in_gateway`)
  - `docs.overviewDocPath` -> `docs/20-ai-gateway/Perplexity Search平台实现线、可选编译与物理隔离基线.md` (`external_legacy_reference`; `path_not_present_in_gateway`)
  - `docs.buildDocPath` -> `docs/20-ai-gateway/examples/credentials/perplexity/official_vendor_api/BUILD.md` (`external_legacy_reference`; `path_not_present_in_gateway`)

<a id="line-poe-openai-aggregator-api"></a>
### poe-openai-aggregator-api

- Identity: `poe_platform` / `poe-openai`
- Capabilities: `conversation`, `models`
- Material kinds: `api_key`
- Manifest: `manifests/lines/poe/aggregator-api.json`
- Resolution: `0` local present, `4` external legacy, `0` unclassified
- Legacy references:
  - `credentials.samplePath` -> `docs/20-ai-gateway/examples/credentials/poe/aggregator_api/minimal.raw.sample.json` (`external_legacy_reference`; `path_not_present_in_gateway`)
  - `credentials.fieldsDocPath` -> `docs/20-ai-gateway/examples/credentials/poe/aggregator_api/FIELDS.md` (`external_legacy_reference`; `path_not_present_in_gateway`)
  - `docs.overviewDocPath` -> `docs/20-ai-gateway/Poe聚合API实现线基线.md` (`external_legacy_reference`; `path_not_present_in_gateway`)
  - `docs.buildDocPath` -> `docs/20-ai-gateway/examples/credentials/poe/aggregator_api/BUILD.md` (`external_legacy_reference`; `path_not_present_in_gateway`)

<a id="line-producer-web-reverse-api"></a>
### producer-web-reverse-api

- Identity: `producer_platform` / `producer`
- Capabilities: `images`, `music`, `videos`
- Material kinds: `session_auth`, `browser_state`
- Manifest: `manifests/lines/producer/web-reverse-api.json`
- Resolution: `0` local present, `4` external legacy, `0` unclassified
- Legacy references:
  - `credentials.samplePath` -> `docs/20-ai-gateway/examples/credentials/producer/web_reverse_api/minimal.raw.sample.json` (`external_legacy_reference`; `path_not_present_in_gateway`)
  - `credentials.fieldsDocPath` -> `docs/20-ai-gateway/examples/credentials/producer/web_reverse_api/FIELDS.md` (`external_legacy_reference`; `path_not_present_in_gateway`)
  - `docs.overviewDocPath` -> `docs/20-ai-gateway/Producer.ai平台实现线、可选编译与物理隔离基线.md` (`external_legacy_reference`; `path_not_present_in_gateway`)
  - `docs.buildDocPath` -> `docs/20-ai-gateway/examples/credentials/producer/web_reverse_api/BUILD.md` (`external_legacy_reference`; `path_not_present_in_gateway`)

<a id="line-qwen-official-api"></a>
### qwen-official-api

- Identity: `qwen_platform` / `qwen-dashscope-openai`
- Capabilities: `conversation`, `models`
- Material kinds: `api_key`
- Manifest: `manifests/lines/qwen/official-api.json`
- Resolution: `0` local present, `4` external legacy, `0` unclassified
- Legacy references:
  - `credentials.samplePath` -> `docs/20-ai-gateway/examples/credentials/qwen/official_api/minimal.raw.sample.json` (`external_legacy_reference`; `path_not_present_in_gateway`)
  - `credentials.fieldsDocPath` -> `docs/20-ai-gateway/examples/credentials/qwen/official_api/FIELDS.md` (`external_legacy_reference`; `path_not_present_in_gateway`)
  - `docs.overviewDocPath` -> `docs/20-ai-gateway/Qwen平台实现线、可选编译与物理隔离基线.md` (`external_legacy_reference`; `path_not_present_in_gateway`)
  - `docs.buildDocPath` -> `docs/20-ai-gateway/examples/credentials/qwen/official_api/BUILD.md` (`external_legacy_reference`; `path_not_present_in_gateway`)

<a id="line-qwen-web-reverse"></a>
### qwen-web-reverse

- Identity: `qwen_platform` / `qwen-web-chat`
- Capabilities: `conversation`, `models`
- Material kinds: `session_auth`, `browser_state`
- Manifest: `manifests/lines/qwen/web-reverse.json`
- Resolution: `0` local present, `4` external legacy, `0` unclassified
- Legacy references:
  - `credentials.samplePath` -> `docs/20-ai-gateway/examples/credentials/qwen/web_reverse/minimal.raw.sample.json` (`external_legacy_reference`; `path_not_present_in_gateway`)
  - `credentials.fieldsDocPath` -> `docs/20-ai-gateway/examples/credentials/qwen/web_reverse/FIELDS.md` (`external_legacy_reference`; `path_not_present_in_gateway`)
  - `docs.overviewDocPath` -> `docs/20-ai-gateway/Qwen平台实现线、可选编译与物理隔离基线.md` (`external_legacy_reference`; `path_not_present_in_gateway`)
  - `docs.buildDocPath` -> `docs/20-ai-gateway/examples/credentials/qwen/web_reverse/BUILD.md` (`external_legacy_reference`; `path_not_present_in_gateway`)

<a id="line-suno-web-reverse-api"></a>
### suno-web-reverse-api

- Identity: `suno_platform` / `suno`
- Capabilities: `models`, `images`, `music`, `videos`
- Material kinds: `session_auth`, `browser_state`
- Manifest: `manifests/lines/suno/web-reverse-api.json`
- Resolution: `0` local present, `4` external legacy, `0` unclassified
- Legacy references:
  - `credentials.samplePath` -> `docs/20-ai-gateway/examples/credentials/suno/web_reverse_api/minimal.raw.sample.json` (`external_legacy_reference`; `path_not_present_in_gateway`)
  - `credentials.fieldsDocPath` -> `docs/20-ai-gateway/examples/credentials/suno/web_reverse_api/FIELDS.md` (`external_legacy_reference`; `path_not_present_in_gateway`)
  - `docs.overviewDocPath` -> `docs/20-ai-gateway/Suno平台实现线、可选编译与物理隔离基线.md` (`external_legacy_reference`; `path_not_present_in_gateway`)
  - `docs.buildDocPath` -> `docs/20-ai-gateway/examples/credentials/suno/web_reverse_api/BUILD.md` (`external_legacy_reference`; `path_not_present_in_gateway`)

<a id="line-tavily-search-official-vendor-api"></a>
### tavily-search-official-vendor-api

- Identity: `tavily_platform` / `tavily-search`
- Capabilities: `search`
- Material kinds: `api_key`
- Manifest: `manifests/lines/tavily/official-vendor-api.json`
- Resolution: `0` local present, `4` external legacy, `0` unclassified
- Legacy references:
  - `credentials.samplePath` -> `docs/20-ai-gateway/examples/credentials/tavily/official_vendor_api/minimal.raw.sample.json` (`external_legacy_reference`; `path_not_present_in_gateway`)
  - `credentials.fieldsDocPath` -> `docs/20-ai-gateway/examples/credentials/tavily/official_vendor_api/FIELDS.md` (`external_legacy_reference`; `path_not_present_in_gateway`)
  - `docs.overviewDocPath` -> `docs/20-ai-gateway/Tavily Search平台实现线、可选编译与物理隔离基线.md` (`external_legacy_reference`; `path_not_present_in_gateway`)
  - `docs.buildDocPath` -> `docs/20-ai-gateway/examples/credentials/tavily/official_vendor_api/BUILD.md` (`external_legacy_reference`; `path_not_present_in_gateway`)

<a id="line-together-openai-aggregator-api"></a>
### together-openai-aggregator-api

- Identity: `together_platform` / `together-openai`
- Capabilities: `conversation`, `models`
- Material kinds: `api_key`
- Manifest: `manifests/lines/together/aggregator-api.json`
- Resolution: `0` local present, `4` external legacy, `0` unclassified
- Legacy references:
  - `credentials.samplePath` -> `docs/20-ai-gateway/examples/credentials/together/aggregator_api/minimal.raw.sample.json` (`external_legacy_reference`; `path_not_present_in_gateway`)
  - `credentials.fieldsDocPath` -> `docs/20-ai-gateway/examples/credentials/together/aggregator_api/FIELDS.md` (`external_legacy_reference`; `path_not_present_in_gateway`)
  - `docs.overviewDocPath` -> `docs/20-ai-gateway/Together平台实现线、可选编译与物理隔离基线.md` (`external_legacy_reference`; `path_not_present_in_gateway`)
  - `docs.buildDocPath` -> `docs/20-ai-gateway/examples/credentials/together/aggregator_api/BUILD.md` (`external_legacy_reference`; `path_not_present_in_gateway`)

<a id="line-udio-web-reverse-api"></a>
### udio-web-reverse-api

- Identity: `udio_platform` / `udio`
- Capabilities: `models`, `images`, `music`, `videos`
- Material kinds: `session_auth`, `browser_state`
- Manifest: `manifests/lines/udio/web-reverse-api.json`
- Resolution: `0` local present, `4` external legacy, `0` unclassified
- Legacy references:
  - `credentials.samplePath` -> `docs/20-ai-gateway/examples/credentials/udio/web_reverse_api/minimal.raw.sample.json` (`external_legacy_reference`; `path_not_present_in_gateway`)
  - `credentials.fieldsDocPath` -> `docs/20-ai-gateway/examples/credentials/udio/web_reverse_api/FIELDS.md` (`external_legacy_reference`; `path_not_present_in_gateway`)
  - `docs.overviewDocPath` -> `docs/20-ai-gateway/Udio Platform实现线、可选编译与物理隔离基线.md` (`external_legacy_reference`; `path_not_present_in_gateway`)
  - `docs.buildDocPath` -> `docs/20-ai-gateway/examples/credentials/udio/web_reverse_api/BUILD.md` (`external_legacy_reference`; `path_not_present_in_gateway`)

<a id="line-websearchapi-search-official-vendor-api"></a>
### websearchapi-search-official-vendor-api

- Identity: `websearchapi_platform` / `websearchapi-search`
- Capabilities: `search`
- Material kinds: `api_key`
- Manifest: `manifests/lines/websearchapi/official-vendor-api.json`
- Resolution: `0` local present, `4` external legacy, `0` unclassified
- Legacy references:
  - `credentials.samplePath` -> `docs/20-ai-gateway/examples/credentials/websearchapi/official_vendor_api/minimal.raw.sample.json` (`external_legacy_reference`; `path_not_present_in_gateway`)
  - `credentials.fieldsDocPath` -> `docs/20-ai-gateway/examples/credentials/websearchapi/official_vendor_api/FIELDS.md` (`external_legacy_reference`; `path_not_present_in_gateway`)
  - `docs.overviewDocPath` -> `docs/20-ai-gateway/WebSearchAPI Search平台实现线、可选编译与物理隔离基线.md` (`external_legacy_reference`; `path_not_present_in_gateway`)
  - `docs.buildDocPath` -> `docs/20-ai-gateway/examples/credentials/websearchapi/official_vendor_api/BUILD.md` (`external_legacy_reference`; `path_not_present_in_gateway`)

<a id="line-xai-openai-official-vendor-api"></a>
### xai-openai-official-vendor-api

- Identity: `xai_platform` / `xai-openai`
- Capabilities: `conversation`, `models`
- Material kinds: `api_key`
- Manifest: `manifests/lines/xai/openai-official-vendor-api.json`
- Resolution: `0` local present, `4` external legacy, `0` unclassified
- Legacy references:
  - `credentials.samplePath` -> `docs/20-ai-gateway/examples/credentials/xai/openai_official_vendor_api/minimal.raw.sample.json` (`external_legacy_reference`; `path_not_present_in_gateway`)
  - `credentials.fieldsDocPath` -> `docs/20-ai-gateway/examples/credentials/xai/openai_official_vendor_api/FIELDS.md` (`external_legacy_reference`; `path_not_present_in_gateway`)
  - `docs.overviewDocPath` -> `docs/20-ai-gateway/xAI OpenAI-compatible平台实现线、可选编译与物理隔离基线.md` (`external_legacy_reference`; `path_not_present_in_gateway`)
  - `docs.buildDocPath` -> `docs/20-ai-gateway/examples/credentials/xai/openai_official_vendor_api/BUILD.md` (`external_legacy_reference`; `path_not_present_in_gateway`)

<a id="line-xfyun-native-websocket-official-vendor-api"></a>
### xfyun-native-websocket-official-vendor-api

- Identity: `xfyun_platform` / `xfyun-native-websocket`
- Capabilities: `conversation`
- Material kinds: `api_key`
- Manifest: `manifests/lines/xfyun/native-websocket-official-vendor-api.json`
- Resolution: `0` local present, `4` external legacy, `0` unclassified
- Legacy references:
  - `credentials.samplePath` -> `docs/20-ai-gateway/examples/credentials/xfyun/native_websocket_official_vendor_api/minimal.raw.sample.json` (`external_legacy_reference`; `path_not_present_in_gateway`)
  - `credentials.fieldsDocPath` -> `docs/20-ai-gateway/examples/credentials/xfyun/native_websocket_official_vendor_api/FIELDS.md` (`external_legacy_reference`; `path_not_present_in_gateway`)
  - `docs.overviewDocPath` -> `docs/20-ai-gateway/XFYun Native WebSocket平台实现线、可选编译与物理隔离基线.md` (`external_legacy_reference`; `path_not_present_in_gateway`)
  - `docs.buildDocPath` -> `docs/20-ai-gateway/examples/credentials/xfyun/native_websocket_official_vendor_api/BUILD.md` (`external_legacy_reference`; `path_not_present_in_gateway`)

<a id="line-xfyun-openai-official-vendor-api"></a>
### xfyun-openai-official-vendor-api

- Identity: `xfyun_platform` / `xfyun-openai`
- Capabilities: `conversation`, `models`
- Material kinds: `api_key`
- Manifest: `manifests/lines/xfyun/openai-official-vendor-api.json`
- Resolution: `0` local present, `4` external legacy, `0` unclassified
- Legacy references:
  - `credentials.samplePath` -> `docs/20-ai-gateway/examples/credentials/xfyun/openai_official_vendor_api/minimal.raw.sample.json` (`external_legacy_reference`; `path_not_present_in_gateway`)
  - `credentials.fieldsDocPath` -> `docs/20-ai-gateway/examples/credentials/xfyun/openai_official_vendor_api/FIELDS.md` (`external_legacy_reference`; `path_not_present_in_gateway`)
  - `docs.overviewDocPath` -> `docs/20-ai-gateway/XFYun OpenAI-compatible平台实现线、可选编译与物理隔离基线.md` (`external_legacy_reference`; `path_not_present_in_gateway`)
  - `docs.buildDocPath` -> `docs/20-ai-gateway/examples/credentials/xfyun/openai_official_vendor_api/BUILD.md` (`external_legacy_reference`; `path_not_present_in_gateway`)

<a id="line-you-search-official-vendor-api"></a>
### you-search-official-vendor-api

- Identity: `you_platform` / `you-search`
- Capabilities: `search`
- Material kinds: `api_key`
- Manifest: `manifests/lines/you/official-vendor-api.json`
- Resolution: `0` local present, `4` external legacy, `0` unclassified
- Legacy references:
  - `credentials.samplePath` -> `docs/20-ai-gateway/examples/credentials/you/official_vendor_api/minimal.raw.sample.json` (`external_legacy_reference`; `path_not_present_in_gateway`)
  - `credentials.fieldsDocPath` -> `docs/20-ai-gateway/examples/credentials/you/official_vendor_api/FIELDS.md` (`external_legacy_reference`; `path_not_present_in_gateway`)
  - `docs.overviewDocPath` -> `docs/20-ai-gateway/You.com Search平台实现线、可选编译与物理隔离基线.md` (`external_legacy_reference`; `path_not_present_in_gateway`)
  - `docs.buildDocPath` -> `docs/20-ai-gateway/examples/credentials/you/official_vendor_api/BUILD.md` (`external_legacy_reference`; `path_not_present_in_gateway`)
