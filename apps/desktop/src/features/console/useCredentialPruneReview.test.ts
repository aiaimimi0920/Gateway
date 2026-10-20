import { act, renderHook } from "@testing-library/react";
import { expect, it, vi } from "vitest";
import { createConsoleApi } from "./BrowserConsoleApp.api-fixture";
import { useCredentialPruneReview } from "./useCredentialPruneReview";
import type { CredentialPrunePreview, CredentialPruneReceipt } from "../../api/schemas/credential-prune";

const preview: CredentialPrunePreview = { providerId: "p", revision: "r1", digest: "digest", archiveRequired: true,
  items: [{ credentialId: "a", classification: "account_deleted" }] };
const receipt: CredentialPruneReceipt = { providerId: "p", sourceRevision: "r1", revision: "r2", outcome: "completed",
  removedCount: 1, items: [{ credentialId: "a", status: "archived_and_removed" }] };
function setup() {
  const api = createConsoleApi();
  vi.mocked(api.previewCredentialPrune).mockResolvedValue({ preview });
  vi.mocked(api.applyCredentialPrune).mockResolvedValue({ receipt });
  const options = { api, managementToken: "token", draftDirty: false, refresh: vi.fn(async () => {}), t: (_zh: string, en: string) => en };
  return { api, options, ...renderHook(props => useCredentialPruneReview(props), { initialProps: options }) };
}

it("opens without applying, binds selection to revision/digest and never auto-replays", async () => {
  const h = setup();
  await act(() => h.result.current.handlePruneCredentialPool("p"));
  expect(h.api.applyCredentialPrune).not.toHaveBeenCalled();
  await act(() => h.result.current.applyCredentialPruneReview(["foreign"]));
  expect(h.api.applyCredentialPrune).not.toHaveBeenCalled();
  await act(() => h.result.current.applyCredentialPruneReview(["a"]));
  expect(h.api.applyCredentialPrune).toHaveBeenCalledExactlyOnceWith("token", "p", { expectedRevision: "r1", previewDigest: "digest", credentialIds: ["a"] });
  expect(h.options.refresh).toHaveBeenCalledOnce();
  await act(() => h.result.current.applyCredentialPruneReview(["a"]));
  expect(h.api.applyCredentialPrune).toHaveBeenCalledOnce();
});

it.each(["token", "api", "unmount", "close"] as const)("ignores late preview after %s", async kind => {
  const h = setup();
  let resolve!: (value: { preview: CredentialPrunePreview }) => void;
  vi.mocked(h.api.previewCredentialPrune).mockReturnValueOnce(new Promise(yes => { resolve = yes; }));
  let pending!: Promise<void>;
  act(() => { pending = h.result.current.handlePruneCredentialPool("p"); });
  if (kind === "unmount") h.unmount();
  else if (kind === "close") act(() => h.result.current.closeCredentialPruneReview());
  else h.rerender({ ...h.options, ...(kind === "api" ? { api: createConsoleApi() } : { managementToken: "other" }) });
  await act(async () => { resolve({ preview }); await pending; });
  if (kind !== "unmount") expect(h.result.current.credentialPruneReview).toBeNull();
  expect(h.api.applyCredentialPrune).not.toHaveBeenCalled();
});

it("rejects retained apply callbacks after dirty draft or identity changes", async () => {
  const h = setup();
  await act(() => h.result.current.handlePruneCredentialPool("p"));
  const apply = h.result.current.applyCredentialPruneReview;
  h.rerender({ ...h.options, draftDirty: true });
  await act(() => apply(["a"]));
  expect(h.api.applyCredentialPrune).not.toHaveBeenCalled();
  h.rerender({ ...h.options, managementToken: "other" });
  await act(() => apply(["a"]));
  expect(h.api.applyCredentialPrune).not.toHaveBeenCalled();
});

it("disables retry after lost apply response and refreshes uncertain state", async () => {
  const h = setup();
  vi.mocked(h.api.applyCredentialPrune).mockRejectedValueOnce(new Error("response lost"));
  await act(() => h.result.current.handlePruneCredentialPool("p"));
  await act(() => h.result.current.applyCredentialPruneReview(["a"]));
  expect(h.result.current.credentialPruneReview?.error).toContain("do not retry blindly");
  await act(() => h.result.current.applyCredentialPruneReview(["a"]));
  expect(h.api.applyCredentialPrune).toHaveBeenCalledOnce();
  expect(h.options.refresh).toHaveBeenCalledOnce();
});

it("rejects a retained apply callback after closing or replacing its preview", async () => {
  const h = setup();
  await act(() => h.result.current.handlePruneCredentialPool("p"));
  const apply = h.result.current.applyCredentialPruneReview;
  await act(async () => {
    h.result.current.closeCredentialPruneReview();
    await apply(["a"]);
  });
  expect(h.api.applyCredentialPrune).not.toHaveBeenCalled();
  await act(() => h.result.current.handlePruneCredentialPool("p"));
  await act(() => apply(["a"]));
  expect(h.api.applyCredentialPrune).not.toHaveBeenCalled();
});

it.each(["token", "api", "unmount"] as const)("ignores a late receipt after %s and deduplicates pending apply", async kind => {
  const h = setup();
  let resolve!: (value: { receipt: CredentialPruneReceipt }) => void;
  vi.mocked(h.api.applyCredentialPrune).mockReturnValueOnce(new Promise(yes => { resolve = yes; }));
  await act(() => h.result.current.handlePruneCredentialPool("p"));
  const apply = h.result.current.applyCredentialPruneReview;
  let pending!: Promise<void>;
  await act(async () => { pending = apply(["a"]); await apply(["a"]); });
  expect(h.api.applyCredentialPrune).toHaveBeenCalledOnce();
  act(() => h.result.current.closeCredentialPruneReview());
  expect(h.result.current.credentialPruneReview?.applying).toBe(true);
  if (kind === "unmount") h.unmount();
  else h.rerender({ ...h.options, ...(kind === "api" ? { api: createConsoleApi() } : { managementToken: "other" }) });
  await act(async () => { resolve({ receipt }); await pending; });
  if (kind !== "unmount") expect(h.result.current.credentialPruneReview).toBeNull();
  expect(h.options.refresh).not.toHaveBeenCalled();
});
