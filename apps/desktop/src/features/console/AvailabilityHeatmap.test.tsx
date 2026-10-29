import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { AvailabilityHeatmap } from "./AvailabilityHeatmap";
import { aggregateSuccessWindows, buildProviderAvailabilityCells } from "./providerCardMetrics";

describe("AvailabilityHeatmap", () => {
  it.each(["provider", "account"] as const)("renders flat %s cells with only time, calls and success", (scope) => {
    const cells = buildProviderAvailabilityCells(aggregateSuccessWindows([{ successWindows: [
      { label: "10:00", success: 3, requests: 4 },
      { label: "10:30", success: 0, requests: 0 },
    ] }]));
    const { container } = render(<AvailabilityHeatmap cells={cells} label="测试" t={(zh) => zh} scope={scope} />);
    const chart = screen.getByRole("group", { name: "测试" });
    expect(chart.children).toHaveLength(2);
    expect(container.querySelector(".nt-provider-card__availability-window")).toBeNull();
    const blocks = screen.getAllByRole("img");
    expect(blocks[0]).toHaveAttribute("title", "时间：10:00–10:30；调用 4 次；成功率 75%");
    expect(blocks[1]).toHaveAttribute("title", "时间：10:30–11:00；调用 0 次；成功率 —");
    expect(blocks[0]).toHaveAttribute("tabindex", "0");
    expect(chart.innerHTML).not.toMatch(/绿色|黄色|红色|灰色|图例|按小时/);
  });

  it("keeps the English tooltip equally concise and does not invent a timestamp for missing data", () => {
    const { rerender } = render(<AvailabilityHeatmap cells={buildProviderAvailabilityCells([
      { label: "23:30", requests: 3, success: 0, rate: 0 },
    ])} label="Quality" t={(_zh, en) => en} />);
    expect(screen.getByRole("img")).toHaveAttribute("title", "Time: 23:30–00:00; Calls: 3; Success: 0%");
    rerender(<AvailabilityHeatmap cells={[]} label="Quality" t={(_zh, en) => en} />);
    expect(screen.getByRole("img")).toHaveAccessibleName("No call data");
    expect(screen.getByRole("img")).not.toHaveAttribute("title");
  });
});
