import * as Dialog from "@radix-ui/react-dialog";
import { ArrowDown, ArrowUp } from "lucide-react";
import { type FormEvent, useEffect, useState } from "react";

import { useUiLocale } from "../../i18n/UiLocaleProvider";

export type ModelPoolModelDialogMode = "add" | "edit";

export type ModelPoolModelDialogProviderOption = {
  id: string;
  label: string;
};

export type ModelPoolModelDialogValue = {
  model: string;
  /** Providers in dispatch order — index 0 is tried first. */
  providerIds: string[];
};

export type ModelPoolModelDialogProps = {
  open: boolean;
  mode: ModelPoolModelDialogMode;
  providerOptions: ModelPoolModelDialogProviderOption[];
  /** Every model the pool already holds, so a rename cannot collide with one. */
  existingModels: string[];
  initialValue?: ModelPoolModelDialogValue | null;
  locked: boolean;
  onOpenChange(open: boolean): void;
  onSubmit(value: ModelPoolModelDialogValue): void;
};

const EMPTY_VALUE: ModelPoolModelDialogValue = { model: "", providerIds: [] };

/**
 * Adds or edits one model of the pool. A model is a `model_routes` entry whose
 * pattern is the model name, so the dialog edits exactly two things: the name
 * and the ordered provider chain behind it.
 */
