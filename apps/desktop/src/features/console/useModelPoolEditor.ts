import { useCallback, type Dispatch, type SetStateAction } from "react";
import type { ConsoleRouteDocument } from "../../api/contracts";
import type { ModelPoolModelDialogMode, ModelPoolModelDialogValue } from "./ModelPoolModelDialog";
import type { ProviderModelMappingEntry } from "./ProviderModelMappingDialog";
import { parseRouteDocument, isRecord } from "./routeDocument";
import { type ModelRouteDraftRow, createModelRouteDraftRow, rewriteModelReferences, declareModelOnProviders, findModelRoute, highestModelRoutePriority } from "./modelRouteDraft";
import { type buildModelPoolDirectory, modelRoutePriority, moveChainProvider } from "./modelPoolViewModel";

const MODEL_ROUTE_EDITOR_JSON_ERROR =
  "Fix Route document JSON before using the structured model route editor.";

type ModelPoolDialogState = {
  mode: ModelPoolModelDialogMode;
  initial: ModelPoolModelDialogValue | null;
};

type ModelPoolEditorOptions = {
  editorText: string;
  modelRouteDraftRows: ModelRouteDraftRow[];
  setModelRouteDraftRows: Dispatch<SetStateAction<ModelRouteDraftRow[]>>;
  setError: Dispatch<SetStateAction<string | null>>;
  replaceEditorDocument: (document: ConsoleRouteDocument, syncStructuredEditors?: boolean) => void;
  modelPoolDirectory: ReturnType<typeof buildModelPoolDirectory>;
  modelPoolDialog: ModelPoolDialogState | null;
  setModelPoolDialog: Dispatch<SetStateAction<ModelPoolDialogState | null>>;
  providerModelMappingProviderId: string | null;
  setProviderModelMappingProviderId: Dispatch<SetStateAction<string | null>>;
  t: (zh: string, en: string) => string;
};

