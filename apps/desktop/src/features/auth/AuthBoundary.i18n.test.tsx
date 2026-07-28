import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it } from "vitest";
import { AuthBoundary } from "./AuthBoundary";
import {
  ManagementSessionContext,
  type ManagementSessionContextValue,
} from "../../session/ManagementSessionProvider";
import { UiLocaleProvider } from "../../i18n/UiLocaleProvider";

type SessionOverrides = Partial<ManagementSessionContextValue>;

function sessionValue(overrides: SessionOverrides = {}): ManagementSessionContextValue {
  return {
    phase: "unauthenticated",
    bootstrapStatus: null,
    session: null,
    managementToken: null,
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
    ...overrides,
  };
}

function renderWithProviders(overrides: SessionOverrides = {}) {
  return render(
    <UiLocaleProvider>
      <ManagementSessionContext.Provider value={sessionValue(overrides)}>
        <AuthBoundary>
          <div>ready</div>
        </AuthBoundary>
      </ManagementSessionContext.Provider>
    </UiLocaleProvider>,
  );
}

describe("AuthBoundary localization", () => {
  beforeEach(() => {
    window.localStorage.clear();
  });

  it("defaults the login experience to Chinese and toggles to English", async () => {
    const user = userEvent.setup();

    renderWithProviders({ phase: "unauthenticated" });

    expect(screen.getByRole("heading", { name: "登录" })).toBeInTheDocument();
    expect(screen.getByLabelText("管理密钥")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "登录" })).toBeInTheDocument();
    expect(screen.getByText("English")).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "切换界面语言" }));

    expect(screen.getByRole("heading", { name: "Sign in" })).toBeInTheDocument();
    expect(screen.getByLabelText("Management token")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Sign in" })).toBeInTheDocument();
    expect(screen.getByText("中文")).toBeInTheDocument();
  });
});
