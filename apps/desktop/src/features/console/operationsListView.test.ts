import { describe, expect, it } from "vitest";
import { operationsRequestFixture } from "./OperationsWorkspace.fixtures";
import { DEFAULT_OPERATIONS_LIST_VIEW, requestRowMetadata, selectOperationsRows, selectOperationsAnomalyLists } from "./operationsListView";

describe("operations local list view", () => {
  const rows = Object.freeze([
    Object.freeze({ name: "Model 10", time: "2026-10-24T08:00:00+08:00", text: ["failed", "额度不足"] }),
    Object.freeze({ name: "Model 2", time: "2026-10-24T01:00:00Z", text: ["completed"] }),
    Object.freeze({ name: "No date", time: null, text: [] }),
    Object.freeze({ name: "Bad date", time: "invalid", text: [] }),
  ]);
  const names = (sort: "default" | "newest" | "oldest" | "name" | "name-desc", query = "") =>
    selectOperationsRows(rows, { sort, query }, (row) => row).map((row) => row.name);

  it("normalizes whitespace, matches multiple terms and supports Chinese text without mutating input", () => {
    expect(names("default", "  MODEL   额度不足  ")).toEqual(["Model 10"]);
    expect(names("default", "failed completed")).toEqual([]);
    expect(names("default", " \t ")).toEqual(rows.map((row) => row.name));
    expect(rows[0].name).toBe("Model 10");
  });

  it("sorts real instants rather than timestamp spelling, with unknown dates last in both directions", () => {
    expect(names("newest")).toEqual(["Model 2", "Model 10", "No date", "Bad date"]);
    expect(names("oldest")).toEqual(["Model 10", "Model 2", "No date", "Bad date"]);
    expect(names("name")).toEqual(["Bad date", "Model 2", "Model 10", "No date"]);
    expect(names("name-desc")).toEqual(["No date", "Model 10", "Model 2", "Bad date"]);
    expect(names("default")).toEqual(rows.map((row) => row.name));
  });

  it("does not search hidden request payloads, route traces or credential identifiers", () => {
    const request = operationsRequestFixture();
    request.routeTrace = { secret: "hidden-trace" };
    request.apiKeyId = "hidden-key-id";
    request.requestArtifactObjectKey = "hidden-artifact";
    for (const query of ["hidden-trace", "hidden-key-id", "hidden-artifact"]) {
      expect(selectOperationsRows([request], { query, sort: "default" }, requestRowMetadata)).toEqual([]);
    }
    expect(selectOperationsRows([request], { query: "429 LIMITED", sort: "default" }, requestRowMetadata)).toEqual([request]);
  });

  it("preserves queue totals, errors, loading and opaque payloads while filtering detail arrays", () => {
    const empty = { data: null, error: null, loading: false };
    const runs = [
      { id: "b", title: "Beta", createdAt: "2026-10-24T01:00:00Z" },
      { id: "a", title: "Alpha", createdAt: "2026-10-24T02:00:00Z" },
    ].map((run) => ({
      ...run, incidentId: "incident", policyId: null, routePolicyId: null,
      actionKey: "retry", executionMode: "apply", status: "completed", dryRun: false,
      actorUserId: "operator", note: null, input: { private: "not-searchable" }, result: null,
      errorSummary: null, completedAt: null
    }));
    const props = {
      incidents: empty, policies: empty, exports: empty, remediationQueue: empty,
      alertQueue: {
        ...empty, error: "queue stale", loading: true, data: {
          generatedAt: "2026-10-24T01:00:00Z", limit: 100, dueOnly: false, incidentCount: 9, dueCount: 7, items: [],
        }
      },
      remediationRuns: { ...empty, data: Object.freeze(runs) as typeof runs },
    };
    const selected = selectOperationsAnomalyLists(props, { query: "alpha", sort: "newest" });
    expect(selected.remediationRuns.data?.map((row) => row.id)).toEqual(["a"]);
    expect(selected.alertQueue).toEqual(props.alertQueue);
    expect(selectOperationsAnomalyLists(props, { query: "not-searchable", sort: "default" }).remediationRuns.data).toEqual([]);
    expect(selectOperationsAnomalyLists(props, DEFAULT_OPERATIONS_LIST_VIEW).remediationRuns.data).toEqual(runs);
    expect(runs[0].id).toBe("b");
  });
});
