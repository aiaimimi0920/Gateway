import { act, cleanup, render } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import { centeredPoolGridLayout, useCenteredPoolGrid } from "./useCenteredPoolGrid";

afterEach(() => { cleanup(); vi.restoreAllMocks(); vi.unstubAllGlobals(); });

it("keeps gaps bounded and admits every column that fits at the minimum gap", () => {
  for (const width of [200, 292, 600, 907, 908, 1170, 1600, 2400]) {
    const result = centeredPoolGridLayout(width, 292);
    expect(result.columns).toBe(Math.max(1, Math.floor((width + 16) / 308)));
    expect(result.gap).toBeGreaterThanOrEqual(16);
    expect(result.gap).toBeLessThanOrEqual(28);
    if (width >= 292) expect(result.columns * 292 + (result.columns - 1) * result.gap).toBeLessThanOrEqual(width);
  }
  expect(centeredPoolGridLayout(907, 292)).toEqual({ columns: 2, gap: 28 });
  expect(centeredPoolGridLayout(908, 292)).toEqual({ columns: 3, gap: 16 });
});

it("updates only grid variables on resize and releases the observer on unmount", () => {
  let resize: ResizeObserverCallback = () => {};
  const disconnect = vi.fn();
  vi.stubGlobal("ResizeObserver", class {
    constructor(callback: ResizeObserverCallback) { resize = callback; }
    observe() {}
    disconnect = disconnect;
  });
  vi.spyOn(HTMLElement.prototype, "getBoundingClientRect").mockReturnValue({ width: 908 } as DOMRect);
  function Grid() {
    const ref = useCenteredPoolGrid();
    return <div ref={ref} data-testid="grid"><article>Original card</article></div>;
  }
  const { getByTestId, getByText, unmount } = render(<Grid />);
  const grid = getByTestId("grid");
  const card = getByText("Original card");
  expect(grid.style.getPropertyValue("--nt-provider-grid-columns")).toBe("3");
  act(() => resize([{ contentRect: { width: 1200 } } as ResizeObserverEntry], {} as ResizeObserver));
  expect(grid.style.getPropertyValue("--nt-provider-grid-column-gap")).toBe("28.00px");
  expect(getByText("Original card")).toBe(card);
  expect(card.getAttribute("style")).toBeNull();
  unmount();
  expect(disconnect).toHaveBeenCalledOnce();
});
