import "@testing-library/jest-dom/vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { useState } from "react";
import { describe, expect, it } from "vitest";
import { GeminiManualAddDialog } from "./GeminiManualAddDialog";

function Harness() {
  const [open, setOpen] = useState(false);
  return (
    <>
      <button onClick={() => setOpen(true)}>Open auth</button>
      <button>Outside action</button>
      <GeminiManualAddDialog
        geminiManualAddDialogState={open ? {
          targetFamily: "gemini-canvas",
          providerId: "canvas-account",
          session: null,
          busy: true,
        } : null}
        closeGeminiManualAddDialog={() => setOpen(false)}
        requestGeminiManualAddCompletion={async () => {}}
        t={(_zh, en) => en}
      />
    </>
  );
}

describe("Gemini manual auth keyboard lifecycle", () => {
  it("focuses the dialog and keeps tab navigation inside it", async () => {
    const user = userEvent.setup();
    render(<Harness />);
    await user.click(screen.getByRole("button", { name: "Open auth" }));
    const close = screen.getByRole("button", { name: "Close" });
    await waitFor(() => expect(close).toHaveFocus());
    await user.tab();
    expect(close).toHaveFocus();
    await user.tab({ shift: true });
    expect(close).toHaveFocus();
  });

  it("closes with Escape and restores focus to the launching control", async () => {
    const user = userEvent.setup();
    render(<Harness />);
    const opener = screen.getByRole("button", { name: "Open auth" });
    await user.click(opener);
    await user.keyboard("{Escape}");
    await waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());
    await waitFor(() => expect(opener).toHaveFocus());
  });

  it("exposes auth progress as a polite live status", async () => {
    const user = userEvent.setup();
    render(<Harness />);
    await user.click(screen.getByRole("button", { name: "Open auth" }));
    expect(screen.getByRole("status")).toHaveTextContent("Starting the web sign-in helper.");
  });

  it("shows AI Studio instructions without offering the Canvas completion signal", () => {
    render(<GeminiManualAddDialog geminiManualAddDialogState={{ targetFamily: "aistudio-web", providerId: "pool", busy: false,
      session: { id: "session", targetFamily: "aistudio-web", providerId: "pool", status: "waiting_user",
        createdAt: "2026-11-01T00:00:00Z", updatedAt: "2026-11-01T00:00:00Z", generatedDrafts: [],
        message: "Close the dedicated window after signing in." } }} closeGeminiManualAddDialog={() => {}}
      requestGeminiManualAddCompletion={async () => {}} t={(_zh, en) => en} />);
    expect(screen.getByRole("dialog", { name: "AI Studio account import" })).toBeInTheDocument();
    expect(screen.getByRole("status")).toHaveTextContent("Close the dedicated window after signing in.");
    expect(screen.getAllByRole("button")).toHaveLength(1);
  });

  it("shows JSON progress without claiming to launch a browser", () => {
    render(<GeminiManualAddDialog geminiManualAddDialogState={{ targetFamily: "aistudio-web", providerId: "pool", busy: true,
      importMethod: "json", session: null }} closeGeminiManualAddDialog={() => {}}
      requestGeminiManualAddCompletion={async () => {}} t={(_zh, en) => en} />);
    expect(screen.getByRole("status")).toHaveTextContent("Parsing web session JSON.");
    expect(screen.queryByText("Starting the web sign-in helper.")).not.toBeInTheDocument();
  });

  it.each(["button", "overlay"])("closes from %s and restores focus", async (target) => {
    const user = userEvent.setup();
    render(<Harness />);
    const opener = screen.getByRole("button", { name: "Open auth" });
    await user.click(opener);
    const dismiss = target === "button"
      ? screen.getByRole("button", { name: "Close" })
      : document.querySelector(".dialog-overlay");
    expect(dismiss).not.toBeNull();
    await user.click(dismiss as Element);
    await waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());
    await waitFor(() => expect(opener).toHaveFocus());
  });
});
