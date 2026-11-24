import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { expect, it } from "vitest";
import { NeuroProvider } from "@neuro/ui";
import { OperationsRequestTable, requestOrigin } from "./OperationsRequestTable";
import { operationsRequestFixture } from "./OperationsWorkspace.fixtures";

it("shows exact rejected accounts without claiming that a gateway rejection reached upstream", async () => {
  const row = { ...operationsRequestFixture(), providerAccountId: null, upstreamStatus: null,
    errorSummary: "Authorized upstream accounts are disabled or temporarily unavailable",
    routeTrace: { routeAttemptCount: 0, errorCode: "authorized_candidates_unavailable", rejectedCandidates: [
      { providerAccountId: "a", providerLabel: "Provider A", realCredentialRef: "account-a", reason: "account_disabled" },
      { providerAccountId: "b", providerLabel: "Provider B", realCredentialRef: "account-b", reason: "model_not_supported" },
    ] } };
  render(<NeuroProvider><OperationsRequestTable rows={[row]} t={(_, en) => en} /></NeuroProvider>);
  expect(screen.getByText("Provider A")).toBeVisible();
  expect(screen.getByText("Account: account-a · Account disabled")).toBeVisible();
  expect(screen.getByText("Account: account-b · Model not configured for account")).toBeVisible();
  expect(screen.getByText("Not sent upstream")).toBeVisible();
  await userEvent.setup().click(screen.getByRole("button", { name: `Request details: ${row.id}` }));
  const details = within(screen.getByRole("region", { name: "Request details" }));
  expect(details.getByText("Gateway error")).toBeVisible();
  expect(details.queryByText("Upstream response")).not.toBeInTheDocument();
  expect(details.queryByText("Attempted providers")).not.toBeInTheDocument();
});

it("retains the actual failed account ahead of rejected alternatives and tolerates historical records", () => {
  const row = { ...operationsRequestFixture(), providerAccountId: "actual", routeTrace: {
    routeAttemptCount: 1, realCredentialRef: "failed-account", selectedCandidate: { providerLabel: "Actual provider" },
    rejectedCandidates: [{ providerLabel: "Skipped", realCredentialRef: "other", reason: "account_disabled" }, null, { providerLabel: "invalid" }],
  } };
  expect(requestOrigin(row)).toMatchObject({ provider: "Actual provider", account: "failed-account", noSend: false });
  expect(requestOrigin(row).rejected).toHaveLength(1);
  expect(requestOrigin({ ...row, routeTrace: null })).toMatchObject({ provider: "actual", account: null, rejected: [] });
  expect(requestOrigin({ ...row, providerAccountId: null, routeTrace: { routeAttemptCount: 0 } })).toMatchObject({ provider: null, account: null, rejected: [], noSend: true });
});
