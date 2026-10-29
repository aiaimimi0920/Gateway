import { useLayoutEffect, useRef } from "react";

const MINIMUM_GAP = 16;
const MAXIMUM_GAP = 28;

export function centeredPoolGridLayout(width: number, cardWidth: number) {
  // Count with the smallest gap so extra spacing never excludes a card that fits.
  const columns = Math.max(1, Math.floor((width + MINIMUM_GAP) / (cardWidth + MINIMUM_GAP)));
  const spare = Math.max(0, width - columns * cardWidth - (columns - 1) * MINIMUM_GAP);
  const gap = columns === 1 ? MINIMUM_GAP : Math.min(MAXIMUM_GAP, MINIMUM_GAP + spare / (columns + 1));
  return { columns, gap };
}

/** Own grid tracks only: card contents, dimensions, and account-library rows stay unchanged. */
export function useCenteredPoolGrid() {
  const gridRef = useRef<HTMLDivElement>(null);
  useLayoutEffect(() => {
    const grid = gridRef.current;
    if (!grid) return;
    const cardWidth = parseFloat(getComputedStyle(grid).getPropertyValue("--nt-provider-grid-card-width")) || 292;
    let previousWidth = -1;
    const update = (width: number) => {
      if (width <= 0 || Math.abs(width - previousWidth) < 0.01) return;
      previousWidth = width;
      const { columns, gap } = centeredPoolGridLayout(width, cardWidth);
      grid.style.setProperty("--nt-provider-grid-columns", String(columns));
      grid.style.setProperty("--nt-provider-grid-column-gap", `${gap.toFixed(2)}px`);
    };
    const measure = () => update(grid.getBoundingClientRect().width);
    measure();
    const observer = typeof ResizeObserver === "undefined" ? null : new ResizeObserver(([entry]) => update(entry.contentRect.width));
    observer?.observe(grid);
    window.addEventListener("resize", measure);
    return () => {
      observer?.disconnect();
      window.removeEventListener("resize", measure);
    };
  }, []);
  return gridRef;
}
