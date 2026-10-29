import { act, cleanup, fireEvent, render, renderHook, screen } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import { usePoolScrollLoading } from "./usePoolScrollLoading";
import type { ProviderPoolView } from "./providerPoolView";

const view: ProviderPoolView = { query: "", sort: "default" };
afterEach(() => { cleanup(); vi.unstubAllGlobals(); });

it("appends batches once, resets search/sort, clamps deletions and rejects stale callbacks", () => {
  const { result, rerender } = renderHook(({ total, view }) => usePoolScrollLoading(total, view), {
    initialProps: { total: 73, view },
  });
  expect(result.current.visibleCount).toBe(24);
  const firstLoad = result.current.loadMore;
  act(() => { firstLoad(); firstLoad(); });
  expect(result.current.visibleCount).toBe(48);
  act(() => result.current.loadMore());
  expect(result.current.visibleCount).toBe(72);
  rerender({ total: 25, view });
  expect(result.current.visibleCount).toBe(25);
  expect(result.current.hasMore).toBe(false);
  rerender({ total: 73, view: { ...view, query: "search" } });
  expect(result.current.visibleCount).toBe(24);
  act(() => firstLoad());
  expect(result.current.visibleCount).toBe(24);
  act(() => result.current.loadMore());
  rerender({ total: 73, view: { query: "search", sort: "name" } });
  expect(result.current.visibleCount).toBe(24);
  rerender({ total: 0, view });
  expect(result.current.visibleCount).toBe(0);
  expect(result.current.hasMore).toBe(false);
});

function observerFixture() {
  const observers: { emit: (intersecting: boolean) => void; root: Element | Document | null | undefined; disconnect: ReturnType<typeof vi.fn> }[] = [];
  vi.stubGlobal("IntersectionObserver", class {
    disconnect = vi.fn();
    observe() {}
    constructor(callback: IntersectionObserverCallback, options: IntersectionObserverInit) {
      expect(options.rootMargin).toBe("240px 0px");
      observers.push({ root: options.root, disconnect: this.disconnect,
        emit: (isIntersecting) => act(() => callback([{ isIntersecting } as IntersectionObserverEntry], this as unknown as IntersectionObserver)) });
    }
  });
  return observers;
}

function Workspace({ query = "" }: { query?: string }) {
  const feed = usePoolScrollLoading(49, { ...view, query });
  return <div data-testid="stage" style={{ overflowY: "auto" }} ref={(element) => {
    if (element) Object.defineProperties(element, {
      scrollHeight: { configurable: true, value: 1000 },
      clientHeight: { configurable: true, value: 100 },
    });
  }}>
    <div ref={feed.workspaceRef}>
      <output data-testid="count">{feed.visibleCount}</output>
      {feed.hasMore && <div ref={feed.loaderRef} data-testid="loader" />}
    </div>
  </div>;
}

it("observes the main scroll owner, re-arms for another batch and disconnects at the end", () => {
  const observers = observerFixture();
  render(<Workspace />);
  expect(observers[0].root).toBe(screen.getByTestId("stage"));
  observers[0].emit(false);
  expect(screen.getByTestId("count")).toHaveTextContent("24");
  observers[0].emit(true);
  expect(screen.getByTestId("count")).toHaveTextContent("48");
  expect(observers[0].disconnect).toHaveBeenCalled();
  observers[0].emit(true);
  expect(screen.getByTestId("count")).toHaveTextContent("48");
  observers[1].emit(true);
  expect(screen.getByTestId("count")).toHaveTextContent("49");
  expect(screen.queryByTestId("loader")).not.toBeInTheDocument();
  expect(observers[1].disconnect).toHaveBeenCalled();
});

it("updates the scroll root on resize and ignores old observers after search or unmount", () => {
  const observers = observerFixture();
  const { rerender, unmount } = render(<Workspace />);
  const stage = screen.getByTestId("stage");
  stage.style.overflowY = "visible";
  fireEvent(window, new Event("resize"));
  expect(observers.at(-1)?.root).toBeNull();
  observers[0].emit(true);
  expect(screen.getByTestId("count")).toHaveTextContent("24");
  stage.style.overflowY = "auto";
  stage.scrollTop = 100;
  rerender(<Workspace query="different" />);
  expect(stage.scrollTop).toBe(0);
  observers[1].emit(true);
  expect(screen.getByTestId("count")).toHaveTextContent("24");
  const last = observers.at(-1)!;
  unmount();
  expect(last.disconnect).toHaveBeenCalled();
  last.emit(true);
});

it("does not treat an unbounded auto-overflow container as the scroll viewport", () => {
  const observers = observerFixture();
  render(<Workspace />);
  const stage = screen.getByTestId("stage");
  Object.defineProperty(stage, "clientHeight", { configurable: true, value: 1000 });
  fireEvent(window, new Event("resize"));
  expect(observers.at(-1)?.root).toBeNull();
  expect(screen.getByTestId("count")).toHaveTextContent("24");
});

it("uses the viewport for document scrolling even when body has auto overflow", () => {
  const observers = observerFixture();
  const bodyStyle = document.body.style.overflowY;
  document.body.style.overflowY = "auto";
  Object.defineProperties(document.body, {
    scrollHeight: { configurable: true, value: 1000 },
    clientHeight: { configurable: true, value: 100 },
  });
  try {
    render(<Workspace />);
    screen.getByTestId("stage").style.overflowY = "visible";
    fireEvent(window, new Event("resize"));
    expect(observers.at(-1)?.root).toBeNull();
    observers.at(-1)!.emit(true);
    expect(screen.getByTestId("count")).toHaveTextContent("48");
  } finally {
    document.body.style.overflowY = bodyStyle;
    Reflect.deleteProperty(document.body, "scrollHeight");
    Reflect.deleteProperty(document.body, "clientHeight");
  }
});
