import React from "react";
import { createRoot } from "react-dom/client";
import { DesktopConnectionApp } from "./features/connection/DesktopConnectionApp";
import { BrowserConsoleApp } from "./features/console/BrowserConsoleApp";
import { AuthBoundary } from "./features/auth/AuthBoundary";
import { HostProvider } from "./platform/HostProvider";
import { createBrowserHost } from "./platform/browserHost";
import type { GatewayUiTarget } from "./platform/types";
import { ManagementSessionProvider } from "./session/ManagementSessionProvider";
import { UiLocaleProvider } from "./i18n/UiLocaleProvider";
import { UiThemeProvider } from "./theme/UiThemeProvider";
import { AppToastViewport } from "./components/AppToast";
import { DesktopWindowChrome } from "./features/shell/DesktopWindowChrome";
import "./styles.css";

declare const __GATEWAY_UI_TARGET__: GatewayUiTarget | undefined;

const rootElement = document.getElementById("root");

if (!rootElement) {
  throw new Error("Neuro Gateway desktop root element was not found");
}

const target = typeof __GATEWAY_UI_TARGET__ === "undefined" ? "web" : __GATEWAY_UI_TARGET__;

createRoot(rootElement).render(
  <React.StrictMode>
    <UiThemeProvider>
      <UiLocaleProvider>
        <DesktopWindowChrome />
        {target === "web" ? (
          <HostProvider adapter={createBrowserHost()}>
            <ManagementSessionProvider>
              <AuthBoundary>
                <BrowserConsoleApp />
              </AuthBoundary>
            </ManagementSessionProvider>
          </HostProvider>
        ) : (
          <DesktopConnectionApp />
        )}
        <AppToastViewport />
      </UiLocaleProvider>
    </UiThemeProvider>
  </React.StrictMode>,
);
