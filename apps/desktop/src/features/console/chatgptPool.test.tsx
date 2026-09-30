import { render, screen, within } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { ProviderAccountLibrary } from "./ProviderAccountLibrary";
import { CHATGPT_POOL_CATEGORIES } from "./chatgptPool";
import { PROVIDER_CATALOG_TEMPLATES, providerDefinitionFromCatalogDraft } from "./providerCatalog";
import type { AccountsLedgerPilotSection } from "./accountsLedgerTypes";

describe("ChatGPT subscription pool", () => {
  it("persists the six independent subscription categories in the catalog definition", () => {
    const template = PROVIDER_CATALOG_TEMPLATES.find((entry) => entry.id === "chatgpt")!;
    const provider = providerDefinitionFromCatalogDraft(template, {
      templateId: template.id, providerId: "chatgpt", providerLabel: "ChatGPT",
      vendorKey: template.vendorKey, vendorName: template.vendorName, baseUrl: template.baseUrl,
      supportedModels: [...template.supportedModels], credentialId: "", accountName: "", apiKey: "",
    });
    expect(provider.credential_identity_categories).toEqual(CHATGPT_POOL_CATEGORIES);
    expect(provider.protocol_profile).toBe("chatgpt_codex_backend");
  });

  it("keeps empty subgroups visible without manufacturing accounts", () => {
    const section: AccountsLedgerPilotSection = {
      providerId: "chatgpt", providerIds: ["chatgpt"], providerLabel: "ChatGPT", vendorLabel: "ChatGPT",
      providerPreset: "chatgpt-codex-oauth-official-api", adapter: "openai_compatible",
      protocolProfile: "chatgpt_codex_backend", hostLabel: "chatgpt.com", defaultAccountId: null,
      manualAddFamily: null, hasExplicitAccounts: false, poolTargetSize: 30,
      autoRefillEnabled: false, autoPruneEnabled: false, permanentDeleteEnabled: false,
      supportsIdentityCategories: true, directAccounts: [],
      identityCategories: CHATGPT_POOL_CATEGORIES.map((category) => ({
        ...category, count: 0, accounts: [], poolTargetSize: 30, autoRefillEnabled: false, autoPruneEnabled: false,
      })),
    };
    const renderAccount = vi.fn();
    render(<ProviderAccountLibrary section={section} providerAccounts={[]} t={(_, en) => en}
      editorLocked={false} onOpenGeminiManualAdd={vi.fn()} onAddExplicit={vi.fn()} renderAccount={renderAccount} />);
    for (const category of CHATGPT_POOL_CATEGORIES) {
      const group = screen.getByRole("region", { name: `${category.label} account library` });
      expect(within(group).getByText("0 accounts")).toBeInTheDocument();
    }
    expect(renderAccount).not.toHaveBeenCalled();
  });
});
