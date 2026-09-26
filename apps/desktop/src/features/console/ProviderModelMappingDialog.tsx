import * as Dialog from "@radix-ui/react-dialog";
import { ArrowRight, Trash2 } from "lucide-react";
import { type FormEvent, useEffect, useRef, useState } from "react";

import { useUiLocale } from "../../i18n/UiLocaleProvider";

export type ProviderModelMappingEntry = {
  /** The model name the caller sends, which is also the pool-wide model name. */
  model: string;
  /** The name this provider expects upstream, e.g. `deepseek-mass` for `deepseek`. */
  upstreamModel: string;
};

type MappingRow = ProviderModelMappingEntry & { rowId: string };

export type ProviderModelMappingDialogProps = {
  open: boolean;
  providerLabel: string;
  providerId: string;
  /** Models worth suggesting: what the provider declares plus what the pool holds. */
  modelOptions: string[];
  initialEntries: ProviderModelMappingEntry[];
  locked: boolean;
  onOpenChange(open: boolean): void;
  onSubmit(entries: ProviderModelMappingEntry[]): void;
};

/**
 * Edits one provider's `model_map`. The gateway keeps the requested model name
 * as the pool-wide identity and rewrites it to the upstream name only while
 * calling this provider, so the same `deepseek` card can reach a provider that
 * publishes the model as `deepseek-mass`.
 */
