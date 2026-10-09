import type { AccountDiscovery } from "./accountDiscovery";
import { useUiLocale } from "../../i18n/UiLocaleProvider";

const names: Record<AccountDiscovery["protocol"], string> = {
  chat_completions: "Chat Completions", responses: "Responses", messages: "Anthropic Messages",
  gemini_generate_content: "Gemini GenerateContent", gemini_interactions: "Gemini Interactions",
  ollama_chat: "Ollama Chat", ollama_generate: "Ollama Generate", cohere_chat: "Cohere Chat",
  bedrock_converse: "Bedrock Converse", completions: "Completions",
  dashscope_text: "DashScope Text", dashscope_multimodal: "DashScope Multimodal",
};

export function CredentialProtocolList({ discovery }: { discovery?: AccountDiscovery }) {
  const { t } = useUiLocale();
  const protocols = discovery?.protocols?.length ? discovery.protocols : discovery ? [discovery] : [];
  return <div className="nt-credential-protocol-list">
    {protocols.length ? protocols.map((entry) => <details key={entry.protocol}>
      <summary>{names[entry.protocol]} <span>{t(`${entry.verified_models.length} 个已验证模型`, `${entry.verified_models.length} verified models`)}</span></summary>
      <ul>{entry.verified_models.map((model) => <li key={model}>{model}</li>)}</ul>
    </details>) : <span>{t("暂无协议识别结果", "No discovered protocols")}</span>}
  </div>;
}
