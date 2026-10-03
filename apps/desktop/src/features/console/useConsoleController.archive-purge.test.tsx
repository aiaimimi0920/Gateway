import { act, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, expect, it, vi } from "vitest";
import { createConsoleApi } from "./BrowserConsoleApp.api-fixture";
import { renderWithProviders } from "./BrowserConsoleApp.render-fixture";
import { useConsoleController } from "./useConsoleController";

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (error: Error) => void;
  const promise = new Promise<T>((yes, no) => { resolve = yes; reject = no; });
  return { promise, resolve, reject };
}
beforeEach(() => window.localStorage.clear());

it.each(["success", "failure"] as const)("locks editing through purge %s and its route refresh", async outcome => {
  const api = createConsoleApi();
  const initial = await api.getRouteConfig("management-secret");
  const purge = deferred<{ purgedCount: number }>();
  const refresh = deferred<typeof initial>();
  vi.mocked(api.getRouteConfig).mockClear().mockResolvedValueOnce(initial).mockReturnValueOnce(refresh.promise);
  vi.mocked(api.purgeCredentialArchive).mockReturnValueOnce(purge.promise);
  function Probe() {
    const controller = useConsoleController(api);
    return <>
      <input aria-label="Route editor" disabled={controller.editorLocked} />
      <button type="button" onClick={() => void controller.handlePurgeCredentialArchive("managed-provider")}>Purge</button>
    </>;
  }
  renderWithProviders(<Probe />);
  const user = userEvent.setup();
  const editor = screen.getByRole("textbox", { name: "Route editor" });
  await waitFor(() => expect(editor).toBeEnabled());
  await user.click(screen.getByRole("button", { name: "Purge" }));
  expect(api.purgeCredentialArchive).toHaveBeenCalledOnce();
  expect(editor).toBeDisabled();
  await user.type(editor, "must not become a draft during purge");
  expect(editor).toHaveValue("");
  await act(async () => {
    if (outcome === "success") purge.resolve({ purgedCount: 1 });
    else purge.reject(new Error("Purge stopped after its durable barrier"));
  });
  await waitFor(() => expect(api.getRouteConfig).toHaveBeenCalledTimes(2));
  expect(editor).toBeDisabled();
  await act(async () => refresh.resolve(initial));
  await waitFor(() => expect(editor).toBeEnabled());
  expect(api.commitRouteConfig).not.toHaveBeenCalled();
});
