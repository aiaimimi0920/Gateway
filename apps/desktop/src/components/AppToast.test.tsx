import { act, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import { AppToastViewport, pushAppToast } from "./AppToast";
import { NeuroProvider } from "@neuro/ui";

afterEach(() => {
  vi.useRealTimers();
});

describe("AppToastViewport", () => {
  it("portals the notification viewport inside the current theme", () => {
    render(<NeuroProvider mode="light"><AppToastViewport /></NeuroProvider>);

    const viewport = screen.getByLabelText("Gateway notifications");
    expect(viewport.parentElement).toHaveClass("n-portal-host");
    expect(viewport.closest("[data-neuro-mode]")).toHaveAttribute("data-neuro-mode", "light");
  });

  it("stacks notifications and dismisses one when clicked", () => {
    vi.useFakeTimers();
    render(<NeuroProvider><AppToastViewport /></NeuroProvider>);

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
    render(<NeuroProvider><AppToastViewport /></NeuroProvider>);

    act(() => pushAppToast("info", "Short-lived information"));
    act(() => vi.advanceTimersByTime(3_420));

    expect(screen.queryByText("Short-lived information")).not.toBeInTheDocument();
  });

  it("uses assertive alerts for errors and preserves the longer error lifetime", () => {
    vi.useFakeTimers();
    render(<NeuroProvider><AppToastViewport /></NeuroProvider>);

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
