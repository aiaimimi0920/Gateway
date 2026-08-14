import { act, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import { AppToastViewport, pushAppToast } from "./AppToast";

afterEach(() => {
  vi.useRealTimers();
});

describe("AppToastViewport", () => {
  it("portals the notification viewport to the document body", () => {
    render(<AppToastViewport />);

    const viewport = screen.getByLabelText("Gateway notifications");
    expect(viewport.parentElement).toBe(document.body);
  });

  it("stacks notifications and dismisses one when clicked", () => {
    vi.useFakeTimers();
    render(<AppToastViewport />);

    act(() => {
      pushAppToast("success", "Saved active revision");
      pushAppToast("info", "Draft updated");
    });

    expect(screen.getAllByRole("status")).toHaveLength(2);
    fireEvent.click(screen.getByRole("status", { name: /Saved active revision/ }));
    act(() => vi.advanceTimersByTime(220));

    expect(screen.queryByText("Saved active revision")).not.toBeInTheDocument();
    expect(screen.getByText("Draft updated")).toBeInTheDocument();
  });

  it("automatically removes informational notifications", () => {
    vi.useFakeTimers();
    render(<AppToastViewport />);

    act(() => pushAppToast("info", "Short-lived information"));
    act(() => vi.advanceTimersByTime(3_420));

    expect(screen.queryByText("Short-lived information")).not.toBeInTheDocument();
  });

  it("uses assertive alerts for errors and preserves the longer error lifetime", () => {
    vi.useFakeTimers();
    render(<AppToastViewport />);

    act(() => {
      pushAppToast("success", "Saved successfully");
      pushAppToast("warning", "Driver is not configured");
      pushAppToast("error", "Automation failed");
    });

    expect(screen.getByRole("alert", { name: /Automation failed/ })).toHaveAttribute(
      "aria-live",
      "assertive",
    );
    act(() => vi.advanceTimersByTime(3_420));
    expect(screen.queryByText("Saved successfully")).not.toBeInTheDocument();
    expect(screen.getByText("Driver is not configured")).toBeInTheDocument();
    expect(screen.getByText("Automation failed")).toBeInTheDocument();

    act(() => vi.advanceTimersByTime(1_000));
    expect(screen.queryByText("Driver is not configured")).not.toBeInTheDocument();
    expect(screen.getByText("Automation failed")).toBeInTheDocument();

    act(() => vi.advanceTimersByTime(1_000));
    expect(screen.queryByText("Automation failed")).not.toBeInTheDocument();
  });
});
