// Persist independent runtime policy alongside the backward-compatible transport projection.
import type { ProviderCatalogDraft, ProviderCatalogTemplate } from "./providerCatalog";
import { CALL_ADAPTERS, SPECIAL_ADAPTERS } from "./providerCatalogWorkflow";

export function writeProviderCallFlow(provider: Record<string, unknown>, template: ProviderCatalogTemplate, draft: ProviderCatalogDraft) {
  const workflow = draft.workflow;
  if (!workflow) return;
  const special = workflow.specialAdapter;
  const managed = special !== "none";
  provider.call_flow = {
    call_adapter: managed || !template.custom ? "auto" : workflow.callAdapter,
    fingerprint_adapter: managed ? "session" : draft.requestClientProfile ?? "auto",
    special_adapter: special === "none" && !template.custom ? "auto" : special,
  };
  if (template.custom) {
    const [, , adapter, profile] = CALL_ADAPTERS[workflow.callAdapter];
    provider.adapter = adapter;
    provider.protocol_profile = profile;
    const preset = SPECIAL_ADAPTERS[special][2];
    if (preset) { provider.preset = preset; delete provider.adapter; delete provider.protocol_profile; }
  }
  if (special === "basispoints") {
    provider.codex_basispoints_enabled = true;
    provider.protocol_profile = "chatgpt_codex_backend";
  } else delete provider.codex_basispoints_enabled;
  if (managed) delete provider.request_client_profile;
}
