import { screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { expect, vi } from "vitest";
import type { ConsoleApi } from "../../api/console";
import type {
  ConsoleAccessCatalog,
  ConsoleAccessKey,
} from "../../api/contracts";
import { AccessKeysSection } from "./AccessKeysSection";
import { useAccessData } from "./useAccessData";

const t = (zh: string) => zh;
export const key = (
  id: string,
  patch: Partial<ConsoleAccessKey> = {},
): ConsoleAccessKey => ({
  id,
  ownerType: "user",
  ownerId: "local",
  resolvedProjectId: "local",
  resolvedTenantId: "local",
  keyKind: "normal",
  status: "active",
  publicKeyPrefix: "sk-gw",
  displayName: id,
  token: null,
  externalKey: null,
  rotatedFromAccessKeyId: null,
  legacyGatewayApiKeyId: null,
  legacyUserCredentialId: null,
  expiresAt: null,
  lastUsedAt: null,
  metadata: null,
  revokedAt: null,
  revokeReason: null,
  createdAt: "2026-10-01T00:00:00Z",
  updatedAt: "2026-10-01T00:00:00Z",
  ...patch,
});

export function fixture(
  keys: ConsoleAccessKey[] = [],
  storageMode: "local" | "server" | undefined = "local",
) {
  const catalog: ConsoleAccessCatalog = {
    storageMode,
    accountGroups: [
      { id: "group-a", name: "基础组", enabled: true, memberCount: 2 },
      { id: "group-b", name: "高级组", enabled: true, memberCount: 3 },
      { id: "group-disabled", name: "停用组", enabled: false, memberCount: 1 },
    ],
    accessKeys: keys,
    balances: [],
    bundles: [],
    bundleItems: [],
    providerCapabilities: [],
    platformAccessRows: [],
    keyBundleBindings: [],
    aggregateMemberships: [],
  };
  const api = {
    getAccessCatalog: vi.fn(async () => ({
      ...catalog,
      accessKeys: [...catalog.accessKeys],
    })),
    createAccessKey: vi.fn(async (_token, input) => {
      const created = key("created-key", input);
      catalog.accessKeys.push(created);
      return { ...created, token: "sk-gw-fixture-secret" };
    }),
    updateAccessKey: vi.fn(async (_token, id, input) => {
      const updated = key(id, input);
      catalog.accessKeys = catalog.accessKeys.map((entry) =>
        entry.id === id ? updated : entry,
      );
      return updated;
    }),
    copyAccessKey: vi.fn(async () => ({ token: "sk-gw-fixture-secret" })),
    rotateAccessKey: vi.fn(async () => ({
      ...key("replacement"),
      token: "sk-gw-rotated-secret",
    })),
    setAccessKeyEnabled: vi.fn(async (_token, id, enabled) => {
      catalog.accessKeys = catalog.accessKeys.map((entry) =>
        entry.id === id
          ? { ...entry, status: enabled ? "active" : "disabled" }
          : entry,
      );
      return { success: true };
    }),
    deleteAccessKey: vi.fn(async (_token, id) => {
      catalog.accessKeys = catalog.accessKeys.filter(
        (entry) => entry.id !== id,
      );
      return { accessKeyId: id, displayName: id };
    }),
  } as unknown as ConsoleApi;
  return { api, catalog };
}

export function Harness({
  api,
  active = true,
  locked = false,
}: {
  api: ConsoleApi;
  active?: boolean;
  locked?: boolean;
}) {
  const access = useAccessData({
    api,
    active,
    managementToken: "management-fixture",
    t,
  });
  return (
    <AccessKeysSection
      t={t}
      catalog={access.catalog}
      editorLocked={locked}
      refreshing={access.refreshing}
      onRefresh={access.refresh}
      keyDraft={access.keyDraft}
      onKeyDraftChange={access.setKeyDraft}
      onCreateKey={access.createKey}
      creatingKey={access.creatingKey}
      keyBusyId={access.keyBusyId}
      onRotateKey={access.rotateKey}
      onKeyLifecycle={access.keyLifecycle}
      onUpdateKey={access.updateKey}
      onCopyKey={access.copyKey}
      revealedSecret={access.revealedSecret}
      onDismissSecret={access.dismissSecret}
    />
  );
}

export async function openCreate(user: ReturnType<typeof userEvent.setup>) {
  const trigger = screen.getByRole("button", { name: "新建 API Key" });
  await waitFor(() => expect(trigger).toBeEnabled());
  await user.click(trigger);
  await user.click(screen.getByRole("checkbox", { name: /基础组/ }));
  return screen.getByRole("dialog", { name: "新建 API Key" });
}
