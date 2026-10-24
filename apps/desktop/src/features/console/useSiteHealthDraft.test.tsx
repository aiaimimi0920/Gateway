import { act, renderHook } from "@testing-library/react";
import { expect, it, vi } from "vitest";
import { createConsoleApi } from "./BrowserConsoleApp.api-fixture";
import { accountGroupDraftRowsFromDocument } from "./accountGroupDraft";
import { useConsoleRouteDraft } from "./useConsoleRouteDraft";
import { parseRouteDocument } from "./routeDocument";
import { siteHealthConfigSchema } from "./siteHealth";

async function setup(hasSecretAccess = true) {
  const response = await createConsoleApi().getRouteConfig("management-secret");
  response.routeConfig.document = parseRouteDocument(
    JSON.stringify({
      providers: [
        { id: "site", base_url: "https://fixture.invalid", site_health: {} },
      ],
      model_routes: [],
      aliases: {},
    }),
  );
  response.routeConfig.secrets = [];
  const setError = vi.fn();
  const setSecretDialogOpen = vi.fn();
  const props = {
    routeConfig: response,
    hasSecretAccess,
    t: (zh: string) => zh,
    invalidateCredentialProbes: vi.fn(),
    accountGroupDraftRowsFromDocument,
    onRouteConfigHydrated: vi.fn(),
    setError,
    setSecretDialogOpen,
  };
  const view = renderHook((options) => useConsoleRouteDraft(options), {
    initialProps: props,
  });
  return { ...view, props, setError, setSecretDialogOpen };
}
const config = () =>
  siteHealthConfigSchema.parse({
    profiles: [
      {
        id: "plus",
        name: "Plus",
        policy: { adapter: "sub2api-v2", auth: { kind: "bearer" } },
      },
    ],
  });

it("builds an authorized secret patch without ever putting the token in editorText", async () => {
  const { result } = await setup();
  act(() =>
    result.current.applySiteHealth("site", config(), [
      { profileId: "plus", operation: "replace", value: "fixture-token" },
    ]),
  );
  expect(result.current.editorText).not.toContain("fixture-token");
  expect(result.current.draftDirty).toBe(true);
  const request = result.current.parseDraft();
  expect(JSON.stringify(request.document)).not.toContain("fixture-token");
  expect(request.secretPatches).toContainEqual({
    path: "/providers/0/site_health/profiles/0/policy/auth/token",
    operation: "replace",
    value: "fixture-token",
  });
});

it("requests authorization before changing the document and drops secrets when access expires", async () => {
  const { result, props, rerender, setSecretDialogOpen, setError } =
    await setup(false);
  const text = result.current.editorText;
  act(() =>
    result.current.applySiteHealth("site", config(), [
      { profileId: "plus", operation: "replace", value: "fixture-token" },
    ]),
  );
  expect(setSecretDialogOpen).toHaveBeenCalledWith(true);
  expect(setError).toHaveBeenCalledWith(
    expect.stringContaining("敏感信息访问权限"),
  );
  expect(result.current.editorText).toBe(text);
  rerender({ ...props, hasSecretAccess: true });
  act(() =>
    result.current.applySiteHealth("site", config(), [
      { profileId: "plus", operation: "replace", value: "fixture-token" },
    ]),
  );
  expect(result.current.siteHealthSecretEdits).toHaveLength(1);
  rerender({ ...props, hasSecretAccess: false });
  expect(result.current.siteHealthSecretEdits).toHaveLength(0);
});

it("rejects raw-editor destination mutations after a token is staged", async () => {
  const { result } = await setup();
  act(() =>
    result.current.applySiteHealth("site", config(), [
      { profileId: "plus", operation: "replace", value: "fixture-token" },
    ]),
  );
  act(() =>
    result.current.setEditorText(
      result.current.editorText.replace(
        "https://fixture.invalid",
        "https://changed.invalid",
      ),
    ),
  );
  expect(() => result.current.parseDraft()).toThrow("监控目标或认证已改变");
  expect(
    result.current.secretPatches.some((p) => p.operation === "replace"),
  ).toBe(false);
});

it.each(["replace", "clear"] as const)(
  "explicit Keep cancels a pending %s instead of silently applying it",
  async (operation) => {
    const { result } = await setup();
    act(() =>
      result.current.applySiteHealth("site", config(), [
        operation === "replace"
          ? { profileId: "plus", operation, value: "fixture-token" }
          : { profileId: "plus", operation },
      ]),
    );
    expect(result.current.siteHealthSecretEdits).toHaveLength(1);
    act(() =>
      result.current.applySiteHealth("site", config(), [
        { profileId: "plus", operation: "keep" },
      ]),
    );
    expect(result.current.siteHealthSecretEdits).toHaveLength(0);
    expect(
      result.current
        .parseDraft()
        .secretPatches.some((p) => p.operation !== "keep"),
    ).toBe(false);
  },
);
