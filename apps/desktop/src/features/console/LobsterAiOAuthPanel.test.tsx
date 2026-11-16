import { act, fireEvent, render, screen } from "@testing-library/react";
import { NeuroProvider } from "@neuro/ui";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { LobsterAiOAuthPanel, type LobsterAiOAuthOperations } from "./LobsterAiOAuthPanel";
import { lobsterSession, authorization } from "./lobsteraiTestFixtures";
vi.mock("../../i18n/UiLocaleProvider", () => ({ useUiLocale: () => ({ t: (zh: string) => zh }) }));
const api = vi.hoisted(() => ({ create: vi.fn(), get: vi.fn(), act: vi.fn() }));
vi.mock("../../api/console/lobsterai-auth", () => ({ createLobsterAiAuthApi: () => api }));
const props = () => ({ client: { request: vi.fn() }, managementToken: "owner", secretGrant: "grant", providerId: "renamed", expectedRevision: "r1",
  operations: new Map() as LobsterAiOAuthOperations, disabled: false, onRequestSecretAccess: vi.fn(), onSaved: vi.fn() });
beforeEach(() => { vi.useFakeTimers(); vi.resetAllMocks(); api.create.mockResolvedValue(lobsterSession("waiting_user")); api.get.mockResolvedValue(lobsterSession("ready")); api.act.mockResolvedValue(lobsterSession("cancelled")); });
afterEach(() => vi.useRealTimers());
const click = async (name: string) => { await act(async () => { fireEvent.click(screen.getByRole("button", { name })); }); };
it("opens a browser session without borrowed registration and saves only on explicit action", async () => {
  const p = props(); render(<LobsterAiOAuthPanel {...p} />, { wrapper: NeuroProvider });
  await click("通过 LobsterAI 登录");
  expect(api.create).toHaveBeenCalledWith({ providerId: "renamed", expectedRevision: "r1", credentialId: undefined }, "grant");
  expect(screen.getByRole("link")).toHaveAttribute("href", authorization);
  await act(async () => { await vi.advanceTimersByTimeAsync(1500); });
  expect(api.act).not.toHaveBeenCalled(); expect(p.onSaved).not.toHaveBeenCalled();
  api.act.mockResolvedValueOnce(lobsterSession("succeeded")); await click("读取目录并保存账号");
  expect(api.act).toHaveBeenCalledWith("login", "import", expect.objectContaining({ expectedRevision: "r1" }), "grant", undefined);
  expect(p.onSaved).toHaveBeenCalledOnce();
});
it("keeps an uncertain save across dialog reopen without duplicate import", async () => {
  const p = props(); api.create.mockResolvedValue(lobsterSession("ready"));
  const view = render(<LobsterAiOAuthPanel {...p} />, { wrapper: NeuroProvider }); await click("通过 LobsterAI 登录");
  api.act.mockRejectedValueOnce(new Error("lost response")); await click("读取目录并保存账号"); view.unmount();
  render(<LobsterAiOAuthPanel {...p} />, { wrapper: NeuroProvider });
  expect(screen.queryByRole("button", { name: "读取目录并保存账号" })).not.toBeInTheDocument();
  await act(async () => { await vi.advanceTimersByTimeAsync(1500); });
  expect(api.act.mock.calls.filter((call) => call[1] === "import")).toHaveLength(1);
});
it("blocks stale saves and does not replace a stable account with manual key input", async () => {
  const p = props(); api.create.mockResolvedValue(lobsterSession("ready"));
  const view = render(<LobsterAiOAuthPanel {...p} credentialId="stable" />, { wrapper: NeuroProvider }); await click("通过 LobsterAI 登录");
  expect(api.create.mock.calls[0][0].credentialId).toBe("stable");
  view.rerender(<LobsterAiOAuthPanel {...p} credentialId="stable" expectedRevision="r2" />);
  expect(screen.getByRole("button", { name: "读取目录并保存账号" })).toBeDisabled(); expect(api.act).not.toHaveBeenCalled();
});
it("requires a saved draft and secret consent before authorization", async () => {
  const p = props(); const view = render(<LobsterAiOAuthPanel {...p} disabled />, { wrapper: NeuroProvider });
  expect(screen.getByRole("button", { name: "通过 LobsterAI 登录" })).toBeDisabled();
  view.rerender(<LobsterAiOAuthPanel {...p} secretGrant={null} />); await click("通过 LobsterAI 登录");
  expect(p.onRequestSecretAccess).toHaveBeenCalledOnce(); expect(api.create).not.toHaveBeenCalled();
});