export function useModelPoolEditor({
  editorText,
  modelRouteDraftRows,
  setModelRouteDraftRows,
  setError,
  replaceEditorDocument,
  modelPoolDirectory,
  modelPoolDialog,
  setModelPoolDialog,
  providerModelMappingProviderId,
  setProviderModelMappingProviderId,
  t,
}: ModelPoolEditorOptions) {
  const applyModelRouteDraftRows = useCallback(
    (nextRows: ModelRouteDraftRow[]) => {
      let document: ConsoleRouteDocument;
      try {
        document = parseRouteDocument(editorText);
      } catch {
        setError(MODEL_ROUTE_EDITOR_JSON_ERROR);
        return;
      }
      setError((current) => (current === MODEL_ROUTE_EDITOR_JSON_ERROR ? null : current));
      document.model_routes = nextRows
        .filter((row) => row.pattern.trim().length > 0)
        .map((row) => ({
          ...row.route,
          pattern: row.pattern.trim(),
        }));
      setModelRouteDraftRows(nextRows);
      replaceEditorDocument(document);
    },
    [editorText, replaceEditorDocument],
  );

  /**
   * Move one provider a slot along a model's fallback chain. The chain lives in
   * the model's own exact-pattern route, so the first reorder pins the inherited
   * order into a route of its own; later reorders just rewrite it.
   */
  const reorderModelPoolChain = useCallback(
    (model: string, providerId: string, direction: "up" | "down") => {
      const card = modelPoolDirectory.find((entry) => entry.model === model);
      if (!card) {
        return;
      }
      const currentOrder = card.chain.map((link) => link.providerId);
      const nextOrder = moveChainProvider(currentOrder, providerId, direction);
      // Already at the head or the tail: nothing to write, so no revision churn.
      if (nextOrder.every((id, index) => id === currentOrder[index])) {
        return;
      }
      const pinnedRow = modelRouteDraftRows.find((row) => row.pattern.trim() === model);
      if (pinnedRow) {
        applyModelRouteDraftRows(
          modelRouteDraftRows.map((row) =>
            row.id === pinnedRow.id
              ? { ...row, route: { ...row.route, provider_ids: nextOrder } }
              : row,
          ),
        );
        return;
      }
      // No route pins this model yet. The gateway resolves matching routes by
      // descending priority, so the new one has to outrank every wildcard route
      // that also matches, or the wildcard's order would still win.
      const highestPriority = modelRouteDraftRows.reduce(
        (highest, row) => Math.max(highest, modelRoutePriority(row.route)),
        0,
      );
      applyModelRouteDraftRows([
        ...modelRouteDraftRows,
        createModelRouteDraftRow(model, {
          pattern: model,
          provider_ids: nextOrder,
          priority: highestPriority + 1,
        }),
      ]);
    },
    [applyModelRouteDraftRows, modelPoolDirectory, modelRouteDraftRows],
  );

  /**
   * Drop the model's own route so its chain goes back to being inherited. Safe
   * even when it was the only matching route: with no match the gateway falls
   * back to every provider that declares the model, which is the same set the
   * card then shows.
   */
  const resetModelPoolChain = useCallback(
    (model: string) => {
      if (!modelRouteDraftRows.some((row) => row.pattern.trim() === model)) {
        return;
      }
      applyModelRouteDraftRows(
        modelRouteDraftRows.filter((row) => row.pattern.trim() !== model),
      );
    },
    [applyModelRouteDraftRows, modelRouteDraftRows],
  );

  /**
   * Stand a model up or down. The flag lives on the model's own route, so a
   * model that never had one gets it here — and a route without providers fails
   * validation, hence the guard.
   */
  const toggleModelPoolEnabled = useCallback(
    (model: string, enabled: boolean) => {
      const pinnedRow = modelRouteDraftRows.find((row) => row.pattern.trim() === model);
      if (pinnedRow) {
        applyModelRouteDraftRows(
          modelRouteDraftRows.map((row) => {
            if (row.id !== pinnedRow.id) {
              return row;
            }
            const route = { ...row.route };
            if (enabled) {
              // The gateway defaults the flag to true, so dropping the key is
              // the same as enabling and keeps the document minimal.
              delete route.enabled;
            } else {
              route.enabled = false;
            }
            return { ...row, route };
          }),
        );
        return;
      }
      if (enabled) {
        return;
      }
      const card = modelPoolDirectory.find((entry) => entry.model === model);
      const providerIds = card?.chain.map((link) => link.providerId) ?? [];
      if (providerIds.length === 0) {
        setError(
          t(
            `模型 ${model} 没有可用服务商，无需停用。`,
            `Model ${model} has no provider to stand down.`,
          ),
        );
        return;
      }
      applyModelRouteDraftRows([
        ...modelRouteDraftRows,
        createModelRouteDraftRow(model, {
          pattern: model,
          provider_ids: providerIds,
          priority:
            modelRouteDraftRows.reduce(
              (highest, row) => Math.max(highest, modelRoutePriority(row.route)),
              0,
            ) + 1,
          enabled: false,
        }),
      ]);
    },
    [applyModelRouteDraftRows, modelPoolDirectory, modelRouteDraftRows, t],
  );

  /**
   * Model-pool writes reach further than the structured route rows: a model also
   * appears in every provider/credential `supported_models`, in the alias table
   * and in a provider's `model_map`. Those go straight at the parsed document.
   */
  const mutateDraftDocument = useCallback(
    (mutate: (document: ConsoleRouteDocument) => void) => {
      let document: ConsoleRouteDocument;
      try {
        document = parseRouteDocument(editorText);
      } catch {
        setError(MODEL_ROUTE_EDITOR_JSON_ERROR);
        return;
      }
      setError((current) => (current === MODEL_ROUTE_EDITOR_JSON_ERROR ? null : current));
      mutate(document);
      replaceEditorDocument(document, true);
    },
    [editorText, replaceEditorDocument],
  );

  const openAddModelDialog = useCallback(() => {
    setModelPoolDialog({ mode: "add", initial: null });
  }, []);

  const openEditModelDialog = useCallback(
    (model: string) => {
      const card = modelPoolDirectory.find((entry) => entry.model === model);
      setModelPoolDialog({
        mode: "edit",
        initial: { model, providerIds: card?.chain.map((link) => link.providerId) ?? [] },
      });
    },
    [modelPoolDirectory],
  );

  const submitModelPoolDialog = useCallback(
    (value: ModelPoolModelDialogValue) => {
      const previousModel = modelPoolDialog?.initial?.model ?? null;
      mutateDraftDocument((document) => {
        const carried = previousModel ? findModelRoute(document, previousModel) : null;
        const carriedPriority = carried ? modelRoutePriority(carried) : 0;
        if (previousModel && previousModel !== value.model) {
          rewriteModelReferences(document, previousModel, value.model);
        }
        const kept = document.model_routes.filter((route) => {
          if (!isRecord(route) || typeof route.pattern !== "string") {
            return true;
          }
          const pattern = route.pattern.trim();
          return pattern !== value.model && pattern !== previousModel;
        });
        const priority =
          carriedPriority > 0 ? carriedPriority : highestModelRoutePriority(document) + 1;
        document.model_routes = [
          ...kept,
          {
            ...(carried ?? {}),
            pattern: value.model,
            provider_ids: [...value.providerIds],
            priority,
          },
        ];
        declareModelOnProviders(document, value.model, value.providerIds);
      });
      setModelPoolDialog(null);
    },
    [modelPoolDialog, mutateDraftDocument],
  );

  /**
   * Retire a model from the pool. Dropping its route is not enough — cards are
   * also derived from what the accounts declare — so every other mention goes
   * with it.
   */
  const deleteModelPoolModel = useCallback(
    (model: string) => {
      mutateDraftDocument((document) => {
        document.model_routes = document.model_routes.filter(
          (route) =>
            !isRecord(route) ||
            typeof route.pattern !== "string" ||
            route.pattern.trim() !== model,
        );
        rewriteModelReferences(document, model, null);
      });
    },
    [mutateDraftDocument],
  );

  /**
   * Rewrite one provider's `model_map`. An empty editor means "no renames", and
   * the key is dropped rather than stored as `{}` so the document keeps matching
   * what the gateway writes itself.
   */
  const submitProviderModelMapping = useCallback(
    (entries: ProviderModelMappingEntry[]) => {
      const providerId = providerModelMappingProviderId;
      if (providerId === null) {
        return;
      }
      mutateDraftDocument((document) => {
        const provider = document.providers.find(
          (candidate) =>
            isRecord(candidate) &&
            typeof candidate.id === "string" &&
            candidate.id.trim() === providerId,
        );
        if (!isRecord(provider)) {
          return;
        }
        if (entries.length === 0) {
          delete provider.model_map;
          return;
        }
        provider.model_map = Object.fromEntries(
          entries.map((entry) => [entry.model, entry.upstreamModel]),
        );
      });
      setProviderModelMappingProviderId(null);
    },
    [mutateDraftDocument, providerModelMappingProviderId],
  );

  return { reorderModelPoolChain, resetModelPoolChain, toggleModelPoolEnabled, openAddModelDialog, openEditModelDialog, submitModelPoolDialog, deleteModelPoolModel, submitProviderModelMapping };
}
