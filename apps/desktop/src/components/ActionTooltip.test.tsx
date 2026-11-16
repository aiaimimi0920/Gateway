import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterAll, beforeAll, describe, expect, it, vi } from "vitest";

import { ActionTooltip, NeuroProvider } from "@neuro/ui";

beforeAll(() => {
  vi.stubGlobal(
    "ResizeObserver",
    class {
      observe() {}
      unobserve() {}
      disconnect() {}
    },
  );
});

afterAll(() => {
  vi.unstubAllGlobals();
});

describe("ActionTooltip", () => {
  it("shows the action label without changing the trigger accessible name", async () => {
    const user = userEvent.setup();

    render(
      <NeuroProvider>
        <ActionTooltip label="Edit account Alpha">
          <button type="button" aria-label="Edit account Alpha">
            Edit
          </button>
        </ActionTooltip>
      </NeuroProvider>,
    );

    const trigger = screen.getByRole("button", { name: "Edit account Alpha" });
    await user.hover(trigger);

    expect(await screen.findByRole("tooltip")).toHaveTextContent("Edit account Alpha");
    expect(trigger).toHaveAccessibleName("Edit account Alpha");
  });
});