export function ProviderModelMappingDialog({
  open,
  providerLabel,
  providerId,
  modelOptions,
  initialEntries,
  locked,
  onOpenChange,
  onSubmit,
}: ProviderModelMappingDialogProps) {
  const { t } = useUiLocale();
  const rowIdCounter = useRef(0);
  const nextRowId = () => {
    rowIdCounter.current += 1;
    return `map-${rowIdCounter.current}`;
  };
  const [rows, setRows] = useState<MappingRow[]>([]);
  const [validationError, setValidationError] = useState<string | null>(null);

  useEffect(() => {
    if (!open) {
      return;
    }
    setRows(initialEntries.map((entry) => ({ ...entry, rowId: nextRowId() })));
    setValidationError(null);
    // The row ids are generated per open, so the counter itself is not a dep.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [initialEntries, open]);

  const datalistId = `provider-model-map-options:${providerId}`;

  const updateRow = (rowId: string, field: "model" | "upstreamModel", value: string) => {
    setRows((current) =>
      current.map((row) => (row.rowId === rowId ? { ...row, [field]: value } : row)),
    );
  };

  const removeRow = (rowId: string) => {
    setRows((current) => current.filter((row) => row.rowId !== rowId));
  };

  const addRow = (model = "", upstreamModel = "") => {
    setRows((current) => [...current, { rowId: nextRowId(), model, upstreamModel }]);
  };

  const submit = (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    const entries: ProviderModelMappingEntry[] = [];
    const seen = new Set<string>();
    for (const row of rows) {
      const model = row.model.trim();
      const upstreamModel = row.upstreamModel.trim();
      if (model.length === 0 && upstreamModel.length === 0) {
        continue;
      }
      if (model.length === 0) {
        setValidationError(
          t(
            `请填写 ${upstreamModel} 对应的用户模型名。`,
            `Fill in the requested model name that maps to ${upstreamModel}.`,
          ),
        );
        return;
      }
      if (upstreamModel.length === 0) {
        setValidationError(
          t(
            `请填写 ${model} 在该服务商的实际模型名。`,
            `Fill in the upstream model name this provider uses for ${model}.`,
          ),
        );
        return;
      }
      if (model.includes("*") || upstreamModel.includes("*")) {
        setValidationError(
          t("模型名不能包含 *，映射只支持精确名称。", "Model names cannot contain *; the mapping is exact-name only."),
        );
        return;
      }
      if (seen.has(model)) {
        setValidationError(t(`用户模型 ${model} 重复。`, `Requested model ${model} is listed twice.`));
        return;
      }
      seen.add(model);
      entries.push({ model, upstreamModel });
    }
    onSubmit(entries);
    onOpenChange(false);
  };

  const mapped = new Set(rows.map((row) => row.model.trim()).filter((model) => model.length > 0));
  const unmapped = modelOptions.filter((model) => !mapped.has(model));

  return (
    <Dialog.Root open={open} onOpenChange={onOpenChange}>
      <Dialog.Portal>
        <Dialog.Overlay className="dialog-overlay" />
        <Dialog.Content
          className="dialog-content nt-model-map-dialog"
          aria-describedby="provider-model-map-description"
        >
          <Dialog.Title>
            {t(`模型映射 · ${providerLabel}`, `Model mapping · ${providerLabel}`)}
          </Dialog.Title>
          <Dialog.Description id="provider-model-map-description">
            {t(
              "用户发送的模型名保持不变，网关调用该服务商时替换成右侧的实际模型名。没有列出的模型按原名直接发送。",
              "The model name the caller sends stays unchanged; the gateway swaps in the upstream name on the right when it calls this provider. Models that are not listed are sent as-is.",
            )}
          </Dialog.Description>

          <form className="nt-stack" onSubmit={submit}>
            <datalist id={datalistId}>
              {modelOptions.map((model) => (
                <option value={model} key={`option:${model}`} />
              ))}
            </datalist>

            <section className="nt-card nt-card--panel nt-model-map__panel">
              <header className="nt-model-map__head">
                <strong>{t("映射关系", "Mappings")}</strong>
                <span className="nt-model-map__count">{rows.length}</span>
              </header>

              {rows.length > 0 ? (
                <div className="nt-model-map__list">
                  <div className="nt-model-map__row nt-model-map__row--head">
                    <span>{t("用户模型", "Requested model")}</span>
                    <span aria-hidden="true" />
                    <span>{t("服务商模型", "Upstream model")}</span>
                    <span aria-hidden="true" />
                  </div>
                  {rows.map((row) => (
                    <div className="nt-model-map__row" key={row.rowId}>
                      <input
                        className="nt-input"
                        value={row.model}
                        disabled={locked}
                        list={datalistId}
                        placeholder="deepseek"
                        aria-label={t("用户模型", "Requested model")}
                        onChange={(event) =>
                          updateRow(row.rowId, "model", event.currentTarget.value)
                        }
                      />
                      <ArrowRight
                        className="nt-model-map__arrow"
                        size={14}
                        aria-hidden="true"
                      />
                      <input
                        className="nt-input"
                        value={row.upstreamModel}
                        disabled={locked}
                        placeholder="deepseek-mass"
                        aria-label={t("服务商模型", "Upstream model")}
                        onChange={(event) =>
                          updateRow(row.rowId, "upstreamModel", event.currentTarget.value)
                        }
                      />
                      <button
                        className="nt-icon-action"
                        type="button"
                        disabled={locked}
                        aria-label={t(
                          `删除 ${row.model || t("空", "empty")} 的映射`,
                          `Remove the mapping for ${row.model || "the empty row"}`,
                        )}
                        title={t("删除映射", "Remove mapping")}
                        onClick={() => removeRow(row.rowId)}
                      >
                        <Trash2 size={13} aria-hidden="true" />
                      </button>
                    </div>
                  ))}
                </div>
              ) : (
                <p className="nt-copy">
                  {t(
                    "还没有映射，该服务商按原模型名调用。",
                    "No mapping yet, so this provider is called with the original model name.",
                  )}
                </p>
              )}

              <div className="nt-model-map__tools">
                <button
                  className="nt-btn nt-btn--outline nt-btn--compact"
                  type="button"
                  disabled={locked}
                  onClick={() => addRow()}
                >
                  {t("添加映射", "Add mapping")}
                </button>
                {unmapped.map((model) => (
                  <button
                    className="nt-btn nt-btn--outline nt-btn--compact"
                    type="button"
                    key={`unmapped:${model}`}
                    disabled={locked}
                    title={t(`为 ${model} 添加映射`, `Add a mapping for ${model}`)}
                    onClick={() => addRow(model, model)}
                  >
                    + {model}
                  </button>
                ))}
              </div>
            </section>

            {validationError ? (
              <div className="nt-validation-list nt-validation-list--warning">
                <strong>{t("请检查输入", "Check the input")}</strong>
                <ul>
                  <li>{validationError}</li>
                </ul>
              </div>
            ) : null}

            <div className="dialog-actions">
              <Dialog.Close asChild>
                <button type="button">{t("取消", "Cancel")}</button>
              </Dialog.Close>
              <button className="nt-btn nt-btn--primary" type="submit" disabled={locked}>
                {t("保存映射", "Save mapping")}
              </button>
            </div>
          </form>
        </Dialog.Content>
      </Dialog.Portal>
    </Dialog.Root>
  );
}
