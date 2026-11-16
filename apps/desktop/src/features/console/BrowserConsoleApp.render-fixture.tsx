import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import type { ReactNode } from "react";
import { expect, vi } from "vitest";
import type { ConsoleApi } from "../../api/console";
import type { ConsoleRouteDocument } from "../../api/contracts";
import { AppToastViewport } from "../../components/AppToast";
import { UiLocaleProvider } from "../../i18n/UiLocaleProvider";
import { UiThemeProvider } from "../../theme/UiThemeProvider";
import { HostProvider } from "../../platform/HostProvider";
import { createBrowserHost } from "../../platform/browserHost";
import {
  ManagementSessionContext,
  type ManagementSessionContextValue,
} from "../../session/ManagementSessionProvider";

type SessionOverrides = Omit<Partial<ManagementSessionContextValue>, "session"> & {
  session?: Partial<NonNullable<ManagementSessionContextValue["session"]>> | null;
};

export function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason?: unknown) => void;
  const promise = new Promise<T>((resolvePromise, rejectPromise) => {
    resolve = resolvePromise;
    reject = rejectPromise;
  });
  return { promise, reject, resolve };
}

function sessionValue(
  overrides: SessionOverrides = {},
): ManagementSessionContextValue {
  const base: ManagementSessionContextValue = {
    phase: "authenticated",
    bootstrapStatus: null,
    session: {
      role: "administrator",
      capabilities: ["route-config:read", "route-config:write"],
      activeRevision: "r1-deadbeefcafe",
      secretAccessGranted: false,
    },
    managementToken: "management-secret",
    secretGrant: null,
    busy: false,
    error: null,
    bootstrap: async () => undefined,
    login: async () => undefined,
    logout: async () => undefined,
    rotate: async () => undefined,
    confirmSecretAccess: async () => undefined,
    clearSecretGrant: () => undefined,
    retryInitialization: async () => undefined,
  };
  const mergedSession =
    overrides.session === null
      ? null
      : ({
          ...base.session,
          ...(overrides.session ?? {}),
        } as NonNullable<ManagementSessionContextValue["session"]>);

  return {
    ...base,
    ...overrides,
    session: mergedSession,
  };
}

export function renderWithProviders(
  children: ReactNode,
  overrides: SessionOverrides = {},
) {
  const host = createBrowserHost();
  const renderTree = (nextOverrides: SessionOverrides) => (
    <UiThemeProvider><UiLocaleProvider>
      <HostProvider adapter={host}>
        <ManagementSessionContext.Provider value={sessionValue(nextOverrides)}>
          {children}
          <AppToastViewport />
        </ManagementSessionContext.Provider>
      </HostProvider>
    </UiLocaleProvider></UiThemeProvider>
  );
  const result = render(renderTree(overrides));
  return {
    ...result,
    rerenderWithProviders(nextOverrides: SessionOverrides) {
      result.rerender(renderTree(nextOverrides));
    },
  };
}

export async function waitForConsoleReady() {
  await waitFor(() =>
    expect(screen.getByRole("navigation", { name: /Gateway console navigation/i })).toBeInTheDocument(),
  );
  await waitFor(() =>
    expect(screen.queryByText(/正在加载 Gateway 控制台|Loading Gateway console/)).not.toBeInTheDocument(),
  );
}

export function consoleNavigation() {
  return screen.getByRole("navigation", { name: /Gateway console navigation/i });
}

export function workspaceButton(name: RegExp) {
  return within(consoleNavigation()).getByRole("button", { name });
}

export async function openWorkspace(user: ReturnType<typeof userEvent.setup>, name: RegExp) {
  await user.click(workspaceButton(name));
}

export async function waitForCommittedRouteDraft(
  consoleApi: ConsoleApi,
  previousCallCount = 0,
): Promise<ConsoleRouteDocument> {
  const commit = vi.mocked(consoleApi.commitRouteConfig);
  await waitFor(() => expect(commit.mock.calls.length).toBeGreaterThan(previousCallCount), {
    timeout: 3000,
  });
  return commit.mock.calls.at(-1)![1].document;
}

export function providerCard(name: RegExp) {
  const card = screen.getByRole("button", { name }).closest(".nt-provider-card");
  expect(card).not.toBeNull();
  return card as HTMLElement;
}

/** Open the current directory settings dialog without bypassing provider selection. */
export async function openProviderSettings(user: ReturnType<typeof userEvent.setup>, name: RegExp) {
  const directory = within(screen.getByRole("navigation", { name: "服务商目录" }));
  await user.click(directory.getByRole("button", { name }));
  await user.click(screen.getByRole("button", { name: "服务商设置" }));
  return screen.getByRole("dialog", { name: "服务商设置" });
}

export function providerAccountLibrary(name: RegExp) {
  return screen.getByRole("region", { name });
}
