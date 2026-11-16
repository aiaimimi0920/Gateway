import { fireEvent, screen } from "@testing-library/react";
import { renderNeuro as render } from "../../test/renderNeuro";
import userEvent from "@testing-library/user-event";
import { useState } from "react";
import { describe, expect, it, vi } from "vitest";
import { OperationsRequestFilters } from "./OperationsRequestFilters";
import {
  OPERATIONS_DEFAULT_REQUEST_FILTERS,
  type OperationsRequestFilterDraft,
} from "./operations-contracts";

function Filters({
  apply,
  locked = false,
  loading = false,
  initial = OPERATIONS_DEFAULT_REQUEST_FILTERS,
}: {
  apply: () => void;
  locked?: boolean;
  loading?: boolean;
  initial?: OperationsRequestFilterDraft;
}) {
  const [draft, setDraft] = useState(initial);
  return (
    <OperationsRequestFilters
      t={(_, en) => en}
      editorLocked={locked}
      requests={{ data: null, error: null, loading }}
      requestFilters={draft}
      onRequestFiltersChange={setDraft}
      onApplyRequestFilters={apply}
      patchFilters={(patch) =>
        setDraft((current) => ({ ...current, ...patch }))
      }
    />
  );
}

describe("operations request filter form", () => {
  it("keeps uncommon fields accessible and accepts custom statuses without submitting on edit", async () => {
    const apply = vi.fn();
    const user = userEvent.setup();
    render(<Filters apply={apply} />);
    expect(screen.queryByLabelText("Project")).not.toBeInTheDocument();
    await user.type(screen.getByLabelText("Status"), "custom-status");
    expect(screen.getByLabelText("Status")).toHaveValue("custom-status");
    await user.click(screen.getByRole("button", { name: "More filters" }));
    await user.type(screen.getByLabelText("Project"), "project-a");
    expect(screen.getByLabelText("Project")).toHaveValue("project-a");
    expect(apply).not.toHaveBeenCalled();
    await user.clear(screen.getByLabelText("Project"));
    expect(screen.getByLabelText("Project")).toBeVisible();
    expect(screen.getByLabelText("Project")).toHaveFocus();
    await user.type(screen.getByLabelText("Project"), "project-b{Enter}");
    expect(apply).toHaveBeenCalledOnce();
  });

  it("counts populated advanced fields and resets the draft without issuing a query", async () => {
    const apply = vi.fn();
    const user = userEvent.setup();
    render(
      <Filters
        apply={apply}
        initial={{
          ...OPERATIONS_DEFAULT_REQUEST_FILTERS,
          errorCode: "upstream_error",
          providerAccountId: "provider-a",
        }}
      />,
    );
    expect(screen.getByRole("button", { name: /More filters/ })).toHaveTextContent("2");
    await user.click(screen.getByRole("button", { name: /More filters/ }));
    expect(screen.getByLabelText("Error code")).toHaveValue("upstream_error");
    await user.keyboard("{Escape}");
    await user.click(screen.getByRole("button", { name: "Reset" }));
    expect(screen.getByLabelText("Limit")).toHaveValue("100");
    await user.click(screen.getByRole("button", { name: /More filters/ }));
    expect(screen.getByLabelText("Error code")).toHaveValue("");
    expect(screen.getByLabelText("Provider account")).toHaveValue("");
    expect(apply).not.toHaveBeenCalled();
  });

  it("blocks form submission while locked or loading, not just button clicks", () => {
    const apply = vi.fn();
    const { rerender } = render(<Filters apply={apply} locked />);
    fireEvent.submit(screen.getByRole("form", { name: "Request filters" }));
    expect(screen.getByRole("button", { name: "Reset" })).toBeDisabled();
    rerender(<Filters apply={apply} loading />);
    fireEvent.submit(screen.getByRole("form", { name: "Request filters" }));
    expect(screen.getByRole("button", { name: "Querying…" })).toBeDisabled();
    expect(apply).not.toHaveBeenCalled();
  });
});
