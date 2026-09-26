import { act, renderHook } from "@testing-library/react";
import { expect, it, vi } from "vitest";
import type { ConsoleRouteDocument } from "../../api/contracts";
import { useModelPoolEditor } from "./useModelPoolEditor";
import { isRecord } from "./routeDocument";

it("stores prototype-named model mapping keys as serializable own data", () => {
  const provider = { id: "provider", model_map: { old: "obsolete" } };
  const document: ConsoleRouteDocument = {
    providers: [provider],
    model_routes: [], aliases: {},
  };
  const replace = vi.fn<(document: ConsoleRouteDocument, sync?: boolean) => void>();
  const error = vi.fn();
  const closeMapping = vi.fn();
  const { result } = renderHook(() => useModelPoolEditor({
    editorText: JSON.stringify(document), modelRouteDraftRows: [],
    setModelRouteDraftRows: vi.fn(), setError: error, replaceEditorDocument: replace,
    modelPoolDirectory: [], modelPoolDialog: null, setModelPoolDialog: vi.fn(),
    providerModelMappingProviderId: "provider", setProviderModelMappingProviderId: closeMapping,
    t: (_zh, en) => en,
  }));
  const entries = [
    { model: "__proto__", upstreamModel: "upstream-proto" },
    { model: "constructor", upstreamModel: "upstream-constructor" },
  ];
  Object.freeze(entries);
  act(() => result.current.submitProviderModelMapping(entries));
  const saved = replace.mock.calls[0][0];
  const savedProvider = saved.providers[0];
  if (!isRecord(savedProvider) || !isRecord(savedProvider.model_map)) {
    throw new Error("Expected a provider model map");
  }
  const mapping = savedProvider.model_map;
  expect(Object.hasOwn(mapping, "__proto__")).toBe(true);
  expect(mapping.__proto__).toBe("upstream-proto");
  expect(mapping.constructor).toBe("upstream-constructor");
  expect(Object.getPrototypeOf(mapping)).toBe(Object.prototype);
  expect(JSON.parse(JSON.stringify(mapping))).toEqual(Object.fromEntries(
    entries.map(({ model, upstreamModel }) => [model, upstreamModel]),
  ));
  expect(provider.model_map).toEqual({ old: "obsolete" });
  expect(closeMapping).toHaveBeenCalledWith(null);
});
