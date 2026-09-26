import { render, waitFor } from "@testing-library/react";
import type { ReactNode } from "react";
import { describe, expect, it, vi } from "vitest";

import type { ConsoleApi } from "../../api/console";
import { useOperationsData } from "./useOperationsData";

function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((resolvePromise) => {
    resolve = resolvePromise;
  });
  return { promise, resolve };
}

function OperationsProbe({ api }: { api: ConsoleApi }): ReactNode {
  useOperationsData({ api, managementToken: "management-token", active: true });
  return null;
}

describe("useOperationsData", () => {
  it("starts request panels without waiting for the primary operations batch", async () => {
    type PressureResponse = Awaited<ReturnType<NonNullable<ConsoleApi["getRuntimePressure"]>>>;
    const pressure = deferred<PressureResponse>();
    const getRuntimePressure = vi.fn(() => pressure.promise);
    const listRequestAudits = vi.fn().mockResolvedValue({ requests: [] });
    const api = {
      getRuntimePressure,
      listRequestAudits,
    } as unknown as ConsoleApi;
    const view = render(<OperationsProbe api={api} />);

    try {
      await waitFor(() => expect(getRuntimePressure).toHaveBeenCalledTimes(1));
      expect(listRequestAudits).toHaveBeenCalledTimes(1);
    } finally {
      pressure.resolve({ pressure: {} } as PressureResponse);
      view.unmount();
    }
  });
});
