import { screen, waitFor, within } from "@testing-library/react";
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
import { renderWithProviders, waitForConsoleReady, openWorkspace, providerAccountLibrary } from "./BrowserConsoleApp.render-fixture";

describe("BrowserConsoleApp", () => {
  beforeEach(() => {
    window.localStorage.clear();
  });

  it("captures Gemini Business runtime material and keeps its JWT in a secret patch", async () => {
    const consoleApi = createConsoleApi();
    const user = userEvent.setup();

    vi.mocked(consoleApi.getRouteConfig).mockResolvedValue({
      routeConfig: {
        revision: { id: "r1-deadbeefcafe", sequence: 1, message: "initial import" },
        source: "redis",
        diagnostics: { diagnostics: [] },
        requiresRepair: false,
        document: {
          providers: [
            {
              id: "gemini-business",
              label: "gemini-business",
              vendor_key: "google-gemini",
              vendor_name: "Google / Gemini",
              preset: "gemini-business",
              base_url: "https://biz-discoveryengine.googleapis.com/v1alpha",
              supported_models: ["nano-banana-pro"],
            },
          ],
          model_routes: [],
          aliases: {},
          account_groups: [],
        },
        secrets: [],
        mutationSupported: true,
      },
    });
    vi.mocked(consoleApi.createGeminiAuthSession).mockResolvedValue({
      session: {
        id: "gemini-business-session-1",
        targetFamily: "gemini-business",
        providerId: "gemini-business",
        status: "waiting_user",
        message:
          "Complete Gemini Business login, then trigger one Gemini Business request in the opened browser window.",
        createdAt: "2026-07-30T09:00:00Z",
        updatedAt: "2026-07-30T09:00:00Z",
        generatedDrafts: [],
      },
    });
    vi.mocked(consoleApi.getGeminiAuthSession).mockResolvedValue({
      session: {
        id: "gemini-business-session-1",
        targetFamily: "gemini-business",
        providerId: "gemini-business",
        status: "succeeded",
        message: "Gemini Business runtime captured.",
        createdAt: "2026-07-30T09:00:00Z",
        updatedAt: "2026-07-30T09:00:03Z",
        generatedDrafts: [
          {
            providerId: "gemini-business",
            credential: {
              id: "gemini-business-manual-1",
              account_name: "Gemini Business Manual 1",
              api_key: "",
              extra_body: {
                configId: "cfg-123",
                session: "projects/demo/sessions/abc",
              },
            },
            secretEdits: [
              {
                field: "api_key",
                operation: "replace",
                value: "ey.demo.jwt",
              },
            ],
          },
        ],
      },
    });

    renderWithProviders(<BrowserConsoleApp consoleApi={consoleApi} />, {
      session: { secretAccessGranted: true },
      secretGrant: { grant: "gemini-business-grant", expiresAt: "2099-01-01T00:00:00Z" },
    });

    await waitForConsoleReady();
    await openWorkspace(user, /凭据池/i);
    await user.click(screen.getByRole("button", { name: /^Gemini Business$/i }));

    const businessArticle = providerAccountLibrary(/Gemini Business 账号库/i);
    await user.click(
      within(businessArticle).getByRole("button", { name: /手动添加|Manual add/i }),
    );

    await waitFor(() =>
      expect(consoleApi.createGeminiAuthSession).toHaveBeenCalledWith("management-secret", {
        targetFamily: "gemini-business",
        providerId: "gemini-business",
      }),
    );

    const dialog = screen.getByRole("dialog", { name: /Gemini 手动添加|Gemini manual add/i });
    expect(within(dialog).getByText("gemini-business")).toBeInTheDocument();

    await waitFor(() =>
      expect(consoleApi.getGeminiAuthSession).toHaveBeenCalledWith(
        "management-secret",
        "gemini-business-session-1",
      ),
    );

    await waitFor(() => expect(consoleApi.commitRouteConfig).toHaveBeenCalled(), { timeout: 3000 });
    await waitFor(() =>
      expect(consoleApi.commitRouteConfig).toHaveBeenCalledWith(
        "management-secret",
        expect.objectContaining({
          document: expect.objectContaining({
            providers: expect.arrayContaining([
              expect.objectContaining({
                id: "gemini-business",
                credentials: expect.arrayContaining([
                  expect.objectContaining({
                    id: "gemini-business-manual-1",
                    extra_body: {
                      configId: "cfg-123",
                      session: "projects/demo/sessions/abc",
                    },
                  }),
                ]),
              }),
            ]),
          }),
          secretPatches: expect.arrayContaining([
            expect.objectContaining({
              operation: "replace",
              value: "ey.demo.jwt",
            }),
          ]),
        }),
        "gemini-business-grant",
      ),
    );
  });
});
