import { expect, it } from "vitest";
import { officialCatalogSchema, applyOfficialCatalog, catalogProviderKind } from "./officialModelCatalog";
import { credentialDialogValueFromDocument, duplicateCredentialDialogValue } from "./providerCredentialDraft";
import { routeDocument } from "./credentialDocumentTestFixtures";

const base = "https://workspace-a.cn-beijing.maas.aliyuncs.com/compatible-mode/v1";
const catalog = (accountBaseUrl = base, sourceUrl = accountBaseUrl.replace("/compatible-mode/v1", "/api/v1/models")) => ({
  accountBaseUrl, sourceUrl, checkedAt: "2026-11-02T00:00:00Z", models: ["fetched"],
});
const document = () => routeDocument([{ id: "arbitrary", preset: "qwen-official-api", base_url: base,
  supported_models: ["fetched", "manual"], credentials: [{ id: "a", supported_models: ["fetched", "manual"], model_catalog: catalog() }] }]);

it.each(["cn-beijing", "ap-southeast-1", "ap-northeast-1", "eu-central-1", "us-east-1", "cn-hongkong"])(
  "accepts the exact native source and workspace binding for %s", (region) => {
    const target = base.replace("cn-beijing", region);
    const source = region === "ap-southeast-1" ? "https://dashscope-intl.aliyuncs.com/api/v1/models"
      : region === "cn-hongkong" ? "https://cn-hongkong.dashscope.aliyuncs.com/api/v1/models"
        : target.replace("/compatible-mode/v1", "/api/v1/models");
    expect(officialCatalogSchema.parse(catalog(target, source))).toEqual(catalog(target, source));
  });

it("rejects missing, noncanonical or mismatched dynamic account binding and fixed-source laundering", () => {
  for (const value of [
    { ...catalog(), accountBaseUrl: undefined }, { ...catalog(), accountBaseUrl: `${base}/` },
    catalog(base.replace("workspace-a", "workspace-b"), catalog().sourceUrl),
    catalog(base, "https://api.openai.com/v1/models"),
    { ...catalog(), sourceUrl: "https://api.openai.com/v1/models", accountBaseUrl: "https://evil.invalid" },
  ]) expect(officialCatalogSchema.safeParse(value).success).toBe(false);
});

it("reads a legacy native catalog as fetched, never as manual, and does not duplicate its evidence", () => {
  const doc = document(); const value = credentialDialogValueFromDocument(doc, "arbitrary", "a")!;
  expect(value.modelCatalog).toEqual(catalog());
  expect(value.fetchedModelsText).toBe("fetched"); expect(value.manualModelsText).toBe("manual");
  const copy = duplicateCredentialDialogValue(doc, "arbitrary", { accountId: "a", enabled: true } as never);
  expect(copy.supportedModelsText).toBe("manual"); expect(copy.modelCatalog).toBeUndefined();
});

it("applies a native catalog only to the exact ordinary product and effective account target", () => {
  const doc = document();
  expect(catalogProviderKind(doc.providers[0] as Record<string, unknown>)).toBe("qwen");
  const next = applyOfficialCatalog(doc, "arbitrary", "a", catalog());
  expect((next.providers[0] as Record<string, unknown>).credentials).toEqual(expect.arrayContaining([
    expect.objectContaining({ model_catalog: catalog(), supported_models: ["fetched", "manual"] }),
  ]));
  const wrong = document(); (wrong.providers[0] as Record<string, unknown>).preset = "qwen-web";
  expect(() => applyOfficialCatalog(wrong, "arbitrary", "a", catalog())).toThrow();
  const override = document(); ((override.providers[0] as Record<string, unknown>).credentials as Record<string, unknown>[])[0].base_url = base.replace("workspace-a", "workspace-b");
  expect(() => applyOfficialCatalog(override, "arbitrary", "a", catalog())).toThrow();
});

it("refuses to open a mismatched native catalog rather than silently washing it into manual models", () => {
  const doc = document(); ((doc.providers[0] as Record<string, unknown>).credentials as Record<string, unknown>[])[0].base_url = base.replace("workspace-a", "workspace-b");
  expect(credentialDialogValueFromDocument(doc, "arbitrary", "a")).toBeNull();
});
it.each(["ap-southeast-1", "cn-hongkong"])("does not confuse shared catalog sources across workspaces in %s", (region) => {
  const target = base.replace("cn-beijing", region);
  const source = region === "ap-southeast-1" ? "https://dashscope-intl.aliyuncs.com/api/v1/models" : "https://cn-hongkong.dashscope.aliyuncs.com/api/v1/models";
  const doc = document(); (doc.providers[0] as Record<string, unknown>).base_url = target;
  expect(() => applyOfficialCatalog(doc, "arbitrary", "a", catalog(target.replace("workspace-a", "workspace-b"), source))).toThrow();
});
