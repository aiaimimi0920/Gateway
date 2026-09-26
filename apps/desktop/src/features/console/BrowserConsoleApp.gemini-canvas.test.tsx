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

  it("starts a browser-first Gemini Canvas manual-add flow and merges both Canvas families into the draft", async () => {
    const consoleApi = createConsoleApi();
    const user = userEvent.setup();

    const initialRouteConfig = {
      routeConfig: {
        revision: { id: "r1-deadbeefcafe", sequence: 1, message: "initial import" },
        source: "redis" as const,
        diagnostics: { diagnostics: [] },
        requiresRepair: false,
        document: {
          providers: [
            {
              id: "gemini-canvas",
              label: "gemini-canvas",
              vendor_key: "google-gemini",
              vendor_name: "Google / Gemini",
              preset: "gemini-canvas",
              base_url: "https://gemini.google.com",
              supported_models: ["gemini-2.5-flash-image-preview"],
            },
            {
              id: "gemini-canvas-chat",
              label: "gemini-canvas-chat",
              vendor_key: "google-gemini",
              vendor_name: "Google / Gemini",
              preset: "gemini-canvas-chat",
              base_url: "https://gemini.google.com",
              supported_models: ["gemini-2.5-flash"],
            },
          ],
          model_routes: [],
          aliases: {},
          account_groups: [],
        },
        secrets: [],
        mutationSupported: true,
      },
    };
    const committedRouteConfig = {
      routeConfig: {
        ...initialRouteConfig.routeConfig,
        revision: { id: "r2-gemini-import", sequence: 2, message: "Import Gemini manual credentials (gemini-session-1)" },
        document: {
          ...initialRouteConfig.routeConfig.document,
          providers: [
            {
              ...initialRouteConfig.routeConfig.document.providers[0],
              credentials: [
                {
                  id: "gemini-canvas-manual-1",
                  account_name: "Gemini Canvas Manual 1",
                  runtime_state_object_key:
                    "credential-runtime/gemini-canvas/manual-1/storage-state.json",
                  extra_body: {
                    shareId: "fe24c455a570",
                  },
                },
              ],
            },
            {
              ...initialRouteConfig.routeConfig.document.providers[1],
              credentials: [
                {
                  id: "gemini-canvas-chat-manual-1",
                  account_name: "Gemini Canvas Chat Manual 1",
                  runtime_state_object_key:
                    "credential-runtime/gemini-canvas/manual-1/storage-state.json",
                  extra_body: {
                    shareId: "fe24c455a570",
                    apiBaseUrl: "https://generativelanguage.googleapis.com/v1beta",
                  },
                },
              ],
            },
          ],
        },
      },
    };
    vi.mocked(consoleApi.getRouteConfig)
      .mockResolvedValueOnce(initialRouteConfig)
      .mockResolvedValue(committedRouteConfig);
    vi.mocked(consoleApi.commitRouteConfig).mockResolvedValue({
      routeConfig: committedRouteConfig.routeConfig,
      committed: true,
    });
    vi.mocked(consoleApi.createGeminiAuthSession).mockResolvedValue({
      session: {
        id: "gemini-session-1",
        targetFamily: "gemini-canvas",
        providerId: "gemini-canvas",
        status: "waiting_user",
        message: "Complete Gemini login in the opened browser window.",
        createdAt: "2026-07-30T09:00:00Z",
        updatedAt: "2026-07-30T09:00:00Z",
        generatedDrafts: [],
      },
    });
    vi.mocked(consoleApi.getGeminiAuthSession).mockResolvedValue({
      session: {
        id: "gemini-session-1",
        targetFamily: "gemini-canvas",
        providerId: "gemini-canvas",
        status: "succeeded",
        message: "Gemini Canvas runtime captured.",
        createdAt: "2026-07-30T09:00:00Z",
        updatedAt: "2026-07-30T09:02:00Z",
        generatedDrafts: [
          {
            providerId: "gemini-canvas",
            credential: {
              id: "gemini-canvas-manual-1",
              account_name: "Gemini Canvas Manual 1",
              runtime_state_object_key:
                "credential-runtime/gemini-canvas/manual-1/storage-state.json",
              extra_body: {
                shareId: "fe24c455a570",
              },
            },
            secretEdits: [],
          },
          {
            providerId: "gemini-canvas-chat",
            credential: {
              id: "gemini-canvas-chat-manual-1",
              account_name: "Gemini Canvas Chat Manual 1",
              runtime_state_object_key:
                "credential-runtime/gemini-canvas/manual-1/storage-state.json",
              extra_body: {
                shareId: "fe24c455a570",
                apiBaseUrl: "https://generativelanguage.googleapis.com/v1beta",
              },
            },
            secretEdits: [],
          },
        ],
      },
    });

    renderWithProviders(<BrowserConsoleApp consoleApi={consoleApi} />);

    await waitForConsoleReady();
    await openWorkspace(user, /凭据池/i);
    await user.click(screen.getByRole("button", { name: /^Gemini Canvas$/i }));

    const canvasArticle = providerAccountLibrary(/Gemini Canvas 账号库/i);
    await user.click(
      within(canvasArticle).getByRole("button", { name: /手动添加|Manual add/i }),
    );

    await waitFor(() =>
      expect(consoleApi.createGeminiAuthSession).toHaveBeenCalledWith("management-secret", {
        targetFamily: "gemini-canvas",
        providerId: "gemini-canvas",
      }),
    );

    const dialog = screen.getByRole("dialog", { name: /Gemini 手动添加|Gemini manual add/i });
    expect(within(dialog).getByText("gemini-canvas")).toBeInTheDocument();

    await waitFor(() =>
      expect(consoleApi.getGeminiAuthSession).toHaveBeenCalledWith(
        "management-secret",
        "gemini-session-1",
      ),
    );
    await waitFor(() =>
      expect(consoleApi.commitRouteConfig).toHaveBeenCalledWith(
        "management-secret",
        expect.objectContaining({
          document: expect.objectContaining({
            providers: expect.arrayContaining([
              expect.objectContaining({
                id: "gemini-canvas",
                credentials: expect.arrayContaining([
                  expect.objectContaining({ id: "gemini-canvas-manual-1" }),
                ]),
              }),
              expect.objectContaining({
                id: "gemini-canvas-chat",
                credentials: expect.arrayContaining([
                  expect.objectContaining({ id: "gemini-canvas-chat-manual-1" }),
                ]),
              }),
            ]),
          }),
        }),
      ),
    );

  });

  it("allows the operator to manually continue Gemini Canvas import after finishing login", async () => {
    const consoleApi = createConsoleApi();
    const user = userEvent.setup();

    const initialRouteConfig = {
      routeConfig: {
        revision: { id: "r1-deadbeefcafe", sequence: 1, message: "initial import" },
        source: "redis" as const,
        diagnostics: { diagnostics: [] },
        requiresRepair: false,
        document: {
          providers: [
            {
              id: "gemini-canvas",
              label: "gemini-canvas",
              vendor_key: "google-gemini",
              vendor_name: "Google / Gemini",
              preset: "gemini-canvas",
              base_url: "https://gemini.google.com",
              supported_models: ["gemini-2.5-flash"],
            },
          ],
          model_routes: [],
          aliases: {},
          account_groups: [],
        },
        secrets: [],
        mutationSupported: true,
      },
    };
    const committedRouteConfig = {
      routeConfig: {
        ...initialRouteConfig.routeConfig,
        revision: {
          id: "r2-gemini-manual-complete",
          sequence: 2,
          message: "Import Gemini manual credentials (gemini-session-manual-complete)",
        },
        document: {
          ...initialRouteConfig.routeConfig.document,
          providers: [
            {
              ...initialRouteConfig.routeConfig.document.providers[0],
              credentials: [
                {
                  id: "gemini-canvas-manual-complete-1",
                  account_name: "Gemini Canvas Manual Complete 1",
                  runtime_state_object_key:
                    "credential-runtime/gemini-canvas/manual-complete/storage-state.json",
                  extra_body: {
                    shareId: "fe24c455a570",
                  },
                },
              ],
            },
          ],
        },
      },
    };
    vi.mocked(consoleApi.getRouteConfig)
      .mockResolvedValueOnce(initialRouteConfig)
      .mockResolvedValue(committedRouteConfig);
    vi.mocked(consoleApi.commitRouteConfig).mockResolvedValue({
      routeConfig: committedRouteConfig.routeConfig,
      committed: true,
    });
    vi.mocked(consoleApi.createGeminiAuthSession).mockResolvedValue({
      session: {
        id: "gemini-session-manual-complete",
        targetFamily: "gemini-canvas",
        providerId: "gemini-canvas",
        status: "waiting_user",
        message: "Complete Gemini login in the opened browser window.",
        createdAt: "2026-07-30T12:00:00Z",
        updatedAt: "2026-07-30T12:00:00Z",
        generatedDrafts: [],
      },
    });
    vi.mocked(consoleApi.completeGeminiAuthSession).mockResolvedValue({
      session: {
        id: "gemini-session-manual-complete",
        targetFamily: "gemini-canvas",
        providerId: "gemini-canvas",
        status: "waiting_user",
        message: "Manual Gemini import requested. Finishing capture.",
        createdAt: "2026-07-30T12:00:00Z",
        updatedAt: "2026-07-30T12:00:02Z",
        generatedDrafts: [],
      },
    });
    vi.mocked(consoleApi.getGeminiAuthSession)
      .mockResolvedValueOnce({
        session: {
          id: "gemini-session-manual-complete",
          targetFamily: "gemini-canvas",
          providerId: "gemini-canvas",
          status: "waiting_user",
          message: "Complete Gemini login in the opened browser window.",
          createdAt: "2026-07-30T12:00:00Z",
          updatedAt: "2026-07-30T12:00:00Z",
          generatedDrafts: [],
        },
      })
      .mockResolvedValue({
        session: {
          id: "gemini-session-manual-complete",
          targetFamily: "gemini-canvas",
          providerId: "gemini-canvas",
          status: "succeeded",
          message: "Gemini Canvas runtime captured.",
          createdAt: "2026-07-30T12:00:00Z",
          updatedAt: "2026-07-30T12:00:03Z",
          generatedDrafts: [
            {
              providerId: "gemini-canvas",
              credential: {
                id: "gemini-canvas-manual-complete-1",
                account_name: "Gemini Canvas Manual Complete 1",
                runtime_state_object_key:
                  "credential-runtime/gemini-canvas/manual-complete/storage-state.json",
                extra_body: {
                  shareId: "fe24c455a570",
                },
              },
              secretEdits: [],
            },
          ],
        },
      });

    renderWithProviders(<BrowserConsoleApp consoleApi={consoleApi} />);

    await waitForConsoleReady();
    await openWorkspace(user, /凭据池/i);
    await user.click(screen.getByRole("button", { name: /^Gemini Canvas$/i }));

    const canvasArticle = providerAccountLibrary(/Gemini Canvas 账号库/i);
    await user.click(
      within(canvasArticle).getByRole("button", { name: /手动添加|Manual add/i }),
    );

    const dialog = screen.getByRole("dialog", { name: /Gemini 手动添加|Gemini manual add/i });
    const continueButton = within(dialog).getByRole("button", {
      name: /已完成登录.*导入|I finished login.*import/i,
    });
    await user.click(continueButton);

    await waitFor(() =>
      expect(consoleApi.completeGeminiAuthSession).toHaveBeenCalledWith(
        "management-secret",
        "gemini-session-manual-complete",
      ),
    );

    await waitFor(() =>
      expect(consoleApi.getGeminiAuthSession).toHaveBeenCalledWith(
        "management-secret",
        "gemini-session-manual-complete",
      ),
    );
    await waitFor(() =>
      expect(consoleApi.commitRouteConfig).toHaveBeenCalledWith(
        "management-secret",
        expect.objectContaining({
          document: expect.objectContaining({
            providers: expect.arrayContaining([
              expect.objectContaining({
                id: "gemini-canvas",
                credentials: expect.arrayContaining([
                  expect.objectContaining({ id: "gemini-canvas-manual-complete-1" }),
                ]),
              }),
            ]),
          }),
        }),
      ),
    );

  });
});
