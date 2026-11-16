import { expect, it } from "vitest";
import { isLumaMcpProvider, savedLumaRegistration } from "./lumaMcp";
import { providerAccessKind } from "./providerAccessKind";
const provider = { adapter: "lumalabs_mcp_readonly", protocol_profile: "lumalabs_mcp_readonly", credentials: [
  { id: "stable", luma_oauth: { client_id: "registered", redirect_uri: "http://127.0.0.1:43567/callback" } },
] };
it("selects MCP by explicit metadata, not a label or shared Luma hostname", () => {
  expect(isLumaMcpProvider(provider)).toBe(true);
  for (const value of [{ id: "lumalabs_mcp_readonly" }, { preset: "lumalabs-api" }, { preset: "lumalabs" }, null]) expect(isLumaMcpProvider(value)).toBe(false);
  expect(providerAccessKind({ adapter: provider.adapter, protocolProfile: provider.protocol_profile, authMode: "luma_mcp_oauth" })).toBe("oauth");
  expect(providerAccessKind({ providerPreset: "lumalabs-api" })).toBe("api");
});
it("reads only the selected account's registration without borrowing another account", () => {
  expect(savedLumaRegistration(provider, "stable")).toEqual({ clientId: "registered", redirectUri: "http://127.0.0.1:43567/callback" });
  expect(savedLumaRegistration(provider, "missing")).toBeUndefined();
  expect(savedLumaRegistration(provider)).toBeUndefined();
  expect(savedLumaRegistration({ ...provider, credentials: [{ id: "stable", luma_oauth: { client_id: "registered", redirect_uri: "https://evil.invalid/callback" } }] }, "stable")).toBeUndefined();
});
