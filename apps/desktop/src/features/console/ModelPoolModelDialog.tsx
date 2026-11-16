import { Button, Checkbox, Dialog, Field, IconButton, Input, MultiSelect, Notice, type DialogProps } from "@neuro/ui";
import { ArrowDown, ArrowUp } from "lucide-react";
import { type FormEvent, useEffect, useId, useRef, useState } from "react";

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
  resolveReturnFocus?: DialogProps["resolveReturnFocus"];
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
  resolveReturnFocus,
}: ModelPoolModelDialogProps) {
  const { t } = useUiLocale();
  const [value, setValue] = useState<ModelPoolModelDialogValue>(() => initialValue ?? EMPTY_VALUE);
  const [validationError, setValidationError] = useState<string | null>(null);
  const [sessionProviderIds, setSessionProviderIds] = useState(initialValue?.providerIds ?? []);
  const nameRef = useRef<HTMLInputElement>(null);
  const formId = useId();

  useEffect(() => {
    if (!open) {
      return;
    }
    setValue(initialValue ?? EMPTY_VALUE);
    setSessionProviderIds(initialValue?.providerIds ?? []);
    setValidationError(null);
  }, [initialValue, open]);

  const optionLabel = (providerId: string): string =>
    providerOptions.find((option) => option.id === providerId)?.label ?? providerId;

  const toggleProvider = (providerId: string) => {
    setSessionProviderIds((current) => current.includes(providerId) ? current : [...current, providerId]);
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

  // Keep candidates mounted after selection, including temporarily missing providers.
  const choices = [...providerOptions, ...sessionProviderIds.filter((id) => !providerOptions.some((option) => option.id === id)).map((id) => ({ id, label: id }))];

  return (
    <Dialog open={open} onOpenChange={onOpenChange} className="nt-model-dialog"
      title={mode === "add" ? t("添加模型", "Add model") : t("编辑模型", "Edit model")}
      closeLabel={t("关闭", "Close")} initialFocusRef={nameRef} resolveReturnFocus={resolveReturnFocus}>
      <form id={formId} className="nt-model-dialog__body" onSubmit={submit}>
        <Field label={t("模型名称", "Model name")}>
          <Input ref={nameRef}
            value={value.model}
            disabled={locked}
            placeholder="deepseek-chat"
            onChange={(event) => {
              const model = event.currentTarget.value;
              setValue((current) => ({ ...current, model }));
            }}
          />
        </Field>
        {choices.length > 6 ? <MultiSelect label={t("服务商", "Providers")} disabled={locked}
          options={choices.map((option) => ({ value: option.id, label: option.label, description: option.label !== option.id ? option.id : undefined }))}
          values={value.providerIds} onValuesChange={(providerIds) => {
            setSessionProviderIds((current) => [...new Set([...current, ...providerIds])]);
            setValue((current) => ({ ...current, providerIds }));
          }}
          formatSummary={(count) => t(`已选 ${count} 项`, `${count} selected`)}
          search={{ label: t("搜索服务商", "Search providers"), emptyMessage: t("无匹配服务商", "No matching providers") }} />
          : <fieldset className="nt-model-dialog__choices" disabled={locked}>
            <legend>{t("服务商", "Providers")}</legend>
            {choices.map((option) => <Checkbox key={option.id} label={option.label} title={option.id}
              checked={value.providerIds.includes(option.id)} onChange={() => toggleProvider(option.id)} />)}
          </fieldset>}
        <section className="nt-model-dialog__chain-panel" aria-label={t("服务商优先级", "Provider priority")}>
          <header className="nt-model-dialog__chain-head">
            <strong>{t("服务商优先级", "Provider priority")}</strong>
            <span className="nt-model-dialog__chain-count">
              {value.providerIds.length}/{choices.length}
            </span>
          </header>
          {value.providerIds.length > 0 ? (
            <ol className="nt-model-dialog__chain">
              {value.providerIds.map((providerId, index) => (
                <li className="nt-model-dialog__chain-row" key={`selected:${providerId}`}>
                  <span className="nt-model-dialog__chain-order" aria-hidden="true">
                    {index + 1}
                  </span>
                  <strong className="nt-model-dialog__chain-label" title={providerId}>
                    {optionLabel(providerId)}
                  </strong>
                  <IconButton variant="quiet" compact
                    disabled={locked || index === 0}
                    label={t(
                      `将 ${optionLabel(providerId)} 上移一位`,
                      `Move ${optionLabel(providerId)} up one slot`,
                    )}
                    onClick={() => moveProvider(providerId, "up")}
                  >
                    <ArrowUp size={13} aria-hidden="true" />
                  </IconButton>
                  <IconButton variant="quiet" compact
                    disabled={locked || index === value.providerIds.length - 1}
                    label={t(
                      `将 ${optionLabel(providerId)} 下移一位`,
                      `Move ${optionLabel(providerId)} down one slot`,
                    )}
                    onClick={() => moveProvider(providerId, "down")}
                  >
                    <ArrowDown size={13} aria-hidden="true" />
                  </IconButton>
                </li>
              ))}
            </ol>
          ) : (
            <p className="nt-copy">
              {t("还没有选择服务商。", "No provider selected yet.")}
            </p>
          )}
        </section>
        {validationError ? <Notice tone="warning" role="alert">{validationError}</Notice> : null}
      </form>
      <div className="n-dialog-actions">
        <Button onClick={() => onOpenChange(false)}>{t("取消", "Cancel")}</Button>
        <Button variant="primary" type="submit" form={formId} disabled={locked} wrap>
          {mode === "add" ? t("添加模型", "Add model") : t("保存模型", "Save model")}
        </Button>
      </div>
    </Dialog>
  );
}
