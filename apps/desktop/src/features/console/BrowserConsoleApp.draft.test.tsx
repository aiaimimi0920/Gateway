import {
  act,
  fireEvent,
  screen,
  waitFor,
  within,
} from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import {
  beforeEach,
  describe,
  expect,
  it,
  vi,
} from "vitest";
import { BrowserConsoleApp } from "./BrowserConsoleApp";
import { createConsoleApi } from "./BrowserConsoleApp.api-fixture";
import {
  deferred,
  renderWithProviders,
  waitForConsoleReady,
  openWorkspace,
  waitForCommittedRouteDraft,
} from "./BrowserConsoleApp.render-fixture";

describe("BrowserConsoleApp", () => {
  beforeEach(() => {
    window.localStorage.clear();
  });

  it("refreshes draft state even when the active revision ID is unchanged", async () => {
    const consoleApi = createConsoleApi();
    const user = userEvent.setup();
    const activeResponse = await consoleApi.getRouteConfig("management-secret");
    vi.mocked(consoleApi.getRouteConfig).mockClear();
    vi.mocked(consoleApi.getRouteConfig).mockResolvedValue(activeResponse);

    renderWithProviders(<BrowserConsoleApp consoleApi={consoleApi} />);

    await waitForConsoleReady();
    await openWorkspace(user, /权益组/i);
    await user.click(screen.getByRole("button", { name: /添加分组/i }));
    await user.click(screen.getByRole("button", { name: /^新分组$/i, expanded: false }));
    await user.type(screen.getByLabelText(/分组 ID/i), "temporary-group");
    expect(screen.getByLabelText(/分组 ID/i)).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: /^刷新$/i }));

    await waitFor(() => {
      expect(screen.queryByLabelText(/分组 ID/i)).not.toBeInTheDocument();
    });
    expect(consoleApi.getRouteConfig).toHaveBeenCalledTimes(2);
    await act(async () => { await new Promise((resolve) => setTimeout(resolve, 1300)); });
    expect(consoleApi.commitRouteConfig).not.toHaveBeenCalled();
  });

  it("registers a beforeunload guard only while the route draft is dirty", async () => {
    const consoleApi = createConsoleApi();
    const user = userEvent.setup();

    renderWithProviders(<BrowserConsoleApp consoleApi={consoleApi} />);

    await waitForConsoleReady();
    const cleanEvent = new Event("beforeunload", { cancelable: true });
    expect(window.dispatchEvent(cleanEvent)).toBe(true);
    expect(cleanEvent.defaultPrevented).toBe(false);

    await openWorkspace(user, /权益组/i);
    await user.click(screen.getByRole("button", { name: /添加分组/i }));
    await user.click(screen.getByRole("button", { name: /^新分组$/i, expanded: false }));
    await user.type(screen.getByLabelText(/分组 ID/i), "temporary-group");

    const dirtyEvent = new Event("beforeunload", { cancelable: true });
    expect(window.dispatchEvent(dirtyEvent)).toBe(false);
    expect(dirtyEvent.defaultPrevented).toBe(true);
  });

  it("does not render the interface-language control in the authenticated console sidebar", async () => {
    const consoleApi = createConsoleApi();
    const user = userEvent.setup();

    renderWithProviders(<BrowserConsoleApp consoleApi={consoleApi} />);

    await waitForConsoleReady();
    expect(screen.queryByRole("button", { name: /切换界面语言/i })).not.toBeInTheDocument();
    expect(document.querySelector(".nt-rail .nt-rail__footer")).toBeNull();
  });

  it("clears active repair diagnostics when refresh replaces the authoritative route", async () => {
    const consoleApi = createConsoleApi();
    const user = userEvent.setup();
    const cleanResponse = await consoleApi.getRouteConfig("management-secret");
    vi.mocked(consoleApi.getRouteConfig).mockResolvedValue({
      routeConfig: {
        ...cleanResponse.routeConfig,
        requiresRepair: true,
        diagnostics: { diagnostics: [{
          code: "secret_document_identity_invalid", severity: "error",
          path: "/providers/0/id", message: "active identity needs repair",
        }] },
      },
    });

    renderWithProviders(<BrowserConsoleApp consoleApi={consoleApi} />);

    await waitForConsoleReady();
    expect(screen.getByRole("alert", { name: /active route diagnostics/i })).toHaveTextContent(
      "active identity needs repair",
    );
    vi.mocked(consoleApi.getRouteConfig).mockResolvedValue(cleanResponse);

    await user.click(screen.getByRole("button", { name: /^刷新$/i }));

    await waitFor(() =>
      expect(screen.queryByRole("alert", { name: /active route diagnostics/i })).not.toBeInTheDocument(),
    );
  });

  it("disables manual refresh while an authoritative refresh is in flight", async () => {
    const consoleApi = createConsoleApi();
    const user = userEvent.setup();
    const initialResponse = await consoleApi.getRouteConfig("management-secret");
    const pendingRefresh = deferred<typeof initialResponse>();
    vi.mocked(consoleApi.getRouteConfig).mockReset();
    vi.mocked(consoleApi.getRouteConfig)
      .mockResolvedValueOnce(initialResponse)
      .mockReturnValueOnce(pendingRefresh.promise);

    renderWithProviders(<BrowserConsoleApp consoleApi={consoleApi} />);

    await waitForConsoleReady();
    const refreshButton = screen.getByRole("button", { name: /^刷新$/i });
    await user.click(refreshButton);

    expect(refreshButton).toBeDisabled();
    pendingRefresh.resolve(JSON.parse(JSON.stringify(initialResponse)));
    await waitFor(() => expect(refreshButton).toBeEnabled());
  });

  it("coalesces overlapping authoritative refreshes for the same console session", async () => {
    const consoleApi = createConsoleApi();
    const initialResponse = await consoleApi.getRouteConfig("management-secret");
    const pendingRefresh = deferred<typeof initialResponse>();
    const responseWithProvider = (providerId: string, revisionId: string) => ({
      routeConfig: {
        ...initialResponse.routeConfig,
        revision: {
          ...initialResponse.routeConfig.revision,
          id: revisionId,
        },
        document: {
          ...initialResponse.routeConfig.document,
          providers: [{ id: providerId }],
        },
      },
    });
    vi.mocked(consoleApi.getRouteConfig).mockReset();
    vi.mocked(consoleApi.getRouteConfig)
      .mockResolvedValueOnce(initialResponse)
      .mockReturnValueOnce(pendingRefresh.promise);

    renderWithProviders(<BrowserConsoleApp consoleApi={consoleApi} />);

    await waitForConsoleReady();
    const refreshButton = screen.getByRole("button", { name: /^刷新$/i });
    act(() => {
      refreshButton.click();
      refreshButton.click();
    });
    await waitFor(() => expect(consoleApi.getRouteConfig).toHaveBeenCalledTimes(2));

    pendingRefresh.resolve(responseWithProvider("coalesced-provider", "r2-coalesced"));
    await waitFor(() =>
      expect(screen.getAllByText("coalesced-provider").length).toBeGreaterThan(0),
    );
  });

  it("shares busy and failure state across coalesced authoritative refreshes", async () => {
    const consoleApi = createConsoleApi();
    const initialResponse = await consoleApi.getRouteConfig("management-secret");
    const failedRefresh = deferred<typeof initialResponse>();
    vi.mocked(consoleApi.getRouteConfig).mockReset();
    vi.mocked(consoleApi.getRouteConfig)
      .mockResolvedValueOnce(initialResponse)
      .mockReturnValueOnce(failedRefresh.promise);

    renderWithProviders(<BrowserConsoleApp consoleApi={consoleApi} />);

    await waitForConsoleReady();
    const refreshButton = screen.getByRole("button", { name: /^刷新$/i });
    act(() => {
      refreshButton.click();
      refreshButton.click();
    });
    await waitFor(() => expect(consoleApi.getRouteConfig).toHaveBeenCalledTimes(2));
    expect(refreshButton).toBeDisabled();

    await act(async () => {
      failedRefresh.reject(new Error("coalesced refresh failed"));
      await Promise.resolve();
    });

    expect(await screen.findByText("coalesced refresh failed")).toBeInTheDocument();
    await waitFor(() => expect(refreshButton).toBeEnabled());
  });

  it("preserves active repair diagnostics when the latest refresh fails", async () => {
    const consoleApi = createConsoleApi();
    const user = userEvent.setup();
    const initialResponse = await consoleApi.getRouteConfig("management-secret");
    vi.mocked(consoleApi.getRouteConfig).mockResolvedValue({
      routeConfig: {
        ...initialResponse.routeConfig,
        requiresRepair: true,
        diagnostics: { diagnostics: [{
          code: "secret_document_identity_invalid", severity: "error",
          path: "/providers/0/id", message: "active identity needs repair",
        }] },
      },
    });

    renderWithProviders(<BrowserConsoleApp consoleApi={consoleApi} />);

    await waitForConsoleReady();
    expect(screen.getByRole("alert", { name: /active route diagnostics/i })).toHaveTextContent(
      "active identity needs repair",
    );

    vi.mocked(consoleApi.getRouteConfig).mockRejectedValueOnce(new Error("latest refresh failed"));
    await user.click(screen.getByRole("button", { name: /^刷新$/i }));

    expect(await screen.findByText("latest refresh failed")).toBeInTheDocument();
    expect(screen.getByRole("alert", { name: /active route diagnostics/i })).toHaveTextContent(
      "active identity needs repair",
    );
  });

  it("blocks invalid account-group billing multipliers and autosaves valid scientific notation", async () => {
    const consoleApi = createConsoleApi();
    const user = userEvent.setup();

    vi.mocked(consoleApi.getRouteConfig).mockResolvedValue({
      routeConfig: {
        revision: { id: "r1-deadbeefcafe", sequence: 1, message: "initial import" },
        source: "redis",
        diagnostics: { diagnostics: [] },
        requiresRepair: false,
        document: {
          providers: [{ id: "managed-provider" }],
          model_routes: [{ pattern: "gpt-5.4", provider_ids: ["managed-provider"] }],
          aliases: { answer: "gpt-5.4" },
          account_groups: [
            {
              id: "group-vip",
              name: "VIP 分组",
              billing_multiplier: 1.5,
              provider_credential_ids: [],
            },
          ],
        },
        secrets: [{ path: "/providers/0/api_key", configured: true, preview: "sk-***" }],
        mutationSupported: true,
      },
    });

    renderWithProviders(<BrowserConsoleApp consoleApi={consoleApi} />);

    await waitForConsoleReady();
    await openWorkspace(user, /权益组/i);
    await user.click(screen.getByRole("button", { name: /^VIP 分组$/i, expanded: false }));
    const multiplier = screen.getByLabelText(/计费倍率/i);

    fireEvent.change(multiplier, { target: { value: "1abc" } });
    expect(multiplier).toHaveValue("1abc");
    expect(multiplier).toHaveAttribute("aria-invalid", "true");
    expect(multiplier).toHaveAttribute("aria-describedby");
    expect(screen.getByRole("alert")).toHaveTextContent(/计费倍率必须是大于等于 0 的数字/i);

    await act(async () => { await new Promise((resolve) => setTimeout(resolve, 1300)); });
    expect(consoleApi.commitRouteConfig).not.toHaveBeenCalled();
    expect(consoleApi.validateRouteConfig).not.toHaveBeenCalled();

    fireEvent.change(multiplier, { target: { value: "-1" } });
    expect(multiplier).toHaveValue("-1");
    expect(screen.getByRole("alert")).toHaveTextContent(/计费倍率必须是大于等于 0 的数字/i);

    fireEvent.change(multiplier, { target: { value: "1e-2" } });
    await waitFor(() => expect(screen.queryByRole("alert")).not.toBeInTheDocument());
    expect(multiplier).toHaveAttribute("aria-invalid", "false");

    const draft = await waitForCommittedRouteDraft(consoleApi);
    expect(draft.account_groups).toEqual([
      expect.objectContaining({ id: "group-vip", billing_multiplier: 0.01 }),
    ]);
    await waitFor(() =>
      expect(consoleApi.commitRouteConfig).toHaveBeenCalledWith(
        "management-secret",
        expect.objectContaining({
          document: expect.objectContaining({
            account_groups: [
              expect.objectContaining({
                id: "group-vip",
                billing_multiplier: 0.01,
              }),
            ],
          }),
        }),
      ),
    );
  });
});
