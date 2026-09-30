import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import { UiThemeProvider, useUiTheme } from "./UiThemeProvider";

function ThemeControl() {
  const { mode, resolved, cycleMode } = useUiTheme();
  return <button onClick={cycleMode}>{mode}:{resolved}</button>;
}

afterEach(() => {
  cleanup();
  localStorage.removeItem("gateway-ui-theme");
  delete document.documentElement.dataset.ntTheme;
  delete document.documentElement.dataset.ntThemeMode;
  vi.unstubAllGlobals();
});

it("starts a fresh installation in Neuro dark even on a light system", () => {
  vi.stubGlobal("matchMedia", vi.fn(() => ({
    matches: true,
    addEventListener: vi.fn(),
    removeEventListener: vi.fn(),
  })));
  render(<UiThemeProvider><ThemeControl /></UiThemeProvider>);
  expect(screen.getByRole("button").textContent).toBe("dark:dark");
  expect(document.documentElement.dataset.ntTheme).toBe("dark");
  fireEvent.click(screen.getByRole("button"));
  expect(screen.getByRole("button").textContent).toBe("light:light");
  expect(localStorage.getItem("gateway-ui-theme")).toBe("light");
});

it.each(["light", "system"])("preserves an existing %s preference", (mode) => {
  localStorage.setItem("gateway-ui-theme", mode);
  vi.stubGlobal("matchMedia", vi.fn(() => ({
    matches: true,
    addEventListener: vi.fn(),
    removeEventListener: vi.fn(),
  })));
  render(<UiThemeProvider><ThemeControl /></UiThemeProvider>);
  expect(screen.getByRole("button").textContent).toBe(`${mode}:light`);
  expect(localStorage.getItem("gateway-ui-theme")).toBe(mode);
});
