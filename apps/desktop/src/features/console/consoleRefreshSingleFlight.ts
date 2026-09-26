import type { ConsoleApi } from "../../api/console";

type ActiveRefresh = {
  api: ConsoleApi;
  managementToken: string;
  promise: Promise<void>;
};

/** Coalesces duplicate console refreshes without merging different sessions. */
export class ConsoleRefreshSingleFlight {
  private active: ActiveRefresh | null = null;

  run(
    api: ConsoleApi,
    managementToken: string,
    operation: () => Promise<void>,
  ): Promise<void> {
    if (this.active?.api === api && this.active.managementToken === managementToken) {
      return this.active.promise;
    }

    const promise = Promise.resolve().then(operation);
    const active = { api, managementToken, promise };
    this.active = active;
    void promise.then(
      () => this.clear(active),
      () => this.clear(active),
    );
    return promise;
  }

  private clear(active: ActiveRefresh): void {
    if (this.active === active) {
      this.active = null;
    }
  }
}
