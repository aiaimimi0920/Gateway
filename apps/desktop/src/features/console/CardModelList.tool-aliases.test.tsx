import { render, within } from "@testing-library/react";
import { expect, it } from "vitest";
import { CardModelList } from "./CardModelList";

it("keeps tool aliases out of rows, counts and totals without hiding concrete operation telemetry", () => {
  const models = ["search", "fetch", "exa-search", "exa-fetch"];
  const { container } = render(<CardModelList models={models} label="Exa" t={(_zh, en) => en}
    traffic={{ emptyRequestCount: 0, models: new Map([
      ["search", { requestCount: 100, successCount: 100, successWindows: [] }],
      ["fetch", { requestCount: 200, successCount: 200, successWindows: [] }],
      ["exa-search", { requestCount: 3, successCount: 2, successWindows: [
        { label: "01:00", requests: 3, success: 2 },
      ] }],
      ["exa-fetch", { requestCount: 2, successCount: 2, successWindows: [] }],
    ]) }} />);
  expect(models).toEqual(["search", "fetch", "exa-search", "exa-fetch"]);
  expect(container.querySelector('[data-card-model="search"]')).toBeNull();
  expect(container.querySelector('[data-card-model="fetch"]')).toBeNull();
  expect(container.querySelector(".nt-card-models__heading b")).toHaveTextContent("2");
  const summary = container.querySelector("summary")!;
  expect(summary.querySelector('[data-model-metric="requests"]')).toHaveTextContent("5");
  expect(summary.querySelector('[data-model-metric="success-rate"]')).toHaveTextContent("80%");
  const search = container.querySelector('[data-card-model="exa-search"]')!;
  expect(within(search as HTMLElement).getByText("exa-search")).toBeInTheDocument();
  expect(search.querySelector('[data-model-metric="requests"]')).toHaveTextContent("3");
  expect(search.querySelector(".nt-card-models__cells > span")).toHaveAttribute("title", "Time: 01:00–01:30; Calls: 3; Success: 66.7%");
});
