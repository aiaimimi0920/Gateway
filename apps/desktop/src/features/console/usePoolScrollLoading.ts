import { useCallback, useEffect, useLayoutEffect, useRef, useState } from "react";
import type { ProviderPoolView } from "./providerPoolView";

const BATCH_SIZE = 24;

function scrollRoot(element: HTMLElement): HTMLElement | null {
  for (let parent = element.parentElement; parent; parent = parent.parentElement) {
    // Document scrolling uses the viewport even when body reports auto overflow.
    if (parent === document.body || parent === document.documentElement) return null;
    // overflow-x:hidden can imply overflow-y:auto on an unbounded mobile container.
    // Such a root sees its entire contents and would eagerly reveal every batch.
    if (/(auto|scroll)/.test(getComputedStyle(parent).overflowY)
      && parent.scrollHeight > parent.clientHeight + 1) return parent;
  }
  return null;
}

/** Reveal the already-fetched catalog incrementally; never change routing or request credentials. */
export function usePoolScrollLoading(total: number, view: ProviderPoolView) {
  const workspaceRef = useRef<HTMLDivElement>(null);
  const loaderRef = useRef<HTMLDivElement>(null);
  const [selection, setSelection] = useState({ ...view, limit: BATCH_SIZE });
  const accessKey = view.accessKinds?.join(",");
  const viewChanged = selection.query !== view.query || selection.sort !== view.sort || selection.accessKinds?.join(",") !== accessKey;
  const limit = viewChanged ? BATCH_SIZE : selection.limit;
  if (viewChanged) setSelection({ ...view, limit });
  const visibleCount = Math.min(limit, total);
  const hasMore = visibleCount < total;
  const automatic = typeof IntersectionObserver !== "undefined";
  const loadMore = useCallback(() => {
    setSelection((current) => {
      // Ignore duplicate observer deliveries and callbacks from a previous search/sort.
      if (current.query !== view.query || current.sort !== view.sort
        || current.accessKinds?.join(",") !== accessKey
        || current.limit !== limit || limit >= total) return current;
      return { ...current, limit: Math.min(total, limit + BATCH_SIZE) };
    });
  }, [limit, total, view.query, view.sort, accessKey]);

  useLayoutEffect(() => {
    const workspace = workspaceRef.current;
    if (!workspace) return;
    const root = scrollRoot(workspace) ?? document.scrollingElement;
    if (root) root.scrollTop = 0;
  }, [view.query, view.sort, accessKey]);

  useEffect(() => {
    const loader = loaderRef.current;
    if (!loader || !hasMore || !automatic) return;
    let active = true;
    let requested = false;
    let generation = 0;
    let observer: IntersectionObserver | undefined;
    const observe = () => {
      observer?.disconnect();
      const currentGeneration = ++generation;
      if (requested) return;
      observer = new IntersectionObserver((entries) => {
        if (!active || requested || currentGeneration !== generation
          || !entries.some((entry) => entry.isIntersecting)) return;
        requested = true;
        loadMore();
      }, { root: scrollRoot(loader), rootMargin: "240px 0px", threshold: 0 });
      observer.observe(loader);
    };
    observe();
    // On narrow screens the stage can stop being the scroll owner; rebuild its root.
    window.addEventListener("resize", observe);
    return () => {
      active = false;
      observer?.disconnect();
      window.removeEventListener("resize", observe);
    };
  }, [automatic, hasMore, visibleCount, loadMore]);

  return { workspaceRef, loaderRef, visibleCount, hasMore, automatic, loadMore };
}
