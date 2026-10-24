import * as Tabs from "@radix-ui/react-tabs";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { useRef, useState } from "react";
import { describe, expect, it } from "vitest";
import { OperationsNavigation } from "./OperationsNavigation";
import { operationsSummaryFixture } from "./OperationsWorkspace.fixtures";
import type {
  OperationsSectionId,
  OperationsWorkspaceProps,
} from "./operations-contracts";

function Navigation({
  operatorSummary,
}: Pick<OperationsWorkspaceProps, "operatorSummary">) {
  const [section, setSection] = useState<OperationsSectionId>("live");
  const ref = useRef<HTMLButtonElement>(null);
  return (
    <Tabs.Root
      value={section}
      onValueChange={(value) => setSection(value as OperationsSectionId)}
    >
      <OperationsNavigation
        t={(_, en) => en}
        operatorSummary={operatorSummary}
        incidentSummary={{ data: null, error: null, loading: false }}
        requestsTabRef={ref}
      />
      {(["live", "requests", "anomalies"] as const).map((id) => (
        <Tabs.Content key={id} value={id}>
          {id} panel
        </Tabs.Content>
      ))}
    </Tabs.Root>
  );
}

describe("operations task navigation", () => {
  it("supports arrows, Home and End with one active panel and a labelled destination", async () => {
    const user = userEvent.setup();
    render(
      <Navigation
        operatorSummary={{
          data: operationsSummaryFixture(false),
          error: null,
          loading: false,
        }}
      />,
    );
    await user.tab();
    expect(screen.getByRole("tab", { name: "Overview" })).toHaveFocus();
    await user.keyboard("{ArrowRight}");
    expect(screen.getByRole("tab", { name: "Request log" })).toHaveFocus();
    expect(
      screen.getByRole("tabpanel", { name: "Request log" }),
    ).toHaveTextContent("requests panel");
    expect(screen.getAllByRole("tabpanel")).toHaveLength(1);
    await user.keyboard("{End}");
    expect(
      screen.getByRole("tab", { name: "Incident management" }),
    ).toHaveAttribute("aria-selected", "true");
    await user.keyboard("{Home}");
    expect(screen.getByRole("tab", { name: "Overview" })).toHaveAttribute(
      "aria-selected",
      "true",
    );
  });

  it("omits redundant descriptions, capability chips and snapshot text in every state", () => {
    const data = operationsSummaryFixture(false);
    const { rerender } = render(
      <Navigation operatorSummary={{ data, error: null, loading: false }} />,
    );
    const expectCompactHeader = () => {
      expect(document.querySelector(".nt-operations-header")).not.toBeInTheDocument();
      expect(screen.queryByText(/Core operations|Summary captured|Capabilities unknown|Summary unavailable|Reading data/)).not.toBeInTheDocument();
      expect(screen.queryByText(/Check runtime health/)).not.toBeInTheDocument();
      expect(screen.getAllByRole("tab")).toHaveLength(3);
    };
    expectCompactHeader();
    rerender(
      <Navigation
        operatorSummary={{ data, error: "read failed", loading: false }}
      />,
    );
    expectCompactHeader();
    rerender(
      <Navigation
        operatorSummary={{ data: null, error: null, loading: true }}
      />,
    );
    expectCompactHeader();
  });
});
