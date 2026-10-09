import { useEffect, useRef, useState } from "react";
import { CashDialog } from "./CashDialog";
import { parseCashMicros } from "./cashAmount";
import { useCashBillingApi } from "./useCashBillingApi";
import type { TranslateFn } from "./accessKeysTypes";

export function AccountBillingDialog({
  providerId,
  accountId,
  name,
  locked,
  onClose,
  t,
}: {
  providerId: string;
  accountId: string;
  name: string;
  locked: boolean;
  onClose(): void;
  t: TranslateFn;
}) {
  const { api, token, sessionBusy } = useCashBillingApi();
  const [value, setValue] = useState("");
  const [loaded, setLoaded] = useState(false);
  const [pending, setPending] = useState(false);
  const [error, setError] = useState("");
  const [retry, setRetry] = useState(0);
  const lifetime = useRef<AbortController | null>(null);
  const inFlight = useRef(false);
  useEffect(() => {
    const controller = new AbortController();
    lifetime.current = controller;
    inFlight.current = false;
    setLoaded(false);
    setPending(false);
    setValue("");
    setError("");
    if (token)
      void api
        .pricing(token, providerId, controller.signal)
        .then((result) => {
          if (controller.signal.aborted) return;
          const current = Object.hasOwn(
            result.accountBillingMultipliers,
            accountId,
          )
            ? result.accountBillingMultipliers[accountId]
            : undefined;
          setValue(current === undefined ? "" : String(current));
          setLoaded(true);
        })
        .catch(() => {
          if (!controller.signal.aborted)
            setError(
              t("倍率读取失败，请重试。", "Could not load multiplier. Retry."),
            );
        });
    return () => controller.abort();
  }, [api, token, providerId, accountId, retry, t]);
  const disabled = locked || sessionBusy || !token || !loaded || pending;
  const save = async () => {
    const controller = lifetime.current;
    if (
      disabled ||
      inFlight.current ||
      !token ||
      !controller ||
      controller.signal.aborted
    )
      return;
    const input = value.trim();
    const ppm = input === "" ? 1_000_000 : parseCashMicros(input);
    if (ppm === null || ppm > 1_000_000_000_000) {
      setError(
        t(
          "倍率须为 0–1000000，最多六位小数。",
          "Use 0–1000000 with at most six decimal places.",
        ),
      );
      return;
    }
    inFlight.current = true;
    setPending(true);
    setError("");
    try {
      await api.saveMultiplier(
        token,
        providerId,
        accountId,
        input || null,
        controller.signal,
      );
      if (!controller.signal.aborted) onClose();
    } catch {
      if (!controller.signal.aborted) {
        // A transport failure does not prove the write failed. Read before another write.
        setLoaded(false);
        setError(
          t(
            "未能确认保存结果，请重新读取后确认。",
            "Save result unknown. Reload to verify before saving again.",
          ),
        );
      }
    } finally {
      if (!controller.signal.aborted) {
        inFlight.current = false;
        setPending(false);
      }
    }
  };
  return (
    <CashDialog
      title={t("账户计费倍率", "Account billing multiplier")}
      description={`${name} · ${accountId}`}
      busy={pending}
      onClose={onClose}
      t={t}
    >
      {!token ? (
        <p role="alert">
          {t("请先登录管理会话。", "Sign in to the management session.")}
        </p>
      ) : null}
      {!loaded && !error && token ? (
        <p role="status">{t("读取中…", "Loading…")}</p>
      ) : null}
      <form
        className="nt-stack"
        onSubmit={(event) => {
          event.preventDefault();
          void save();
        }}
      >
        <label className="nt-field">
          <span>{t("账户倍率", "Account multiplier")}</span>
          <input
            className="nt-input"
            value={value}
            disabled={disabled}
            maxLength={30}
            inputMode="decimal"
            placeholder="1"
            onChange={(event) => setValue(event.target.value)}
          />
        </label>
        <small>
          {t(
            "留空恢复默认 1；只影响新请求。",
            "Leave blank for default 1; applies to new requests only.",
          )}
        </small>
        {error ? (
          <div className="nt-alert nt-alert--danger" role="alert">
            {error}
          </div>
        ) : null}
        <div className="nt-cash-dialog__actions">
          {!loaded && error ? (
            <button
              type="button"
              className="nt-btn nt-btn--outline"
              disabled={pending || !token}
              onClick={() => setRetry(retry + 1)}
            >
              {t("重新读取", "Reload")}
            </button>
          ) : null}
          <button
            type="submit"
            className="nt-btn nt-btn--primary"
            disabled={disabled}
          >
            {pending ? t("保存中…", "Saving…") : t("保存", "Save")}
          </button>
        </div>
      </form>
    </CashDialog>
  );
}
