import type { ConsoleRouteDocument } from "../../api/contracts";
import { updateExplicitCredential, type CredentialSecretEdit } from "./credentialDocument";
import { awsMaterialField, isBedrockProvider, record, validateBedrockMaterial, type BedrockMaterial } from "./bedrockMaterial";
import { bedrockMaterialTarget } from "./bedrockMaterialBinding";

export function applyBedrockMaterial(document: ConsoleRouteDocument, providerId: string, credentialId: string, material: BedrockMaterial) {
  const baseUrl = validateBedrockMaterial(material);
  const provider = document.providers.map(record).find((p) => p.id === providerId);
  const credential = (Array.isArray(provider?.credentials) ? provider.credentials : []).map(record).find((c) => c.id === credentialId);
  if (!provider || !credential || !isBedrockProvider(provider)) throw new Error("找不到相同产品下的 Bedrock 账号。");
  const headers = Object.fromEntries(Object.entries(record(credential.headers)).filter(([key]) => !["authorization", "cookie"].includes(key.toLowerCase())));
  const extra = Object.fromEntries(Object.entries(record(credential.extra_body)).filter(([key]) => !awsMaterialField(key)));
  // Runtime merges maps by exact key, not by AWS alias family. Override every inherited region key.
  const regions = Object.fromEntries(Object.keys(record(provider.extra_body)).filter((key) => awsMaterialField(key) === "region").map((key) => [key, material.region]));
  const next = updateExplicitCredential(document, { providerId, credentialId }, {
    auth_mode: material.authMode, authMode: undefined, base_url: baseUrl, api_key: undefined, auth_token: undefined, refresh_token: undefined,
    headers, extra_body: { ...extra, ...regions, awsRegion: material.region }, session_auth: undefined, expires_at: undefined,
    refresh_endpoint: undefined, refresh_client_id: undefined, token_expires_in_secs: undefined,
  });
  const { binding } = bedrockMaterialTarget(next, providerId, credentialId);
  const values: [CredentialSecretEdit["field"], string][] = [
    ["api_key", material.apiKey], ["extra_body/awsAccessKeyId", material.accessKeyId],
    ["extra_body/awsSecretAccessKey", material.secretAccessKey], ["extra_body/awsSessionToken", material.sessionToken],
    ["auth_token", ""], ["refresh_token", ""], ["headers/Authorization", ""], ["headers/Cookie", ""],
  ];
  const secretEdits: CredentialSecretEdit[] = values.map(([field, value]) => value
    ? { providerId, credentialId, field, operation: "replace", value, bedrockBinding: binding }
    : { providerId, credentialId, field, operation: "clear", bedrockBinding: binding });
  return { document: next, secretEdits };
}
