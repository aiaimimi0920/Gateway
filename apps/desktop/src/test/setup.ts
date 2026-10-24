import "@testing-library/jest-dom/vitest";
import { cleanup } from "@testing-library/react";
import { afterAll, afterEach, beforeAll, vi } from "vitest";
import { server } from "./server";

beforeAll(() => server.listen({ onUnhandledRequest: "error" }));
// jsdom has no layout observer; Radix mounts one when keyboard focus opens a tooltip.
beforeAll(() => vi.stubGlobal("ResizeObserver", class {
  observe() { }
  unobserve() { }
  disconnect() { }
}));

// Native layout/pointer/popover methods are absent in jsdom; browser tests cover top-layer geometry.
const selectDomFallbacks = {
  hasPointerCapture: () => false,
  releasePointerCapture: () => { },
  scrollIntoView: () => { },
  showPopover(this: HTMLElement) { this.style.display = "block"; },
  hidePopover(this: HTMLElement) { this.style.removeProperty("display"); },
};
const addedDomMethods: string[] = [];
beforeAll(() => {
  for (const [name, value] of Object.entries(selectDomFallbacks)) {
    if (name in HTMLElement.prototype) continue;
    Object.defineProperty(HTMLElement.prototype, name, { value, configurable: true });
    addedDomMethods.push(name);
  }
});
afterAll(() => {
  for (const name of addedDomMethods) Reflect.deleteProperty(HTMLElement.prototype, name);
});

afterEach(() => {
  cleanup();
  server.resetHandlers();
  window.sessionStorage.clear();
  window.localStorage.clear();
});

afterAll(() => server.close());
afterAll(() => vi.unstubAllGlobals());
