import { fireEvent, render, screen } from "@testing-library/react";
import { expect, it, vi } from "vitest";
import { PilotProviderSchedulePanel } from "./PilotProviderSchedulePanel";

it("uses the standard field error without changing schedule validity or string input", () => {
  const props = { t: (zh: string) => zh, section: {} as never, closePilotActionDialog: vi.fn(), editorLocked: false, pilotScheduleEnabled: true, setPilotScheduleEnabled: vi.fn(), pilotScheduleIntervalMinutes: "0", setPilotScheduleIntervalMinutes: vi.fn(), applyProviderProbeSchedule: vi.fn() };
  const view = render(<PilotProviderSchedulePanel {...props} />);
  const input = screen.getByRole("spinbutton", { name: "执行间隔（分钟）" });
  expect(input).toHaveAttribute("aria-invalid", "true");
  expect(input).toHaveAccessibleDescription("请输入 1–10080 的整数。");
  expect(screen.getByRole("alert")).toHaveClass("n-field-error");
  expect(screen.getByRole("button", { name: "保存" })).toBeDisabled();
  fireEvent.change(input, { target: { value: "10080" } });
  expect(props.setPilotScheduleIntervalMinutes).toHaveBeenCalledWith("10080");
  view.rerender(<PilotProviderSchedulePanel {...props} pilotScheduleIntervalMinutes="10080" />);
  expect(screen.getByRole("spinbutton")).toBe(input);
  expect(input).not.toHaveAttribute("aria-describedby");
  expect(screen.getByRole("button", { name: "保存" })).toBeEnabled();
  view.rerender(<PilotProviderSchedulePanel {...props} pilotScheduleIntervalMinutes="1.5" editorLocked />);
  expect(input).toBeDisabled();
  expect(screen.getByRole("button", { name: "取消" })).toBeEnabled();
});
