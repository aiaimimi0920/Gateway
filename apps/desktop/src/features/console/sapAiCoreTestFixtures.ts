// Synthetic public-target fixtures; no network request or real service credential.
import type { CredentialDialogValue } from "./CredentialDialog";
export const sapBase = "https://api.ai.prod.eu-central-1.aws.ml.hana.ondemand.com/v2/inference/deployments/deployment1";
export const sapKey = JSON.stringify({ clientid: "synthetic-client", clientsecret: "synthetic-secret",
  url: "https://synthetic.authentication.eu10.hana.ondemand.com",
  serviceurls: { AI_API_URL: new URL(sapBase).origin } });
export const sapAzureBase = "https://api.ai.prod-eu20.westeurope.azure.ml.hana.ondemand.com/v2/inference/deployments/deployment1";
export const sapAzureKey = JSON.stringify({ clientid: "synthetic-client", clientsecret: "synthetic-secret",
  url: "https://synthetic.authentication.eu20.hana.ondemand.com",
  serviceurls: { AI_API_URL: `${new URL(sapAzureBase).origin}/v2` } });
export const sapProvider = { id: "sap", preset: "sap-aicore-chat", base_url: sapBase,
  headers: { "AI-Resource-Group": "group-a" }, extra_body: { sap_api_version: "2024-10-21" },
  explicit_credentials_only: true, credentials: [] };
export const sapDraft: CredentialDialogValue = { providerId: "sap", credentialId: "stable", accountName: "", enabled: true,
  baseUrl: "", supportedModelsText: "", manualModelsText: "", fetchedModelsText: "",
  apiKeyOperation: "replace", apiKeyValue: sapKey };
