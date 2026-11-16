import { Button, ConfirmDialog, Notice } from "@neuro/ui";
import { useUiLocale } from "../../i18n/UiLocaleProvider";
import type { useFreebuffDeviceRegistration } from "./useFreebuffDeviceRegistration";
import type { FreebuffCatalogProps } from "./useFreebuffCatalog";
export function FreebuffDeviceTools({ props, origin, client, catalogBusy, flow, onRegister }: {
  props: FreebuffCatalogProps; origin: string; client: string; catalogBusy: boolean;
  flow: ReturnType<typeof useFreebuffDeviceRegistration>; onRegister(): void;
}) {
  const { t } = useUiLocale();
  const kind = flow.state?.registration;
  const label = kind === "stored" ? t("重新注册设备", "Re-register device")
    : kind === "unconfirmed" ? t("再次注册", "Register again") : t("注册设备", "Register device");
  const title = flow.approved?.registration === "stored" ? t("确认重新注册", "Confirm re-registration")
    : flow.approved?.registration === "unconfirmed" ? t("确认再次注册", "Confirm another registration") : t("确认设备注册", "Confirm device registration");
  return <div className="nt-stack" aria-label={t("FreeBuff 设备注册", "FreeBuff device registration")}>
    <div><Button compact disabled={!flow.enabled || flow.busy || catalogBusy} loading={flow.busy} onClick={() => void flow.read()}>{t("检查设备", "Check device")}</Button>
      {flow.state && <Button compact disabled={!flow.enabled || flow.busy || catalogBusy} onClick={flow.prepare}>{label}</Button>}
      {flow.busy && <Button compact onClick={flow.cancel}>{t("停止等待", "Stop waiting")}</Button>}</div>
    {kind === "absent" && <Notice>{t("尚无设备注册记录", "No device registration stored")}</Notice>}
    {kind === "stored" && <Notice>{t("设备注册已保存", "Device registration stored")}</Notice>}
    {kind === "unconfirmed" && <Notice tone="warning">{t("注册结果未确认", "Registration outcome unconfirmed")}</Notice>}
    {flow.error && <Notice tone="danger">{t(flow.error, "Device operation did not return a matching confirmation. Check device state before trying again.")}</Notice>}
    <ConfirmDialog open={!!flow.approved} onOpenChange={open => { if (!open) flow.dismiss(); }} title={title}
      description={t(`账号 ${props.credentialId} · ${origin} · ${client}：将发送此账号令牌并注册本机公钥。${flow.approved?.registration === "unconfirmed" ? "上一轮可能已经成功；再次注册不保证幂等。" : flow.approved?.registration === "stored" ? "将替换已保存的注册；新结果确认前保留旧记录，但不自动回退使用。" : ""}停止等待不会撤销远端注册。`,
        `Account ${props.credentialId} · ${origin} · ${client}: sends this account token and registers this installation's public key. ${flow.approved?.registration === "unconfirmed" ? "The previous attempt may have succeeded; another registration is not known to be idempotent. " : flow.approved?.registration === "stored" ? "Replaces the stored registration. Its previous receipt is retained until confirmation, without automatic fallback. " : ""}Stopping the wait cannot undo remote registration.`)}
      danger={flow.approved?.registration !== "absent"} confirmLabel={t("确认注册", "Confirm registration")} cancelLabel={t("取消", "Cancel")}
      confirmDisabled={!flow.enabled || catalogBusy || flow.busy || !flow.approved}
      onConfirm={() => { onRegister(); void flow.confirm(); }} />
  </div>;
}