export function ModelPoolModelDialog({
  open,
  mode,
  providerOptions,
  existingModels,
  initialValue,
  locked,
  onOpenChange,
  onSubmit,
}: ModelPoolModelDialogProps) {
  const { t } = useUiLocale();
  const [value, setValue] = useState<ModelPoolModelDialogValue>(() => initialValue ?? EMPTY_VALUE);
  const [validationError, setValidationError] = useState<string | null>(null);

  useEffect(() => {
    if (!open) {
      return;
    }
    setValue(initialValue ?? EMPTY_VALUE);
    setValidationError(null);
  }, [initialValue, open]);

  const optionLabel = (providerId: string): string =>
    providerOptions.find((option) => option.id === providerId)?.label ?? providerId;

  const toggleProvider = (providerId: string) => {
    setValue((current) => ({
      ...current,
      providerIds: current.providerIds.includes(providerId)
        ? current.providerIds.filter((candidate) => candidate !== providerId)
        : [...current.providerIds, providerId],
    }));
  };

  const moveProvider = (providerId: string, direction: "up" | "down") => {
    setValue((current) => {
      const index = current.providerIds.indexOf(providerId);
      const target = direction === "up" ? index - 1 : index + 1;
      if (index < 0 || target < 0 || target >= current.providerIds.length) {
        return current;
      }
      const providerIds = [...current.providerIds];
      providerIds[index] = providerIds[target];
      providerIds[target] = providerId;
      return { ...current, providerIds };
    });
  };

  const submit = (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    const model = value.model.trim();
    if (model.length === 0) {
      setValidationError(t("模型名称不能为空。", "Model name is required."));
      return;
    }
    if (model.includes("*")) {
      setValidationError(
        t(
          "模型名称不能包含 *，通配规则请在路由中配置。",
          "A model name cannot contain *; configure wildcard patterns as routes.",
        ),
      );
      return;
    }
    const renamed = mode === "add" || model !== (initialValue?.model ?? "");
    if (renamed && existingModels.some((existing) => existing.trim() === model)) {
      setValidationError(t(`模型 ${model} 已存在。`, `Model ${model} already exists.`));
      return;
    }
    if (value.providerIds.length === 0) {
      setValidationError(t("至少选择一个服务商。", "Select at least one provider."));
      return;
    }
    onSubmit({ model, providerIds: value.providerIds });
    onOpenChange(false);
  };

  const unselected = providerOptions.filter((option) => !value.providerIds.includes(option.id));

  return (
    <Dialog.Root open={open} onOpenChange={onOpenChange}>
      <Dialog.Portal>
        <Dialog.Overlay className="dialog-overlay" />
        <Dialog.Content
          className="dialog-content nt-model-dialog"
          aria-describedby="model-pool-dialog-description"
        >
          <Dialog.Title>
            {mode === "add" ? t("添加模型", "Add model") : t("编辑模型", "Edit model")}
          </Dialog.Title>
          <Dialog.Description id="model-pool-dialog-description">
            {t(
              "模型以 model_routes 规则的形式写入草稿：名称即匹配的模型，服务商顺序即调用顺序。",
              "The model is written to the draft as a model_routes rule: the name is the matched model and the provider order is the dispatch order.",
            )}
          </Dialog.Description>

          <form className="nt-stack" onSubmit={submit}>
            <label className="nt-field">
              <span>{t("模型名称", "Model name")}</span>
              <input
                className="nt-input"
                value={value.model}
                disabled={locked}
                placeholder="deepseek-chat"
                onChange={(event) => {
                  const model = event.currentTarget.value;
                  setValue((current) => ({ ...current, model }));
                }}
              />
            </label>

            <section className="nt-card nt-card--panel nt-model-dialog__chain-panel">
              <header className="nt-model-dialog__chain-head">
                <strong>{t("服务商优先级", "Provider priority")}</strong>
                <span className="nt-model-dialog__chain-count">
                  {value.providerIds.length}/{providerOptions.length}
                </span>
              </header>
              {value.providerIds.length > 0 ? (
                <ol className="nt-model-dialog__chain">
                  {value.providerIds.map((providerId, index) => (
                    <li className="nt-model-dialog__chain-row" key={`selected:${providerId}`}>
                      <span className="nt-model-dialog__chain-order" aria-hidden="true">
                        {index + 1}
                      </span>
                      <strong className="nt-model-dialog__chain-label">
                        {optionLabel(providerId)}
                      </strong>
                      <span className="nt-model-dialog__chain-id">{providerId}</span>
                      <button
                        className="nt-icon-action"
                        type="button"
                        disabled={locked || index === 0}
                        aria-label={t(
                          `将 ${optionLabel(providerId)} 上移一位`,
                          `Move ${optionLabel(providerId)} up one slot`,
                        )}
                        onClick={() => moveProvider(providerId, "up")}
                      >
                        <ArrowUp size={13} aria-hidden="true" />
                      </button>
                      <button
                        className="nt-icon-action"
                        type="button"
                        disabled={locked || index === value.providerIds.length - 1}
                        aria-label={t(
                          `将 ${optionLabel(providerId)} 下移一位`,
                          `Move ${optionLabel(providerId)} down one slot`,
                        )}
                        onClick={() => moveProvider(providerId, "down")}
                      >
                        <ArrowDown size={13} aria-hidden="true" />
                      </button>
                      <button
                        className="nt-btn nt-btn--outline nt-btn--compact"
                        type="button"
                        disabled={locked}
                        onClick={() => toggleProvider(providerId)}
                      >
                        {t("移除", "Remove")}
                      </button>
                    </li>
                  ))}
                </ol>
              ) : (
                <p className="nt-copy">
                  {t("还没有选择服务商。", "No provider selected yet.")}
                </p>
              )}
              {unselected.length > 0 ? (
                <div className="nt-model-dialog__pool">
                  {unselected.map((option) => (
                    <button
                      className="nt-btn nt-btn--outline nt-btn--compact"
                      type="button"
                      key={`pool:${option.id}`}
                      disabled={locked}
                      title={option.id}
                      onClick={() => toggleProvider(option.id)}
                    >
                      + {option.label}
                    </button>
                  ))}
                </div>
              ) : null}
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
                {mode === "add" ? t("添加模型", "Add model") : t("保存模型", "Save model")}
              </button>
            </div>
          </form>
        </Dialog.Content>
      </Dialog.Portal>
    </Dialog.Root>
  );
}
