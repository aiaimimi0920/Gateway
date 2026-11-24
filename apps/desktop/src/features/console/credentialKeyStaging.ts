// Recheck staged static Keys after raw JSON edits; previews omit unsafe values, commits reject them.
import type { ConsoleRouteDocument } from "../../api/contracts";
import type { CredentialSecretEdit } from "./credentialDocument";
import { validateQwenStaging } from "./qwenMaterialBinding";
import { validateKiroKeyStaging } from "./kiroKeyMaterialBinding";
import { validateVertexExpressStaging } from "./vertexExpress";
import { validateLumaAgentsStaging } from "./lumaAgents";
import { lumaInferenceTarget } from "./lumaInferenceCredential";
import { isRecord } from "./routeDocument";

export function validateCredentialKeyStaging(document: ConsoleRouteDocument, edit: CredentialSecretEdit, strict: boolean): boolean {
  const provider = document.providers.find(p => isRecord(p) && p.id === edit.providerId);
  const preset = isRecord(provider) ? provider.preset : undefined;
  const replacement = edit.operation === "replace" && edit.field === "api_key";
  try {
    if (edit.qwenBinding || replacement && preset === "qwen-official-api")
      validateQwenStaging(document, edit.providerId, edit.credentialId, edit.qwenBinding);
    if (edit.kiroKeyBinding || replacement && preset === "kiro-api-key")
      validateKiroKeyStaging(document, edit.providerId, edit.credentialId, edit.kiroKeyBinding);
    if (edit.vertexExpressBinding || replacement && preset === "vertex-express-api")
      validateVertexExpressStaging(document, edit.providerId, edit.credentialId, edit.vertexExpressBinding);
    if (edit.lumaAgentsBinding || replacement && preset === "lumalabs-api")
      validateLumaAgentsStaging(document, edit.providerId, edit.credentialId, edit.lumaAgentsBinding);
    if (edit.lumaInferenceBinding || replacement && preset === "lumalabs-inference-api") {
      const current = lumaInferenceTarget(document, edit.providerId, edit.credentialId);
      if (!edit.lumaInferenceBinding || current.product !== edit.lumaInferenceBinding.product
        || current.baseUrl !== edit.lumaInferenceBinding.baseUrl) throw new Error("Luma Inference Key 未绑定当前目标，请重新录入。");
    }
    return true;
  } catch (error) {
    if (strict) throw error;
    return false;
  }
}
